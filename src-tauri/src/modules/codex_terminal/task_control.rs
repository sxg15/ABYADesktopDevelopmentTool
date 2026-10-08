use super::continuation_queue;
use super::*;
use crate::modules::tasks::{ProductionMutation, ProductionView};
use std::sync::atomic::Ordering;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionSubmission {
    pub production: ProductionView,
    pub continuation: Option<IntakeContinuation>,
    pub warning: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskControlState {
    pub task_id: String,
    pub conversation_id: String,
    pub native_session_id: Option<String>,
    pub state: String,
    pub connection: String,
    pub stage: Option<String>,
    pub turn_id: Option<String>,
    pub turn_started_at: Option<String>,
    pub queued: bool,
    pub request_id: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
    pub last_event_at: String,
    pub sequence: u64,
}

impl CodexTerminalService {
    pub fn submit_questions(&self, input: ProductionMutation) -> AppResult<QuestionSubmission> {
        if input.operation != "submit-answers" {
            return Err(AppError::validation("该入口只用于提交答案并继续。"));
        }
        let task = input.task_id.clone();
        let id = input.data["id"].as_str().unwrap_or("").to_string();
        let production = self.tasks.production_answer(input)?;
        let group = production
            .record
            .as_ref()
            .and_then(|r| r.question_groups.iter().find(|g| g.id == id));
        let outcome = match group {
            Some(g) if g.provider == "codex" => {
                self.continue_questions(&task, &id, g.answers.last().unwrap().revision)
            }
            _ => {
                return Ok(QuestionSubmission {
                    production,
                    continuation: None,
                    warning: Some("答案已保存，请在对应提供方的原会话中继续。".into()),
                });
            }
        };
        Ok(match outcome {
            Ok(continuation) => QuestionSubmission {
                production,
                continuation: Some(continuation),
                warning: None,
            },
            Err(error) => QuestionSubmission {
                production,
                continuation: None,
                warning: Some(format!("答案已保存；继续请求尚未完成：{}", error.message)),
            },
        })
    }

    pub fn task_control(
        &self,
        task_id: &str,
        conversation_id: &str,
    ) -> AppResult<TaskControlState> {
        let _guard = self.lifecycle_lock.lock();
        let task = self.tasks.get(task_id)?;
        let metadata = self.conversation(task_id, conversation_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let queue = continuation_queue::read(&directory)?;
        if queue.is_some() {
            self.queue_tasks
                .lock()
                .insert(conversation_id.into(), task_id.into());
        }
        let workflow = self.workflow(task_id, conversation_id)?;
        let turn = workflow
            .current_turn_id
            .as_ref()
            .and_then(|id| workflow.turns.iter().find(|t| &t.id == id));
        let active = turn.is_some_and(|t| {
            matches!(
                t.status,
                CodexWorkflowTurnStatus::WaitingForPlan | CodexWorkflowTurnStatus::InProgress
            )
        });
        let runtime = self
            .runtime_servers
            .lock()
            .get(conversation_id)
            .is_some_and(|s| s.is_alive());
        let connected = self.sessions.lock().contains_key(conversation_id);
        let production = self.tasks.production_get(task_id)?;
        let queued = queue.as_ref().is_some_and(|q| {
            q.status == "queued"
                && q.owner == self.process_epoch
                && !self.cancelled.lock().contains(conversation_id)
        });
        let state = if self.stopping.lock().contains(conversation_id) {
            "pausing"
        } else if runtime && active {
            if queued {
                "queued"
            } else if turn.is_some_and(|t| t.activities.is_empty()) {
                "waitingForResponse"
            } else {
                "running"
            }
        } else if task.status != crate::modules::tasks::TaskStatus::Active {
            "closed"
        } else if queue
            .as_ref()
            .is_some_and(|q| q.status == "sending" || q.status == "needsReview")
        {
            "needsReview"
        } else if queue.as_ref().is_some_and(|q| {
            q.status == "paused"
                || ((q.status == "queued" || q.status == "needsConnection")
                    && q.owner != self.process_epoch)
        }) {
            "paused"
        } else if queued {
            "queued"
        } else if queue
            .as_ref()
            .is_some_and(|q| q.status == "needsConnection")
        {
            "disconnected"
        } else if let Some(wait) = continuation_queue::waiting(Some(&production)) {
            if wait == super::intake_receipt::IntakeContinueStatus::WaitingForAnswers {
                "waitingForAnswers"
            } else {
                "waitingForApproval"
            }
        } else if queue.as_ref().is_some_and(|q| q.status == "needsResume") {
            "failed"
        } else if active && !runtime {
            "disconnected"
        } else if turn.is_some_and(|t| t.status == CodexWorkflowTurnStatus::Interrupted) {
            "paused"
        } else if turn.is_some_and(|t| t.status == CodexWorkflowTurnStatus::Failed) {
            "failed"
        } else if turn.is_none() {
            "notStarted"
        } else {
            "ready"
        };
        Ok(TaskControlState {
            task_id: task_id.into(),
            conversation_id: conversation_id.into(),
            native_session_id: metadata.native_session_id,
            state: state.into(),
            connection: if connected {
                "connected"
            } else if runtime {
                "background"
            } else {
                "disconnected"
            }
            .into(),
            stage: production.record.as_ref().map(|r| r.current_stage.clone()),
            turn_id: turn.map(|t| t.id.clone()),
            turn_started_at: turn.map(|t| t.started_at.clone()),
            queued,
            request_id: queue.as_ref().map(|q| q.id.clone()),
            last_error: queue.and_then(|q| q.error),
            updated_at: Utc::now().to_rfc3339(),
            last_event_at: workflow.updated_at,
            sequence: self.control_sequence.fetch_add(1, Ordering::SeqCst),
        })
    }
}
