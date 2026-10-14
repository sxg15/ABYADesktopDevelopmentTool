use super::*;
use crate::modules::development_terminal::{ConversationAttention, TerminalProvider};
use crate::modules::tasks::DevelopmentTask;

impl GrokTerminalService {
    pub fn attention_states(
        &self,
        task: &DevelopmentTask,
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
            let metadata: GrokConversation = serde_json::from_slice(&std::fs::read(path)?)?;
            if metadata.task_id != task.id || metadata.id != entry.file_name().to_string_lossy() {
                continue;
            }
            let workflow = {
                let _read = self
                    .workflow_write_lock
                    .try_lock()
                    .ok_or_else(|| AppError::validation("Conversation state is changing."))?;
                load_workflow(&entry.path(), &task.id, &metadata.id)?
            };
            // ACP does not currently expose native approval/user-input waits.
            // Never infer these from PTY text, tool names or elapsed time.
            let state = attention_state(&workflow);
            result.push(ConversationAttention {
                provider: TerminalProvider::Grok,
                task_id: task.id.clone(),
                conversation_id: metadata.id,
                state: state.into(),
                turn_id: workflow.current_turn_id,
                request_id: None,
                interaction_id: None,
            });
        }
        result.sort_by(|a, b| a.conversation_id.cmp(&b.conversation_id));
        Ok(result)
    }
}

fn attention_state(workflow: &CodexWorkflowSnapshot) -> &'static str {
    if workflow.observability != CodexObservabilityStatus::Native {
        return "unknown";
    }
    let turn = workflow
        .current_turn_id
        .as_ref()
        .and_then(|id| workflow.turns.iter().find(|t| &t.id == id));
    let fresh = chrono::DateTime::parse_from_rfc3339(&workflow.updated_at)
        .is_ok_and(|at| (Utc::now() - at.with_timezone(&Utc)).num_seconds() < 15);
    // A durable explicit failure remains actionable until a newer turn replaces it.
    match turn.map(|t| t.status) {
        Some(CodexWorkflowTurnStatus::Failed) => "failed",
        Some(CodexWorkflowTurnStatus::Interrupted) => "paused",
        Some(CodexWorkflowTurnStatus::Completed) => "ready",
        Some(CodexWorkflowTurnStatus::InProgress) if fresh => "running",
        Some(CodexWorkflowTurnStatus::WaitingForPlan) if fresh => "waitingForResponse",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn age_does_not_resolve_an_explicit_failure_or_invent_a_wait() {
        let mut workflow =
            CodexWorkflowSnapshot::empty("task", "c", CodexObservabilityStatus::Native, "");
        workflow.begin_turn("turn");
        workflow.updated_at = "2020-01-01T00:00:00Z".into();
        workflow.turns[0].status = CodexWorkflowTurnStatus::Failed;
        assert_eq!(attention_state(&workflow), "failed");
        workflow.turns[0].status = CodexWorkflowTurnStatus::InProgress;
        assert_eq!(attention_state(&workflow), "unknown");
        workflow.terminal_interaction = true;
        assert_eq!(attention_state(&workflow), "unknown");
        workflow.observability = CodexObservabilityStatus::Compatibility;
        workflow.turns[0].status = CodexWorkflowTurnStatus::Failed;
        assert_eq!(attention_state(&workflow), "unknown");
    }
}
