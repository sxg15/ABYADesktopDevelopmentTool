use super::continuation_queue::{self, ContinueRequest};
use super::intake_receipt::IntakeContinueStatus as S;
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RepairIntent {
    pub issue_id: String,
    pub fingerprint: String,
}

pub(super) fn receipt_path(directory: &Path, fingerprint: &str) -> AppResult<PathBuf> {
    if fingerprint.len() != 64 || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::validation("修复请求标识无效，请核对记录。"));
    }
    Ok(directory.join(format!("issue-repair-{fingerprint}.json")))
}

impl CodexTerminalService {
    pub fn repair_issue(
        &self,
        task: &str,
        conversation: &str,
        issue: &str,
        revision: u64,
    ) -> AppResult<IntakeContinuation> {
        let _guard = self.lifecycle_lock.lock();
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst)
            || self.stopping.lock().contains(conversation)
        {
            return Err(AppError::validation("正在停止执行，请稍后再请求修复。"));
        }
        let spec: crate::modules::tasks::IssueRepairSpec =
            self.tasks
                .prepare_issue_repair(task, issue, Some(revision))?;
        let metadata = self.conversation(task, conversation)?;
        if metadata.archived {
            return Err(AppError::validation("原会话已归档，请先恢复。"));
        }
        let native = metadata
            .native_session_id
            .ok_or_else(|| AppError::validation("请先打开任务的原会话，再请求 AI 修复。"))?;
        let directory = conversation_directory(
            Path::new(&self.tasks.get(task)?.workspace_path),
            conversation,
        );
        let previous = continuation_queue::read(&directory)?;
        if previous.as_ref().is_some_and(|q| {
            q.task_id != task || q.conversation_id != conversation || q.native_id != native
        }) {
            return Err(AppError::validation(
                "待处理请求与当前任务会话不匹配，请核对记录。",
            ));
        }
        // Updating progress on the same issue does not authorize a second repair
        // while its first native turn is still running.
        if let Some(old) = previous.as_ref().filter(|q| {
            q.repair.as_ref().is_some_and(|r| r.issue_id == issue)
                && matches!(q.status.as_str(), "sent" | "running")
        }) {
            let runtime = self.runtime_servers.lock().get(conversation).cloned();
            if let Some(server) = runtime.filter(|s| s.is_alive()) {
                let snapshot = server.request(
                    "thread/read",
                    json!({"threadId":native,"includeTurns":true}),
                )?;
                if snapshot["thread"]["id"] == native
                    && snapshot["thread"]["turns"].as_array().is_some_and(|turns| {
                        turns.iter().any(|t| {
                            t["id"].as_str() == old.turn_id.as_deref()
                                && t["status"] == "inProgress"
                        })
                    })
                {
                    return Ok(old.response(S::Running));
                }
            }
        }
        if let Some(old) = previous.as_ref().filter(|q| {
            matches!(
                q.status.as_str(),
                "queued"
                    | "needsConnection"
                    | "sending"
                    | "needsReview"
                    | "waitingForAnswers"
                    | "waitingForApproval"
            )
        }) {
            if old
                .repair
                .as_ref()
                .is_none_or(|r| r.fingerprint != spec.fingerprint)
            {
                return Err(AppError::validation(
                    "已有其他操作等待处理，请先查看原会话；未覆盖已有请求。",
                ));
            }
            if old.owner == self.process_epoch {
                let mut old = old.clone();
                return self.dispatch_or_record(&directory, &mut old);
            }
        }
        let path = receipt_path(&directory, &spec.fingerprint)?;
        if path.exists() {
            let mut old: ContinueRequest = serde_json::from_slice(&std::fs::read(path)?)?;
            if old.native_id != native
                || old.task_id != task
                || old.conversation_id != conversation
                || old.repair.as_ref().is_none_or(|r| {
                    r.issue_id != spec.issue_id || r.fingerprint != spec.fingerprint
                })
            {
                return Err(AppError::validation(
                    "旧修复请求与原会话身份不匹配，请核对记录。",
                ));
            }
            if matches!(old.status.as_str(), "sending" | "needsReview") {
                return Ok(old.response(S::NeedsReview));
            }
            if matches!(old.status.as_str(), "sent" | "running" | "settled") {
                if old.owner != self.process_epoch {
                    return Ok(old.response(S::NeedsReview));
                }
                return self.dispatch_or_record(&directory, &mut old);
            }
        }
        let mut request = ContinueRequest {
            id: Uuid::new_v4().to_string(),
            task_id: task.into(),
            conversation_id: conversation.into(),
            native_id: native,
            repair: Some(RepairIntent {
                issue_id: spec.issue_id,
                fingerprint: spec.fingerprint,
            }),
            group_id: None,
            answer_revision: 0,
            after_revision: None,
            automatic_after: None,
            status: "queued".into(),
            owner: self.process_epoch.clone(),
            revision: previous.as_ref().map_or(0, |q| q.revision),
            updated_at: String::new(),
            after_turn: None,
            turn_id: None,
            error: None,
        };
        self.cancelled.lock().remove(conversation);
        self.queue_tasks
            .lock()
            .insert(conversation.into(), task.into());
        continuation_queue::save(&directory, &mut request)?;
        self.dispatch_or_record(&directory, &mut request)
    }
}
