use super::*;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IntakeContinueStatus {
    Queued,
    Paused,
    WaitingForAnswers,
    WaitingForApproval,
    NeedsReview,
    NeedsConnection,
    Started,
    Resumed,
    Running,
    Completed,
    Superseded,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> Value {
        json!({"status":"sent","nativeSessionId":"native","turnId":"one","messageId":"message"})
    }
    fn state(status: &str) -> Value {
        json!({"thread":{"id":"native","turns":[{"id":"one","status":status}]}})
    }

    #[test]
    fn interrupted_and_failed_attempts_resume_but_running_and_completed_do_not_repeat() {
        for status in ["interrupted", "failed"] {
            assert_eq!(
                decide(Some(&receipt()), "native", Some(&state(status))).unwrap(),
                ReceiptDecision::Resume
            );
        }
        assert_eq!(
            decide(Some(&receipt()), "native", Some(&state("inProgress"))).unwrap(),
            ReceiptDecision::Existing(IntakeContinueStatus::Running)
        );
        assert_eq!(
            decide(Some(&receipt()), "native", Some(&state("completed"))).unwrap(),
            ReceiptDecision::Existing(IntakeContinueStatus::Completed)
        );
        assert_eq!(
            decide(None, "native", None).unwrap(),
            ReceiptDecision::Start
        );
    }

    #[test]
    fn later_manual_turn_supersedes_old_interrupted_receipt() {
        let mut snapshot = state("interrupted");
        snapshot["thread"]["turns"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"manual","status":"completed"}));
        assert_eq!(
            decide(Some(&receipt()), "native", Some(&snapshot)).unwrap(),
            ReceiptDecision::Existing(IntakeContinueStatus::Superseded)
        );
    }

    #[test]
    fn uncertain_or_foreign_history_never_authorizes_a_retry() {
        let mut pending = receipt();
        pending["status"] = json!("pending");
        assert!(decide(Some(&pending), "native", Some(&state("interrupted"))).is_err());
        assert!(decide(Some(&receipt()), "different", Some(&state("interrupted"))).is_err());
        assert!(
            decide(
                Some(&receipt()),
                "native",
                Some(&json!({"thread":{"id":"native","turns":[]}}))
            )
            .is_err()
        );
        assert!(decide(Some(&receipt()), "native", Some(&state("unknown"))).is_err());
    }

    #[test]
    fn new_attempt_keeps_previous_receipt_and_persists_pending_before_delivery() {
        let root = std::env::temp_dir().join(format!("abya-intake-receipt-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("receipt.json");
        let old = receipt();
        save(&path, &old, true).unwrap();
        let next = start_attempt(&path, Some(&old), "native").unwrap();
        assert_eq!(next["attempt"], 2);
        assert_ne!(next["messageId"], old["messageId"]);
        assert_eq!(next["status"], "pending");
        assert!(decide(Some(&next), "native", Some(&state("interrupted"))).is_err());
        let backup = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .find(|e| e.path() != path)
            .unwrap();
        let preserved: Value =
            serde_json::from_slice(&std::fs::read(backup.path()).unwrap()).unwrap();
        assert_eq!(preserved, old);
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeContinuation {
    pub request_id: Option<String>,
    pub status: IntakeContinueStatus,
    pub conversation_id: String,
    pub native_session_id: String,
    pub turn_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReceiptDecision {
    Start,
    Resume,
    Existing(IntakeContinueStatus),
}

pub(super) fn decide(
    saved: Option<&Value>,
    native: &str,
    snapshot: Option<&Value>,
) -> AppResult<ReceiptDecision> {
    let Some(saved) = saved else {
        return Ok(ReceiptDecision::Start);
    };
    if saved["nativeSessionId"].as_str() != Some(native) || saved["status"] != "sent" {
        return Err(unknown());
    }
    let turn_id = saved["turnId"].as_str().ok_or_else(unknown)?;
    let thread = snapshot.and_then(|s| s.get("thread")).ok_or_else(unknown)?;
    if thread["id"].as_str() != Some(native) {
        return Err(unknown());
    }
    let turns = thread["turns"].as_array().ok_or_else(unknown)?;
    let index = turns
        .iter()
        .position(|t| t["id"] == turn_id)
        .ok_or_else(unknown)?;
    // A later manual continuation may already have processed these answers.
    if index + 1 < turns.len() {
        return Ok(ReceiptDecision::Existing(IntakeContinueStatus::Superseded));
    }
    match turns[index]["status"].as_str() {
        Some("inProgress") => Ok(ReceiptDecision::Existing(IntakeContinueStatus::Running)),
        Some("completed") => Ok(ReceiptDecision::Existing(IntakeContinueStatus::Completed)),
        Some("interrupted" | "failed") => Ok(ReceiptDecision::Resume),
        _ => Err(unknown()),
    }
}

fn unknown() -> AppError {
    AppError::new(
        "intake_outcome_unknown",
        "上次继续请求的结果尚不能确认。请核对原生历史；未自动重发。",
        "",
    )
}

pub(super) fn save(path: &Path, value: &Value, new: bool) -> AppResult<()> {
    let mut options = OpenOptions::new();
    options.write(true);
    if new {
        options.create_new(true);
    } else {
        options.truncate(true);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    Ok(())
}

pub(super) fn start_attempt(
    path: &Path,
    previous: Option<&Value>,
    native: &str,
) -> AppResult<Value> {
    if let Some(previous) = previous {
        let backup = path.with_extension(format!("attempt-{}.json", Uuid::new_v4()));
        save(&backup, previous, true)?;
    }
    let next = json!({"status":"pending","messageId":Uuid::new_v4().to_string(),"nativeSessionId":native,
        "attempt":previous.map_or(1,|v|v["attempt"].as_u64().unwrap_or(1)+1),"at":Utc::now().to_rfc3339()});
    // Persist before RPC. Any ambiguous outcome stays pending and cannot be blindly replayed.
    save(path, &next, previous.is_none())?;
    Ok(next)
}
