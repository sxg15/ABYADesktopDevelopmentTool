use super::*;

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["iterationMode"] == "human-feedback"
}

pub(super) fn save_test(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    if !enabled(r) {
        return Err(AppError::validation(
            "旧任务继续使用固定轮次，须明确升级后使用必要自测。",
        ));
    }
    validation::approved(r, root, "requirements")?;
    validation::approved(r, root, "plan")?;
    let version = validation::version(r)?.to_owned();
    let id = mutation::text(data, "id")?;
    mutation::text(data, "observations")?;
    let status = mutation::text(data, "status")?;
    if !["passed", "failed", "blocked", "not-applicable"].contains(&status) {
        return Err(AppError::validation("无效自测状态。"));
    }
    if status == "passed" {
        validation::evidence_refs(r, root, &data["evidenceIds"], &version)?;
    }
    if status == "not-applicable" && (id != "authority" || r.player_mode != "single") {
        return Err(AppError::validation(
            "只有单人任务的多人权限检查可标记不适用。",
        ));
    }
    let mut entry = data.clone();
    entry["version"] = json!(version);
    entry["cycle"] = json!(r.cycle);
    entry["recordedAt"] = json!(now());
    r.self_tests.push(entry);
    Ok(())
}

pub(super) fn ready(r: &ProductionRecord, root: &Path) -> AppResult<()> {
    versions::verify(&r.version_details)?;
    let version = validation::version(r)?;
    let required = r.policy["selfTestChecks"]
        .as_array()
        .ok_or_else(|| AppError::validation("缺少必要自测配置。"))?;
    for id in required {
        let check = r
            .self_tests
            .iter()
            .rev()
            .find(|c| c["id"] == *id && c["version"] == version && c["cycle"] == r.cycle)
            .ok_or_else(|| {
                AppError::validation(format!(
                    "当前版本缺少必要自测：{}。",
                    id.as_str().unwrap_or("")
                ))
            })?;
        if check["status"] == "not-applicable" && id == "authority" && r.player_mode == "single" {
            continue;
        }
        if check["status"] != "passed" {
            return Err(AppError::validation("当前版本仍有未通过的必要自测。"));
        }
        validation::evidence_refs(r, root, &check["evidenceIds"], version)?;
    }
    Ok(())
}

pub(super) fn reuse_evidence(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    if !enabled(r) {
        return Err(AppError::validation("旧轮次证据不允许以复用代替本轮执行。"));
    }
    let id = mutation::text(data, "id")?;
    let source = mutation::text(data, "sourceId")?;
    let reason = mutation::text(data, "reason")?;
    let scope = mutation::text(data, "unaffectedScope")?;
    if r.evidence.iter().any(|e| e.id == id) {
        return Err(AppError::validation("证据ID已存在。"));
    }
    let old = r
        .evidence
        .iter()
        .find(|e| e.id == source)
        .ok_or_else(|| AppError::validation("来源证据不存在。"))?;
    if !old.reviewed
        || old.cycle != r.cycle
        || files::hash_file(&files::safe_path(root, &old.path)?)?.0 != old.sha256
    {
        return Err(AppError::validation("来源证据未审阅、过期或已变化。"));
    }
    let mut e = old.clone();
    e.id = id.into();
    e.version = validation::version(r)?.into();
    e.recorded_at = now();
    e.stage = r.current_stage.clone();
    e.description = format!(
        "复用证据 {source}（原版本 {}）；未受影响范围：{scope}；依据：{reason}。不是本版本重新执行。",
        old.version
    );
    r.evidence.push(e);
    Ok(())
}

pub(super) fn update_feedback(
    r: &mut ProductionRecord,
    root: &Path,
    data: &Value,
) -> AppResult<()> {
    let id = mutation::text(data, "id")?;
    let status = mutation::text(data, "status")?;
    if !["in-progress", "awaiting-recheck"].contains(&status) {
        return Err(AppError::validation(
            "AI只能记录处理中或已修复待复验；关闭反馈由策划操作。",
        ));
    }
    let fix = mutation::text(data, "fix")?;
    let index = r
        .feedback
        .iter()
        .position(|f| f["id"] == id)
        .ok_or_else(|| AppError::validation("反馈不存在。"))?;
    if r.feedback[index]["status"] == "resolved" {
        return Err(AppError::validation(
            "已关闭反馈保留历史，请重新打开后处理。",
        ));
    }
    if status == "awaiting-recheck" {
        validation::evidence_refs(r, root, &data["evidenceIds"], validation::version(r)?)?;
        mutation::text(data, "recheck")?;
    }
    let at = now();
    let version = r.current_version.clone();
    let f = &mut r.feedback[index];
    f["status"] = json!(status);
    f["fix"] = json!(fix);
    f["recheck"] = data["recheck"].clone();
    f["candidateVersion"] = json!(version);
    f["evidenceIds"] = data["evidenceIds"].clone();
    f["updatedAt"] = json!(at);
    if let Some(history) = f["history"].as_array_mut() {
        history.push(json!({"status":status,"actor":"ai","version":version,"at":at,"fix":fix}));
    }
    Ok(())
}

