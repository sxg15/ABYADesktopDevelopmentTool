use super::*;

pub(super) fn authorize(r: &ProductionRecord, run: &str) -> AppResult<()> {
    if r.no_questions["enabled"] != true || r.no_questions["runId"] != run {
        return Err(AppError::validation(
            "不提问模式已关闭或本轮已结束，请回读当前状态。",
        ));
    }
    Ok(())
}
impl TaskService {
    pub fn no_questions_mode(&self, id: &str, enable: bool) -> AppResult<ProductionView> {
        let r = self
            .production_get(id)?
            .record
            .ok_or_else(|| AppError::validation("请先启用完整制作流程。"))?;
        if (r.no_questions["enabled"] == true) == enable {
            return self.production_get(id);
        }
        let player = if enable {
            Some(self.task_player_status(id)?)
        } else {
            None
        };
        if let Some(player) = &player {
            self.adopt_task_player(id, mutation::text(player, "selectedHash")?)?;
        }
        self.production_commit(id,r.revision,|r|{
            let mut history=r.no_questions["history"].as_array().cloned().unwrap_or_default();
            if !r.no_questions.is_null(){let mut old=r.no_questions.clone();old.as_object_mut().unwrap().remove("history");history.push(old);}
            if history.len()>30{history.remove(0);}
            if enable{
                r.no_questions=json!({"enabled":true,"runId":uuid::Uuid::new_v4().to_string(),"status":"starting","startedAt":now(),
                    "baselineVersion":r.current_version,"baselineArchiveHash":r.version_details["archiveHash"],
                    "baselinePackage":r.workbench.execution.verification_package["sha256"],"player":player,
                    "previousQuestionMode":r.question_mode,"history":history,"message":"已授权本项目自行审阅与推进，等待 Codex 接续。"});
                r.question_mode="no-followup".into();
            }else{
                r.question_mode=r.no_questions["previousQuestionMode"].as_str().unwrap_or("ask").into();
                r.no_questions["enabled"]=json!(false);r.no_questions["status"]=json!("off");r.no_questions["endedAt"]=json!(now());
                r.no_questions["message"]=json!("已关闭自动审阅和推进。");
            }
            Ok(())
        })?;
        self.production_get(id)
    }
    pub fn no_questions_respond(
        &self,
        id: &str,
        revision: u64,
        op: &str,
        data: &Value,
    ) -> AppResult<ProductionView> {
        let run = mutation::text(data, "runId")?;
        if op == "auto-review" {
            let reason = mutation::text(data, "review")?;
            if reason.chars().count() < 12 {
                return Err(AppError::validation("请记录实际审阅结果与采用的判断。"));
            }
            return self.decide_with_source(
                ProductionDecision {
                    task_id: id.into(),
                    expected_revision: revision,
                    kind: mutation::text(data, "kind")?.into(),
                    document_hash: mutation::text(data, "documentHash")?.into(),
                    accepted: true,
                    feedback: format!("[AI 自审 · 不提问模式] {reason}"),
                },
                Some(run),
            );
        }
        let root = self.production_workspace(id)?;
        self.production_commit(id,revision,|r|{
            authorize(r,run)?;
            match op{
                "auto-answer"=>{
                    let action=mutation::text(data,"action")?;
                    if !["submit-answers","set-player-mode"].contains(&action){return Err(conflict())}
                    questions::answer(r,action,&data["answer"])?;
                    r.no_questions["lastDecision"]=json!({"action":action,"data":data["answer"],"at":now(),"source":"ai-self-review"});
                }
                "auto-block"=>{
                    r.no_questions["status"]=json!("blocked");r.no_questions["message"]=json!(mutation::text(data,"reason")?);
                    r.no_questions["blockedRevision"]=json!(r.revision+1);
                }
                "auto-finish"=>finish(r,&root)?,
                _=>return Err(AppError::validation("未知不提问模式操作。")),
            }
            Ok(())
        })?;
        self.production_export(id)?;
        self.production_get(id)
    }
    pub fn no_questions_status(
        &self,
        id: &str,
        run: &str,
        status: &str,
        message: &str,
    ) -> AppResult<()> {
        let r = self.production_get(id)?.record.ok_or_else(conflict)?;
        if r.no_questions["enabled"] != true || r.no_questions["runId"] != run {
            return Ok(());
        }
        if r.no_questions["status"] == status && r.no_questions["message"] == message {
            return Ok(());
        }
        self.production_commit(id, r.revision, |r| {
            authorize(r, run)?;
            r.no_questions["status"] = json!(status);
            r.no_questions["message"] = json!(message);
            Ok(())
        })
    }
    pub fn no_questions_tasks(&self) -> AppResult<Vec<String>> {
        self.database.with_connection(|c|{
            let mut q=c.prepare("SELECT task_id FROM task_production p JOIN development_tasks t ON t.id=p.task_id WHERE t.status='active' AND json_extract(body,'$.noQuestions.enabled')=1")?;
            Ok(q.query_map([],|r|r.get(0))?.collect::<Result<Vec<_>,_>>()?)
        })
    }
    pub fn no_questions_finish_if_ready(&self, id: &str) -> AppResult<bool> {
        let r = self.production_get(id)?.record.ok_or_else(conflict)?;
        if r.no_questions["enabled"] != true {
            return Ok(true);
        }
        let root = self.production_workspace(id)?;
        let mut candidate = r.clone();
        if finish(&mut candidate, &root).is_err() {
            return Ok(false);
        }
        self.production_commit(id, r.revision, |r| finish(r, &root))?;
        self.production_export(id)?;
        Ok(true)
    }
}
fn finish(r: &mut ProductionRecord, root: &Path) -> AppResult<()> {
    let m = &r.no_questions;
    let pack = &r.workbench.execution.verification_package;
    if m["enabled"] != true
        || r.current_version.is_none()
        || pack["version"] == m["baselineVersion"]
        || pack["sha256"] == m["baselinePackage"]
        || r.version_details["archiveHash"] == m["baselineArchiveHash"]
        || !verification_package::valid(r, root)
    {
        return Err(AppError::validation(
            "尚未取得本轮新产出且通过实测的试玩包。",
        ));
    }
    let created = pack["createdAt"]
        .as_str()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
    let started = m["startedAt"]
        .as_str()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
    if !created.zip(started).is_some_and(|(a, b)| a >= b) {
        return Err(AppError::validation("试玩包不是本轮开启之后生成的。"));
    }
    let version = r.current_version.as_deref().ok_or_else(conflict)?;
    for check in ["lifecycle", "authority"] {
        if check == "authority" && r.player_mode != "multiplayer" {
            continue;
        }
        let proof = r
            .self_tests
            .iter()
            .rev()
            .find(|t| t["id"] == check && t["version"] == version && t["cycle"] == r.cycle)
            .ok_or_else(|| {
                AppError::validation(format!("新版本仍缺少 {check} 的实际通过回执。"))
            })?;
        if proof["status"] != "passed" {
            return Err(AppError::validation(format!(
                "新版本的 {check} 最近一次检查未通过。"
            )));
        }
        validation::evidence_refs(r, root, &proof["evidenceIds"], version)?;
    }
    let previous_mode = m["previousQuestionMode"]
        .as_str()
        .unwrap_or("ask")
        .to_owned();
    let version = pack["version"].clone();
    let package = pack["path"].clone();
    // Reuse the normal handoff, including art, issue, question and self-test gates.
    // A new ad-hoc stage status would strand the playable at the feedback gate.
    mutation::apply(
        r,
        root,
        "complete-stage",
        &json!({"stage":"implementation"}),
    )?;
    r.question_mode = previous_mode;
    r.no_questions["enabled"] = json!(false);
    r.no_questions["status"] = json!("ready");
    r.no_questions["endedAt"] = json!(now());
    r.no_questions["version"] = version;
    r.no_questions["package"] = package;
    r.no_questions["message"] = json!("新试玩版已生成，本轮不提问模式已结束。");
    Ok(())
}
