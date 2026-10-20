use crate::modules::development_terminal::TerminalProvider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionTarget {
    pub task_id: String,
    pub action: String,
    pub stage: String,
    pub provider: Option<TerminalProvider>,
    pub conversation_id: Option<String>,
    pub group_id: Option<String>,
    pub document_revision: Option<u64>,
    pub document_hash: Option<String>,
    pub feedback_id: Option<String>,
    pub candidate_artifact_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionText {
    pub title: String,
    pub instruction: String,
    pub detail: String,
    pub destination: String,
    pub button: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAttention {
    pub key: String,
    pub task_id: String,
    pub task_title: String,
    pub kind: String,
    pub tone: String,
    pub stage_name: String,
    pub target: AttentionTarget,
    pub zh: AttentionText,
    pub en: AttentionText,
}

impl TaskAttention {
    pub fn text(&self, english: bool) -> &AttentionText {
        if english { &self.en } else { &self.zh }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAttentionSnapshot {
    pub task_id: String,
    pub items: Vec<TaskAttention>,
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationActivation {
    pub id: String,
    pub target: Option<AttentionTarget>,
    /// current, resolved, missing or test. Activation never mutates a task.
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    pub enabled: bool,
    pub available: bool,
    pub reason: String,
    pub identity: String,
    pub test_environment: bool,
}
