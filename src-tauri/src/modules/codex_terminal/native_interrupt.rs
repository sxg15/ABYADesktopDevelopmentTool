use super::*;

pub(super) fn observe(
    turns: &parking_lot::Mutex<HashMap<String, (String, String)>>,
    event: &Value,
) {
    let method = event["method"].as_str().unwrap_or("");
    if !matches!(method, "turn/started" | "turn/completed") {
        return;
    }
    let p = &event["params"];
    if let (Some(thread), Some(id), Some(status)) = (
        p["threadId"].as_str(),
        p["turn"]["id"].as_str(),
        p["turn"]["status"].as_str(),
    ) {
        let mut turns = turns.lock();
        if method == "turn/started" || turns.get(thread).is_none_or(|t| t.0 == id) {
            turns.insert(thread.into(), (id.into(), status.into()));
        }
    }
}

#[cfg(test)]
#[path = "native_interrupt_tests.rs"]
mod tests;

fn uncertain() -> AppError {
    AppError::new(
        "codex_interrupt_uncertain",
        "无法确认当前轮次，未确认停止；可重试或强制停止此会话。",
        "",
    )
}

fn target(
    snapshot: &Value,
    thread: &str,
    observed: Option<(String, String)>,
) -> AppResult<Option<String>> {
    if snapshot["thread"]["id"].as_str() != Some(thread)
        || snapshot["thread"]["status"]["type"] == "notLoaded"
    {
        return Err(uncertain());
    }
    let turns = snapshot["thread"]["turns"]
        .as_array()
        .ok_or_else(uncertain)?;
    // Only the newest turn can be the current executor; old rollout statuses may be stale.
    let latest = turns.last();
    if let Some((id, status)) = observed
        && status == "inProgress"
    {
        let superseded = turns
            .iter()
            .position(|t| t["id"] == id)
            .is_some_and(|index| index + 1 < turns.len());
        let finished = turns.iter().any(|t| {
            t["id"] == id
                && matches!(
                    t["status"].as_str(),
                    Some("completed" | "interrupted" | "failed")
                )
        });
        if !finished && !superseded {
            return Ok(Some(id));
        }
    }
    match latest {
        Some(t) if t["status"] == "inProgress" => t["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(|id| Some(id.into()))
            .ok_or_else(uncertain),
        Some(t)
            if !matches!(
                t["status"].as_str(),
                Some("completed" | "interrupted" | "failed")
            ) =>
        {
            Err(uncertain())
        }
        _ if snapshot["thread"]["status"]["type"] == "active" => Err(uncertain()),
        _ => Ok(None),
    }
}

impl AppServerHandle {
    pub(in crate::modules::codex_terminal) fn interrupt_and_wait(
        &self,
        thread: &str,
    ) -> AppResult<()> {
        self.interrupt_observed(thread, |_, _| {})
    }

    pub(in crate::modules::codex_terminal) fn interrupt_observed(
        &self,
        thread: &str,
        mut record: impl FnMut(&str, &str),
    ) -> AppResult<()> {
        let deadline = Instant::now() + Duration::from_secs(20);
        let request = |method, params| {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AppError::new(
                    "codex_interrupt_timeout",
                    "停止确认超时；可重试或强制停止此会话。",
                    "",
                ));
            }
            self.request_with_timeout(method, params, remaining.min(Duration::from_secs(5)))
        };
        let mut attempted = None;
        let mut attempts = 0;
        let mut retried = false;
        loop {
            let snapshot = request(
                "thread/read",
                json!({"threadId":thread,"includeTurns":true}),
            )?;
            let active = target(
                &snapshot,
                thread,
                self.observed_turns.lock().get(thread).cloned(),
            )?;
            let Some(id) = active else {
                return Ok(());
            };
            if attempted.as_ref() != Some(&id) {
                if attempts >= 3 {
                    return Err(uncertain());
                }
                record("target-selected", &id);
                attempts += 1;
                match request("turn/interrupt", json!({"threadId":thread,"turnId":id})) {
                    Ok(_) => attempted = Some(id),
                    Err(e) if !retried && e.code == "codex_turn_changed" => {
                        retried = true;
                        record("target-refresh", &id);
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
