use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionRecord {
    pub schema: String,
    pub task_id: String,
    pub revision: u64,
    pub workflow_version: String,
    pub policy: Value,
    pub question_mode: String,
    #[serde(default = "unspecified")]
    pub player_mode: String,
    #[serde(default)]
    pub question_groups: Vec<QuestionGroup>,
    #[serde(default)]
    pub artifacts: Vec<Value>,
    #[serde(default)]
    pub stage_updates: BTreeMap<String, Value>,
    #[serde(default)]
    pub events: Vec<Value>,
    pub task_template: Option<String>,
    pub art_template: Option<String>,
    pub current_stage: String,
    pub current_round: u8,
    pub cycle: u32,
    pub stages: BTreeMap<String, String>,
    pub documents: BTreeMap<String, Document>,
    pub approvals: Vec<Approval>,
    pub current_version: Option<String>,
    pub version_details: Value,
    pub issues: Vec<Value>,
    pub evidence: Vec<Evidence>,
    pub rounds: Vec<Value>,
    pub milestones: BTreeMap<String, Vec<String>>,
    pub checks: Vec<Value>,
    pub knowledge: Vec<Value>,
    pub skill_pins: Vec<SkillEntry>,
    pub legacy_record: Option<String>,
    pub updated_at: String,
}

fn unspecified() -> String {
    "unspecified".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntakeQuestion {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionGroup {
    #[serde(default = "requirements_stage")]
    pub stage: String,
    #[serde(default)]
    pub cycle: u32,
    pub id: String,
    pub title: String,
    pub provider: String,
    pub conversation_id: String,
    pub native_session_id: Option<String>,
    pub questions: Vec<IntakeQuestion>,
    pub draft: BTreeMap<String, String>,
    pub answers: Vec<AnswerRevision>,
    pub status: String,
    pub published_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerRevision {
    pub revision: u64,
    pub answers: BTreeMap<String, String>,
    pub submitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    #[serde(default)]
    pub template_choices: BTreeMap<String, String>,
    pub kind: String,
    pub revision: u64,
    pub path: String,
    pub sha256: String,
    pub content: String,
    pub game_version: Option<String>,
    pub submitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub kind: String,
    pub document_hash: String,
    pub document_revision: u64,
    pub decision: String,
    pub feedback: String,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    #[serde(default)]
    pub stage: String,
    pub cycle: u32,
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub kind: String,
    pub capture_type: String,
    pub version: String,
    pub reviewed: bool,
    pub description: String,
    pub instance_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub id: String,
    pub provider: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionView {
    pub stage_statuses: BTreeMap<String, String>,
    pub artifacts: Vec<Value>,
    pub record: Option<ProductionRecord>,
    pub policy: Value,
    pub warnings: Vec<String>,
    pub report_paths: Vec<String>,
    pub available_update: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductionMutation {
    pub task_id: String,
    pub expected_revision: u64,
    pub operation: String,
    #[serde(default = "empty_object")]
    pub data: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductionDecision {
    pub task_id: String,
    pub expected_revision: u64,
    pub kind: String,
    pub document_hash: String,
    pub accepted: bool,
    #[serde(default)]
    pub feedback: String,
}

fn empty_object() -> Value {
    serde_json::json!({})
}

fn requirements_stage() -> String {
    "requirements".into()
}
