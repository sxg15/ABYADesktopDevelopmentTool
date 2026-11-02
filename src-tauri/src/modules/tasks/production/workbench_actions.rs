use super::*;

impl TaskService {
    /// APP-owned input. This cannot approve a document or close a defect.
    pub fn production_workbench(&self, input: ProductionMutation) -> AppResult<ProductionView> {
        let task = self.get(&input.task_id)?;
        let preview = if input.operation == "enter-playtest" {
            Some(self.preview_spec(&input.task_id)?)
        } else {
            None
        };
        self.production_commit(&input.task_id, input.expected_revision, |r| {
            match input.operation.as_str() {
                "enter-playtest" => {
                    let source = preview.as_ref().ok_or_else(conflict)?;
                    if r.current_stage != "implementation"
                        || Some(&source.version) != r.current_version.as_ref()
                        || input.data["version"].as_str() != r.current_version.as_deref()
                        || r.workbench.attempts.iter().any(|a| matches!(a.status.as_str(),
                            "running" | "sending" | "queued" | "reconciling"))
                    {
                        return Err(AppError::validation("请先停止执行，并核对当前候选的试玩包。"));
                    }
                    // 用户主动转入反馈迭代；保留失败检查、问题、预算和原始工作项。
                    r.stages.insert("implementation".into(), "playable".into());
                    r.stages.insert("review".into(), "awaiting-feedback".into());
                    r.current_stage = "review".into();
                    r.workbench.execution.state = "paused".into();
                    r.workbench.execution.active_unit_id = None;
                    autonomy::event(&mut r.workbench.execution, "user-entered-playtest", None,
                        "用户选择带着待处理项进入试玩与修改；未批准交付或更改检查结果。");
                }
                "revoke-prototype" => {
                    let id=mutation::text(&input.data,"id")?;
                    let grant=r.workbench.prototype_grants.iter_mut().find(|g|g["id"]==id)
                        .ok_or_else(||AppError::validation("授权不存在。"))?;
                    grant["status"]=json!("revoked");
                }
                "attach" => super::workbench_import::attach(
                    r,
                    Path::new(&task.workspace_path),
                    &input.data,
                )?,
                "save-draft" => {
                    let key = mutation::text(&input.data, "key")?;
                    let text = input.data["text"].as_str().unwrap_or("");
                    if key.len() > 160 || text.len() > 32000 || r.workbench.drafts.len() > 200 {
                        return Err(AppError::validation("草稿超出大小限制。"));
                    }
                    r.workbench.drafts.insert(key.into(), json!(text));
                }
                "request" => {
                    let id = mutation::text(&input.data, "id")?;
                    let text = mutation::text(&input.data, "text")?;
                    if text.len() > 32000 || id.len() > 80 || r.workbench.requests.len() >= 200 {
                        return Err(AppError::validation("请求超出大小限制。"));
                    }
                    if let Some(old) = r.workbench.requests.iter().find(|v| v["id"] == id) {
                        if old["text"] != text {
                            return Err(conflict());
                        }
                        return Ok(());
                    }
                    r.workbench
                        .requests
                        .push(json!({"id":id,"text":text,"stage":r.current_stage,
                        "revision":r.revision+1,"status":"pending","createdAt":now()}));
                    r.workbench.drafts.remove("current");
                }
                "defer" => {
                    let id = mutation::text(&input.data, "id")?;
                    let reason = mutation::text(&input.data, "reason")?;
                    let impact = mutation::text(&input.data, "impact")?;
                    let issue = r
                        .issues
                        .iter()
                        .find(|i| i["id"] == id)
                        .ok_or_else(|| AppError::validation("问题已不存在。"))?;
                    // Only optional suggestions are deferrable; required blockers retain all gates.
                    if issue["kind"] != "suggestion" || issue["status"] == "resolved" {
                        return Err(AppError::validation("此项是必要检查或阻塞，不能略过。"));
                    }
                    if reason.len() > 4000 || impact.len() > 4000 {
                        return Err(AppError::validation("说明过长。"));
                    }
                    r.workbench.deferrals.retain(|v| v["id"] != id);
                    r.workbench
                        .deferrals
                        .push(json!({"id":id,"description":issue["description"],"stage":issue["stage"],"affectedStages":issue["affectedStages"],"blockedOperations":issue["blockedOperations"],
                        "reason":reason,"impact":impact,"status":"deferred","updatedAt":now()}));
                }
                "restore" => {
                    let id = mutation::text(&input.data, "id")?;
                    let entry = r
                        .workbench
                        .deferrals
                        .iter_mut()
                        .find(|v| v["id"] == id)
                        .ok_or_else(|| AppError::validation("找不到略过记录。"))?;
                    entry["status"] = json!("restored");
                    entry["updatedAt"] = json!(now());
                }
                _ => return Err(AppError::validation("不支持的工作台操作。")),
            }
            Ok(())
        })?;
        self.production_get(&input.task_id)
    }

    pub fn production_snapshot(&self, task: &str, revision: u64) -> AppResult<ProductionRecord> {
        self.get(task)?;
        self.database.with_connection(|c| {
            let current = load(c, task)?.ok_or_else(|| AppError::validation("任务尚未初始化。"))?;
            if current.revision == revision {
                return Ok(current);
            }
            let body: Option<String> = c
                .query_row(
                    "SELECT body FROM task_production_history WHERE task_id=?1 AND revision=?2",
                    rusqlite::params![task, revision as i64],
                    |r| r.get(0),
                )
                .optional()?;
            serde_json::from_str(&body.ok_or_else(|| AppError::validation("历史版本不可用。"))?)
                .map_err(Into::into)
        })
    }
}
