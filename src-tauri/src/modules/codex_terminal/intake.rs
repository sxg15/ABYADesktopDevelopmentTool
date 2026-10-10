use super::intake_receipt::{self, IntakeContinueStatus, ReceiptDecision};
use super::*;

impl CodexTerminalService {
    pub fn continue_questions(
        &self,
        task_id: &str,
        group_id: &str,
        revision: u64,
    ) -> AppResult<IntakeContinuation> {
        let record = self
            .tasks
            .production_get(task_id)?
            .record
            .ok_or_else(|| AppError::validation("制作流程尚未启用。"))?;
        let group = record
            .question_groups
            .iter()
            .find(|g| g.id == group_id)
            .ok_or_else(|| AppError::validation("问题组不存在。"))?;
        if group.provider != "codex"
            || group.status != "submitted"
            || group.answers.last().is_none_or(|a| a.revision != revision)
        {
            return Err(AppError::validation("答案已变化，请刷新后继续。"));
        }
        self.request_continuation(
            task_id,
            &group.conversation_id,
            Some(group_id),
            revision,
            None,
        )
    }

    pub(super) fn continue_questions_locked(
        &self,
        task_id: &str,
        group_id: &str,
        revision: u64,
    ) -> AppResult<IntakeContinuation> {
        let record = self
            .tasks
            .production_get(task_id)?
            .record
            .ok_or_else(|| AppError::validation("制作流程尚未启用。"))?;
        let group = record
            .question_groups
            .iter()
            .find(|g| g.id == group_id)
            .ok_or_else(|| AppError::validation("问题组不存在。"))?;
        if group.provider != "codex"
            || group.status != "submitted"
            || group.answers.last().is_none_or(|a| a.revision != revision)
        {
            return Err(AppError::validation("答案已变化，请刷新后继续。"));
        }
        let conversation = self.conversation(task_id, &group.conversation_id)?;
        let native = conversation
            .native_session_id
            .as_deref()
            .ok_or_else(|| AppError::validation("尚未关联原生会话。"))?;
        if conversation.archived || group.native_session_id.as_deref() != Some(native) {
            return Err(AppError::validation(
                "问题与当前原生会话不匹配，请先修复关联。",
            ));
        }
        let task = self.tasks.get(task_id)?;
        let receipt =
            conversation_directory(Path::new(&task.workspace_path), &group.conversation_id)
                .join(format!("intake-{}-{revision}.json", group.id));
        let previous = if receipt.exists() {
            Some(serde_json::from_slice::<serde_json::Value>(
                &std::fs::read(&receipt)?,
            )?)
        } else {
            None
        };
        let runtime = self
            .runtime_servers
            .lock()
            .get(&group.conversation_id)
            .cloned();
        let response = |status, turn_id| IntakeContinuation {
            request_id: None,
            status,
            conversation_id: group.conversation_id.clone(),
            native_session_id: native.into(),
            turn_id,
        };
        let snapshot = if previous.is_some() {
            let server = match runtime.clone() {
                Some(server) if server.is_alive() => server,
                _ => self.ensure_app_server(
                    &(self.resolver)().ok_or_else(|| AppError::validation("Codex 不可用。"))?,
                )?,
            };
            Some(server.request(
                "thread/read",
                json!({"threadId":native,"includeTurns":true}),
            )?)
        } else {
            None
        };
        let decision = intake_receipt::decide(previous.as_ref(), native, snapshot.as_ref())?;
        if let ReceiptDecision::Existing(status) = decision {
            return Ok(response(
                status,
                previous
                    .as_ref()
                    .and_then(|v| v["turnId"].as_str().map(str::to_owned)),
            ));
        }
        let connected = self.session(&group.conversation_id).is_ok()
            && runtime.as_ref().is_some_and(|s| s.is_alive());
        if !connected {
            return Ok(response(IntakeContinueStatus::NeedsConnection, None));
        }
        let server = runtime.unwrap();
        server.ensure_idle(native)?;
        let resuming = decision == ReceiptDecision::Resume;
        let mut attempt = intake_receipt::start_attempt(&receipt, previous.as_ref(), native)?;
        let recovery = if resuming {
            "上次执行被中断或失败。先核对上次已完成的操作及实际状态，只继续未完成部分，不要盲目重放写入。"
        } else {
            ""
        };
        let result = server.request("turn/start", json!({"threadId":native,"clientUserMessageId":attempt["messageId"],
            "input":[{"type":"text","text":format!("策划已提交阶段问题组 {} 第 {} 版答案的处理。{}请先运行 doctor，再 production get 读取最新答案和人数模式。文档确认状态以实际记录为准，提交问答本身不代表批准文档。",group.id,revision,recovery)}]}))?;
        let turn = result["turn"]["id"].as_str().ok_or_else(|| {
            AppError::validation("Codex 未返回执行 ID，请核对原生历史；未自动重发。")
        })?;
        attempt["status"] = json!("sent");
        attempt["turnId"] = json!(turn);
        intake_receipt::save(&receipt, &attempt, false)?;
        Ok(response(
            if resuming {
                IntakeContinueStatus::Resumed
            } else {
                IntakeContinueStatus::Started
            },
            Some(turn.into()),
        ))
    }
}