impl TaskService {
    // This entry is only exposed by Tauri, never by the agent mutation dispatcher.
    pub fn production_feedback(&self, input: ProductionMutation) -> AppResult<ProductionView> {
        let root = self.production_workspace(&input.task_id)?;
        self.production_commit(&input.task_id,input.expected_revision,|r| {
            let data = &input.data;
            match input.operation.as_str() {
                "attach" => super::visuals::attach_feedback(r, &root, data)?,
                "add" => {
                    let description = mutation::text(data,"description")?;
                    if description.chars().count() > 8000 { return Err(AppError::validation("反馈请控制在8000字以内。")); }
                    let artifact = data["artifactId"].as_str();
                    if let Some(id) = artifact && !r.artifacts.iter().any(|a| a["id"] == id) {
                        return Err(AppError::validation("所选视觉产物不属于此任务。"));
                    }
                    let attachments = data.get("attachmentIds").cloned().unwrap_or(json!([]));
                    if attachments.as_array().is_none_or(|ids| ids.len() > 8 || ids.iter().any(|id|
                        !r.artifacts.iter().any(|a| a["id"] == *id && a["sourceType"] == "feedback"))) {
                        return Err(AppError::validation("反馈附件必须来自本任务，最多8个。"));
                    }
                    let at = now();
                    let instances = r.acceptance_sessions.last().map(|s| s["instanceIds"].clone()).unwrap_or(json!([]));
                    r.feedback.push(json!({"id":uuid::Uuid::new_v4().to_string(),"description":description,
                        "artifactId":artifact,"attachmentIds":attachments,"version":r.current_version,
                        "instanceIds":instances,"status":"open","stage":if artifact.is_some() {"resources"} else if r.current_version.is_none() {r.current_stage.as_str()} else {"review"},
                        "createdAt":at,"updatedAt":at,"history":[{"actor":"user","status":"open","at":at}]}));
                    if r.current_version.is_some() {
                        r.stages.insert("delivery".into(),"not-started".into());
                        r.stages.insert("closeout".into(),"not-started".into());
                        r.stages.insert("review".into(),"in-progress".into());
                        r.current_stage="review".into();
                    }
                }
                "resolve" | "reopen" => {
                    let id = mutation::text(data,"id")?;
                    let index = r.feedback.iter().position(|f| f["id"] == id).ok_or_else(|| AppError::validation("反馈不存在。"))?;
                    let resolving = input.operation == "resolve";
                    if resolving && r.feedback[index]["stage"] == "review" && r.feedback[index]["artifactId"].is_null() {
                        if r.feedback[index]["status"] != "awaiting-recheck" || r.feedback[index]["candidateVersion"] != json!(r.current_version) {
                            return Err(AppError::validation("请先等待当前版本修复与复验完成。"));
                        }
                        versions::verify(&r.version_details)?;
                        validation::evidence_refs(r,&root,&r.feedback[index]["evidenceIds"],validation::version(r)?)?;
                    }
                    let status = if resolving {"resolved"} else {"open"};
                    let at = now();
                    r.feedback[index]["status"] = json!(status); r.feedback[index]["updatedAt"] = json!(at);
                    r.feedback[index]["history"].as_array_mut().unwrap().push(json!({"actor":"user","status":status,"at":at,"version":r.current_version}));
                    if !resolving && r.current_version.is_some() {
                        r.stages.insert("delivery".into(),"not-started".into());
                        r.stages.insert("closeout".into(),"not-started".into());
                        r.stages.insert("review".into(),"in-progress".into()); r.current_stage="review".into();
                    }
                }
                _ => return Err(AppError::validation("无效人工反馈操作。")),
            }
            Ok(())
        })?;
        self.production_export(&input.task_id)?;
        self.production_get(&input.task_id)
    }
}
