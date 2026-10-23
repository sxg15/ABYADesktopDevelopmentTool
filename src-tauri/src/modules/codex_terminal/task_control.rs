use super::continuation_queue;
use super::*;
use crate::modules::development_terminal::{ConversationAttention, TerminalProvider};
use crate::modules::tasks::{DevelopmentTask, ProductionMutation, ProductionView};
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
    pub autonomy_notice: Option<String>,
    pub generation: u64,
    pub active_goal_id: Option<String>,
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
    pub execution_control: Option<super::execution_control::ExecutionControl>,
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
        let task = self.tasks.get_readonly(task_id)?;
        let metadata = self.conversation(task_id, conversation_id)?;
        let workflow = self.workflow(task_id, conversation_id)?;
        let production = self.tasks.production_attention(&task)?;
        self.project_control(&task, &metadata, &workflow, &production, true)
    }

    /// Uses only local metadata and the caller's read-only production snapshot.
    /// In particular this does not call list_conversations/native synchronization.
    pub fn attention_states(
        &self,
        task: &DevelopmentTask,
        production: &ProductionView,
    ) -> AppResult<Vec<ConversationAttention>> {
        let root = conversations_root(Path::new(&task.workspace_path));
        if task.workspace_path.is_empty() || !root.is_dir() {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir()
                || Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err()
            {
                continue;
            }
            let path = entry.path().join(CONVERSATION_METADATA_FILE);
            if !path.is_file() {
                continue;
            }
            let metadata: CodexConversation = serde_json::from_slice(&std::fs::read(path)?)?;
            if metadata.task_id != task.id
                || metadata.id != entry.file_name().to_string_lossy()
                || metadata.archived
            {
                continue;
            }
            let workflow = {
                let _read = self
                    .workflow_write_lock
                    .try_lock()
                    .ok_or_else(|| AppError::validation("Conversation state is changing."))?;
                load_workflow(&entry.path(), &task.id, &metadata.id)?
            };
            let control = self.project_control(task, &metadata, &workflow, production, false)?;
            result.push(ConversationAttention {
                provider: TerminalProvider::Codex,
                task_id: task.id.clone(),
                conversation_id: metadata.id,
                state: control.state,
                turn_id: control.turn_id,
                request_id: control.request_id,
                interaction_id: workflow.interaction_id,
            });
        }
        result.sort_by(|a, b| a.conversation_id.cmp(&b.conversation_id));
        Ok(result)
    }

    fn project_control(
        &self,
        task: &DevelopmentTask,
        metadata: &CodexConversation,
        workflow: &CodexWorkflowSnapshot,
        production: &ProductionView,
        track_queue: bool,
    ) -> AppResult<TaskControlState> {
        let task_id = task.id.as_str();
        let conversation_id = metadata.id.as_str();
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let queue = continuation_queue::read(&directory)?;
        let mut execution_control = super::execution_control::read(&directory)?;
        if let Some(control) = &mut execution_control
            && control.status == "stopping"
            && control.owner != self.process_epoch
        {
            control.status = "failed".into();
            control.error = Some("上次停止过程未完成确认，请核对原会话。".into());
        }
        if track_queue && queue.is_some() {
            self.queue_tasks
                .lock()
                .insert(conversation_id.into(), task_id.into());
        }
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
            .runtime_handle(conversation_id)
            .is_some_and(|s| s.is_alive());
        let connected = self.sessions.lock().contains_key(conversation_id);
        let same_executor = execution_control.as_ref().is_some_and(|control| {
            control.native_id == metadata.native_session_id
                && (!runtime
                    || (control.owner == self.process_epoch
                        && control.backend_pid
                            == self
                                .runtime_handle(conversation_id)
                                .as_ref()
                                .and_then(AppServerHandle::pid)))
        });
        let queued = queue.as_ref().is_some_and(|q| {
            q.status == "queued"
                && q.owner == self.process_epoch
                && !self.cancelled.lock().contains(conversation_id)
        });
        let attempt = production.record.as_ref().and_then(|r| {
            r.workbench
                .attempts
                .iter()
                .rev()
                .find(|a| a.conversation_id == conversation_id)
        });
        let state = if self.stopping.lock().contains(conversation_id) {
            "pausing"
        } else if same_executor
            && execution_control
                .as_ref()
                .is_some_and(|c| c.status == "failed")
        {
            "stopFailed"
        } else if !runtime
            && same_executor
            && execution_control
                .as_ref()
                .is_some_and(|c| c.status == "stopped")
        {
            "paused"
        } else if runtime && workflow.terminal_interaction {
            "waitingForTerminal"
        } else if runtime && active {
            if queued {
                "queued"
            } else if turn.is_some_and(|t| t.activities.is_empty()) {
                "waitingForResponse"
            } else {
                "running"
            }
        } else if production.record.as_ref().is_some_and(|r| {
            crate::modules::tasks::autonomous_implementation(r)
                && matches!(
                    r.workbench.execution.state.as_str(),
                    "waitingDependencies" | "settledWithGaps" | "settled"
                )
        }) {
            "workSettled"
        } else if task.status != crate::modules::tasks::TaskStatus::Active {
            "closed"
        } else if !runtime && attempt.is_some_and(|a| a.status == "interrupted") {
            "interrupted"
        } else if (!runtime
            && attempt.is_some_and(|a| a.status == "reconciling" || a.status == "needsReview"))
            || queue
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
            if connected && runtime {
                "awaitingContinue"
            } else {
                "disconnected"
            }
        } else if let Some(wait) = continuation_queue::waiting(Some(production)) {
            if wait == super::intake_receipt::IntakeContinueStatus::WaitingForAnswers {
                "waitingForAnswers"
            } else {
                "waitingForApproval"
            }
        } else if connected
            && runtime
            && (queue.as_ref().is_some_and(|q| q.status == "interrupted")
                || (same_executor
                    && execution_control
                        .as_ref()
                        .is_some_and(|c| c.status == "interrupted")))
        {
            "awaitingInput"
        } else if production.record.as_ref().is_some_and(|r| {
            production
                .stage_statuses
                .get(&r.current_stage)
                .is_some_and(|s| s == "blocked")
        }) {
            "blocked"
        } else if queue.as_ref().is_some_and(|q| q.status == "needsResume") {
            "failed"
        } else if active && !runtime {
            "disconnected"
        } else if turn.is_some_and(|t| t.status == CodexWorkflowTurnStatus::Interrupted) {
            if connected && runtime {
                "awaitingInput"
            } else {
                "paused"
            }
        } else if turn.is_some_and(|t| t.status == CodexWorkflowTurnStatus::Failed) {
            "failed"
        } else if turn.is_none() {
            "notStarted"
        } else {
            "ready"
        };
        let settings = super::settings::read(&directory).unwrap_or_default();
        let autonomy_notice=production.record.as_ref().filter(|r|r.policy["implementationExecution"]["mode"]=="autonomous"&&["plan","implementation"].contains(&r.current_stage.as_str()))
            .filter(|_|settings["sandboxPolicy"]["type"]=="readOnly"||settings["approvalsReviewer"]=="user"&&settings["approvalPolicy"]!="never")
            .map(|_|"当前原生权限为只读或人工逐项审核，可能暂停等待命令确认。可在终端执行设置选择 AI 自动审核；APP 不会代你改变权限。".into());
        Ok(TaskControlState {
            autonomy_notice,
            generation: self.gate(conversation_id).epoch(),
            active_goal_id: production
                .record
                .as_ref()
                .and_then(|r| r.workbench.active_goal_id.clone()),
            task_id: task_id.into(),
            conversation_id: conversation_id.into(),
            native_session_id: metadata.native_session_id.clone(),
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
            last_error: execution_control
                .as_ref()
                .filter(|_| same_executor)
                .and_then(|c| c.error.clone())
                .or_else(|| queue.and_then(|q| q.error)),
            execution_control,
            updated_at: Utc::now().to_rfc3339(),
            last_event_at: workflow.updated_at.clone(),
            sequence: if track_queue {
                self.control_sequence.fetch_add(1, Ordering::SeqCst)
            } else {
                0
            },
        })
    }
}
