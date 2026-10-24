use super::*;

pub(super) fn limit(r: &ProductionRecord, key: &str, default: i64) -> i64 {
    r.policy["implementationExecution"][key]
        .as_i64()
        .unwrap_or(default)
        .clamp(1, 120)
}
fn running_ms(u: &autonomy::WorkUnit) -> i64 {
    chrono::DateTime::parse_from_rfc3339(&u.attempt_started_at)
        .ok()
        .map(|at| {
            Utc::now()
                .signed_duration_since(at)
                .num_milliseconds()
                .max(0)
        })
        .unwrap_or(0)
}
fn keys(r: &ProductionRecord, u: &autonomy::WorkUnit) -> Vec<String> {
    // A new unit ID cannot buy another budget for the same issue/build/scope.
    u.issue_ids
        .iter()
        .filter(|id| {
            r.issues
                .iter()
                .any(|i| i["id"] == **id && i["status"] != "resolved")
        })
        .map(|id| {
            format!(
                "{}:{}:{}",
                u.scope_hash,
                r.version_details["playerBuildHash"]
                    .as_str()
                    .unwrap_or("unknown"),
                id
            )
        })
        .collect()
}
pub(super) fn exhausted(r: &ProductionRecord, u: &autonomy::WorkUnit) -> bool {
    let extra = running_ms(u);
    u.elapsed_ms + extra >= limit(r, "workMinutes", 60) * 60_000
        || keys(r, u).iter().any(|k| {
            r.workbench
                .execution
                .recovery_ms
                .get(k)
                .copied()
                .unwrap_or(0)
                + extra
                >= limit(r, "recoveryMinutes", 30) * 60_000
        })
}
pub(super) fn account(r: &mut ProductionRecord) {
    let Some(u) = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| Some(&u.id) == r.workbench.execution.active_unit_id.as_ref())
        .cloned()
    else {
        return;
    };
    let elapsed = running_ms(&u);
    for k in keys(r, &u) {
        *r.workbench.execution.recovery_ms.entry(k).or_default() += elapsed;
    }
    if let Some(unit) = r
        .workbench
        .execution
        .units
        .iter_mut()
        .find(|v| v.id == u.id)
    {
        unit.elapsed_ms += elapsed;
        unit.route_elapsed_ms += elapsed;
        unit.attempt_started_at.clear();
    }
}
impl TaskService {
    pub fn production_execution_observation_gap(&self, task: &str) -> AppResult<()> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(());
        };
        if !autonomy::autonomous_implementation(&r)
            || !matches!(r.workbench.execution.state.as_str(), "running" | "ready")
        {
            return Ok(());
        }
        self.production_commit(task, r.revision, |r| {
            for u in &mut r.workbench.execution.units {
                u.attempt_started_at.clear();
            }
            r.workbench.execution.state = "paused".into();
            autonomy::event(
                &mut r.workbench.execution,
                "observation-gap",
                None,
                "执行器已退出，未观测区间不计为执行；恢复沿用原工作项。",
            );
            Ok(())
        })
    }
    pub fn production_execution_budget_due(&self, task: &str) -> AppResult<bool> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(false);
        };
        if !autonomy::autonomous_implementation(&r)
            || !matches!(r.workbench.execution.state.as_str(), "running" | "ready")
        {
            return Ok(false);
        }
        // Reporting a unit early must not allow the same native turn to run forever.
        // This clock spans the dispatch, while operation accounting stops at the report.
        let dispatch_expired = r.workbench.execution.active_unit_id.is_some()
            && r.workbench
                .execution
                .events
                .iter()
                .rev()
                .find(|e| matches!(e["kind"].as_str(), Some("scheduled" | "resumed")))
                .and_then(|e| e["at"].as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_some_and(|at| {
                    Utc::now().signed_duration_since(at).num_milliseconds()
                        >= limit(&r, "turnMinutes", 15) * 60_000
                });
        if dispatch_expired {
            return Ok(true);
        }
        Ok(r.workbench.execution.units.iter().any(|u| {
            u.status == "running"
                && Some(&u.id) == r.workbench.execution.active_unit_id.as_ref()
                && (running_ms(u) >= limit(&r, "turnMinutes", 15) * 60_000
                    || u.route_elapsed_ms + running_ms(u) >= limit(&r, "routeMinutes", 15) * 60_000
                    || exhausted(&r, u))
        }))
    }
}
