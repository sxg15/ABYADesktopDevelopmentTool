use crate::foundation::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

pub(crate) const WORKFLOW_SNAPSHOT_FILE: &str = "workflow.json";
const WORKFLOW_EVENTS_FILE: &str = "workflow-events.jsonl";
const MAX_WORKFLOW_EVENTS_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CONVERSATION_TITLE_CHARS: usize = 100;
const MAX_ACTIVITY_SUMMARY_CHARS: usize = 240;
const MAX_ACTIVITY_DETAIL_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexObservabilityStatus {
    Native,
    Compatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexWorkflowTurnStatus {
    WaitingForPlan,
    InProgress,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexPlanStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Unplanned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexActivityStatus {
    Started,
    Progress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexActivityKind {
    Analysis,
    Command,
    FileChange,
    Mcp,
    GameInstance,
    Test,
    Web,
    Agent,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexPlanStep {
    pub id: String,
    pub step: String,
    pub status: CodexPlanStepStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexActivity {
    pub id: String,
    pub step_id: Option<String>,
    pub kind: CodexActivityKind,
    pub status: CodexActivityStatus,
    pub summary: String,
    pub detail: String,
    pub source: String,
    pub unplanned: bool,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexWorkflowTurn {
    pub id: String,
    pub status: CodexWorkflowTurnStatus,
    pub explanation: String,
    pub plan: Vec<CodexPlanStep>,
    pub activities: Vec<CodexActivity>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexWorkflowSnapshot {
    pub task_id: String,
    pub conversation_id: String,
    pub observability: CodexObservabilityStatus,
    pub warning: String,
    pub current_turn_id: Option<String>,
    pub turns: Vec<CodexWorkflowTurn>,
    pub updated_at: String,
}

impl CodexWorkflowSnapshot {
    pub(crate) fn empty(
        task_id: &str,
        conversation_id: &str,
        observability: CodexObservabilityStatus,
        warning: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.to_string(),
            conversation_id: conversation_id.to_string(),
            observability,
            warning: warning.into(),
            current_turn_id: None,
            turns: Vec::new(),
            updated_at: Utc::now().to_rfc3339(),
        }
    }

    pub(crate) fn begin_turn(&mut self, turn_id: &str) {
        if self.turns.iter().any(|turn| turn.id == turn_id) {
            self.current_turn_id = Some(turn_id.to_string());
            return;
        }
        self.turns.push(CodexWorkflowTurn {
            id: turn_id.to_string(),
            status: CodexWorkflowTurnStatus::WaitingForPlan,
            explanation: String::new(),
            plan: Vec::new(),
            activities: Vec::new(),
            started_at: Utc::now().to_rfc3339(),
            completed_at: None,
        });
        self.current_turn_id = Some(turn_id.to_string());
        self.touch();
    }

    pub(crate) fn update_plan(
        &mut self,
        turn_id: &str,
        explanation: Option<&str>,
        steps: &[(String, CodexPlanStepStatus)],
    ) {
        self.begin_turn(turn_id);
        let turn = self.current_turn_mut(turn_id);
        let previous = turn.plan.clone();
        turn.plan = steps
            .iter()
            .enumerate()
            .map(|(index, (step, status))| {
                let id = previous
                    .iter()
                    .find(|candidate| candidate.step == *step)
                    .or_else(|| previous.get(index))
                    .map(|candidate| candidate.id.clone())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                CodexPlanStep {
                    id,
                    step: bounded(step, MAX_ACTIVITY_SUMMARY_CHARS),
                    status: *status,
                }
            })
            .collect();
        turn.explanation = explanation
            .map(|value| bounded(value, MAX_ACTIVITY_DETAIL_CHARS))
            .unwrap_or_default();
        turn.status = if turn
            .plan
            .iter()
            .all(|step| step.status == CodexPlanStepStatus::Completed)
            && !turn.plan.is_empty()
        {
            CodexWorkflowTurnStatus::Completed
        } else {
            CodexWorkflowTurnStatus::InProgress
        };
        self.touch();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_activity(
        &mut self,
        turn_id: &str,
        activity_id: Option<&str>,
        kind: CodexActivityKind,
        status: CodexActivityStatus,
        summary: &str,
        detail: &str,
        source: &str,
    ) {
        self.begin_turn(turn_id);
        let turn = self.current_turn_mut(turn_id);
        let step_id = turn
            .plan
            .iter()
            .find(|step| step.status == CodexPlanStepStatus::InProgress)
            .map(|step| step.id.clone());
        let unplanned = step_id.is_none();
        if unplanned
            && !turn
                .plan
                .iter()
                .any(|step| step.status == CodexPlanStepStatus::Unplanned)
        {
            turn.plan.push(CodexPlanStep {
                id: format!("unplanned-{turn_id}"),
                step: "Unplanned operation".into(),
                status: CodexPlanStepStatus::Unplanned,
            });
        }
        let step_id = step_id.or_else(|| Some(format!("unplanned-{turn_id}")));
        let id = activity_id
            .map(str::to_string)
            .or_else(|| {
                turn.activities
                    .iter()
                    .rev()
                    .find(|activity| {
                        activity.source == source
                            && activity.summary == summary
                            && !matches!(
                                activity.status,
                                CodexActivityStatus::Completed | CodexActivityStatus::Failed
                            )
                    })
                    .map(|activity| activity.id.clone())
            })
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now().to_rfc3339();
        if let Some(existing) = turn.activities.iter_mut().find(|item| item.id == id) {
            existing.kind = kind;
            existing.status = status;
            existing.summary = bounded(summary, MAX_ACTIVITY_SUMMARY_CHARS);
            existing.detail = bounded(detail, MAX_ACTIVITY_DETAIL_CHARS);
            existing.source = source.to_string();
            if matches!(
                status,
                CodexActivityStatus::Completed | CodexActivityStatus::Failed
            ) {
                existing.completed_at = Some(now);
            }
        } else {
            turn.activities.push(CodexActivity {
                id,
                step_id,
                kind,
                status,
                summary: bounded(summary, MAX_ACTIVITY_SUMMARY_CHARS),
                detail: bounded(detail, MAX_ACTIVITY_DETAIL_CHARS),
                source: source.to_string(),
                unplanned,
                started_at: now.clone(),
                completed_at: matches!(
                    status,
                    CodexActivityStatus::Completed | CodexActivityStatus::Failed
                )
                .then_some(now),
            });
        }
        if turn.status == CodexWorkflowTurnStatus::WaitingForPlan {
            turn.status = CodexWorkflowTurnStatus::InProgress;
        }
        self.touch();
    }

    pub(crate) fn complete_turn(&mut self, turn_id: &str, status: CodexWorkflowTurnStatus) {
        self.begin_turn(turn_id);
        let turn = self.current_turn_mut(turn_id);
        turn.status = status;
        turn.completed_at = Some(Utc::now().to_rfc3339());
        self.touch();
    }

    fn current_turn_mut(&mut self, turn_id: &str) -> &mut CodexWorkflowTurn {
        self.turns
            .iter_mut()
            .find(|turn| turn.id == turn_id)
            .expect("turn was created before access")
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now().to_rfc3339();
    }
}

pub(crate) fn validate_conversation_title(title: &str) -> AppResult<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::validation("Conversation title is required."));
    }
    if title.chars().count() > MAX_CONVERSATION_TITLE_CHARS {
        return Err(AppError::validation(
            "Conversation title must be 100 characters or fewer.",
        ));
    }
    Ok(title.to_string())
}

pub(crate) fn load_workflow(
    directory: &Path,
    task_id: &str,
    conversation_id: &str,
) -> AppResult<CodexWorkflowSnapshot> {
    let path = directory.join(WORKFLOW_SNAPSHOT_FILE);
    if !path.is_file() {
        return Ok(CodexWorkflowSnapshot::empty(
            task_id,
            conversation_id,
            CodexObservabilityStatus::Compatibility,
            "This conversation has no native Codex activity stream.",
        ));
    }
    let workflow: CodexWorkflowSnapshot = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    if workflow.task_id != task_id || workflow.conversation_id != conversation_id {
        return Err(AppError::validation(
            "Codex workflow metadata does not match the conversation.",
        ));
    }
    Ok(workflow)
}

pub(crate) fn save_workflow(
    directory: &Path,
    workflow: &CodexWorkflowSnapshot,
    event_type: &str,
) -> AppResult<()> {
    std::fs::create_dir_all(directory)?;
    let target = directory.join(WORKFLOW_SNAPSHOT_FILE);
    let temporary = directory.join(format!(".{WORKFLOW_SNAPSHOT_FILE}.{}.tmp", Uuid::new_v4()));
    std::fs::write(&temporary, serde_json::to_vec_pretty(workflow)?)?;
    if let Err(error) = std::fs::rename(&temporary, &target) {
        if target.is_file() {
            std::fs::remove_file(&target)?;
            std::fs::rename(&temporary, &target)?;
        } else {
            let _ = std::fs::remove_file(&temporary);
            return Err(error.into());
        }
    }

    let events_path = directory.join(WORKFLOW_EVENTS_FILE);
    let current_turn_status = workflow.current_turn_id.as_deref().and_then(|turn_id| {
        workflow
            .turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .map(|turn| turn.status)
    });
    let event = json!({
        "schemaVersion": 2,
        "id": Uuid::new_v4(),
        "type": event_type,
        "timestamp": Utc::now().to_rfc3339(),
        "workflowUpdatedAt": workflow.updated_at,
        "currentTurnId": workflow.current_turn_id,
        "currentTurnStatus": current_turn_status,
        "turnCount": workflow.turns.len()
    });
    let mut encoded_event = serde_json::to_vec(&event)?;
    encoded_event.push(b'\n');
    compact_workflow_events_if_needed(&events_path, encoded_event.len() as u64)?;
    let mut events = OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path)?;
    events.write_all(&encoded_event)?;
    events.flush()?;
    Ok(())
}

fn compact_workflow_events_if_needed(path: &Path, incoming_bytes: u64) -> AppResult<()> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len().saturating_add(incoming_bytes) <= MAX_WORKFLOW_EVENTS_BYTES {
        return Ok(());
    }

    // Legacy records embedded the complete, ever-growing workflow projection in
    // every line. Once the journal reaches its retention bound, replace that
    // redundant history with a marker; workflow.json remains the authoritative
    // projection and the following save appends the current compact revision.
    let marker = json!({
        "schemaVersion": 2,
        "id": Uuid::new_v4(),
        "type": "historyCompacted",
        "timestamp": Utc::now().to_rfc3339(),
        "previousBytes": metadata.len()
    });
    let mut contents = serde_json::to_vec(&marker)?;
    contents.push(b'\n');
    let temporary = path.with_file_name(format!(".{WORKFLOW_EVENTS_FILE}.{}.tmp", Uuid::new_v4()));
    std::fs::write(&temporary, contents)?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        if path.is_file() {
            std::fs::remove_file(path)?;
            std::fs::rename(&temporary, path)?;
        } else {
            let _ = std::fs::remove_file(&temporary);
            return Err(error.into());
        }
    }
    Ok(())
}

pub(crate) fn sanitize_detail(value: &Value) -> String {
    fn redact(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, value)| {
                        let key_lower = key.to_ascii_lowercase();
                        let value = if ["token", "secret", "password", "authorization", "api_key"]
                            .iter()
                            .any(|needle| key_lower.contains(needle))
                        {
                            Value::String("[REDACTED]".into())
                        } else {
                            redact(value)
                        };
                        (key.clone(), value)
                    })
                    .collect(),
            ),
            Value::Array(values) => Value::Array(values.iter().map(redact).collect()),
            other => other.clone(),
        }
    }
    bounded(&redact(value).to_string(), MAX_ACTIVITY_DETAIL_CHARS)
}

fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_long_titles() {
        assert!(validate_conversation_title("  ").is_err());
        assert!(validate_conversation_title(&"x".repeat(101)).is_err());
        assert_eq!(validate_conversation_title(" Review ").unwrap(), "Review");
    }

    #[test]
    fn records_unplanned_activity_without_blocking() {
        let mut workflow = CodexWorkflowSnapshot::empty(
            "task",
            "conversation",
            CodexObservabilityStatus::Native,
            "",
        );
        workflow.record_activity(
            "turn",
            Some("activity"),
            CodexActivityKind::Command,
            CodexActivityStatus::Started,
            "Inspect files",
            "",
            "codex",
        );
        assert_eq!(
            workflow.turns[0].plan[0].status,
            CodexPlanStepStatus::Unplanned
        );
        assert!(workflow.turns[0].activities[0].unplanned);
    }

    #[test]
    fn reuses_step_ids_across_plan_updates() {
        let mut workflow = CodexWorkflowSnapshot::empty(
            "task",
            "conversation",
            CodexObservabilityStatus::Native,
            "",
        );
        workflow.update_plan(
            "turn",
            None,
            &[
                ("Inspect".into(), CodexPlanStepStatus::InProgress),
                ("Test".into(), CodexPlanStepStatus::Pending),
            ],
        );
        let id = workflow.turns[0].plan[0].id.clone();
        workflow.update_plan(
            "turn",
            None,
            &[
                ("Inspect".into(), CodexPlanStepStatus::Completed),
                ("Test".into(), CodexPlanStepStatus::InProgress),
            ],
        );
        assert_eq!(workflow.turns[0].plan[0].id, id);
    }

    #[test]
    fn redacts_secret_fields() {
        let detail = sanitize_detail(&json!({
            "instanceId": "safe",
            "authorization": "Bearer secret",
            "nested": { "api_key": "secret" }
        }));
        assert!(detail.contains("safe"));
        assert!(!detail.contains("Bearer secret"));
        assert!(!detail.contains("\"secret\""));
    }

    #[test]
    fn replaces_an_existing_snapshot() {
        let root = std::env::temp_dir().join(format!("abya-workflow-{}", Uuid::new_v4()));
        let mut workflow = CodexWorkflowSnapshot::empty(
            "task",
            "conversation",
            CodexObservabilityStatus::Native,
            "",
        );
        save_workflow(&root, &workflow, "created").unwrap();
        workflow.begin_turn("turn");
        save_workflow(&root, &workflow, "updated").unwrap();

        let loaded = load_workflow(&root, "task", "conversation").unwrap();
        assert_eq!(loaded.current_turn_id.as_deref(), Some("turn"));
        assert_eq!(
            std::fs::read_to_string(root.join(WORKFLOW_EVENTS_FILE))
                .unwrap()
                .lines()
                .count(),
            2
        );
        let events = std::fs::read_to_string(root.join(WORKFLOW_EVENTS_FILE)).unwrap();
        assert!(!events.contains("\"workflow\":"));
        assert!(events.lines().all(|line| {
            serde_json::from_str::<Value>(line)
                .unwrap()
                .get("schemaVersion")
                .and_then(Value::as_i64)
                == Some(2)
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compacts_an_oversized_legacy_event_journal() {
        let root = std::env::temp_dir().join(format!("abya-workflow-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let events_path = root.join(WORKFLOW_EVENTS_FILE);
        let events = std::fs::File::create(&events_path).unwrap();
        events.set_len(MAX_WORKFLOW_EVENTS_BYTES).unwrap();
        drop(events);

        let workflow = CodexWorkflowSnapshot::empty(
            "task",
            "conversation",
            CodexObservabilityStatus::Native,
            "",
        );
        save_workflow(&root, &workflow, "updated").unwrap();

        let contents = std::fs::read_to_string(&events_path).unwrap();
        assert!(contents.len() < 2_000);
        assert!(contents.len() as u64 <= MAX_WORKFLOW_EVENTS_BYTES);
        let types = contents
            .lines()
            .map(|line| {
                serde_json::from_str::<Value>(line).unwrap()["type"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(types, vec!["historyCompacted", "updated"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reported_activity_updates_the_matching_live_milestone() {
        let mut workflow = CodexWorkflowSnapshot::empty(
            "task",
            "conversation",
            CodexObservabilityStatus::Native,
            "",
        );
        workflow.record_activity(
            "turn",
            None,
            CodexActivityKind::Test,
            CodexActivityStatus::Started,
            "Run smoke test",
            "starting",
            "desktopMcpReport",
        );
        workflow.record_activity(
            "turn",
            None,
            CodexActivityKind::Test,
            CodexActivityStatus::Completed,
            "Run smoke test",
            "passed",
            "desktopMcpReport",
        );

        assert_eq!(workflow.turns[0].activities.len(), 1);
        assert_eq!(
            workflow.turns[0].activities[0].status,
            CodexActivityStatus::Completed
        );
    }
}
