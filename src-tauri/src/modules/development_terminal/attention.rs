use super::TerminalProvider;
use serde::{Deserialize, Serialize};

/// Sanitized provider state for task attention. It contains no request text,
/// command, credential or transcript and does not connect/resume a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationAttention {
    pub provider: TerminalProvider,
    pub task_id: String,
    pub conversation_id: String,
    pub state: String,
    pub turn_id: Option<String>,
    pub request_id: Option<String>,
    pub interaction_id: Option<String>,
}

impl ConversationAttention {
    pub fn is_running(&self) -> bool {
        matches!(
            self.state.as_str(),
            "running" | "waitingForResponse" | "queued"
        )
    }
}
