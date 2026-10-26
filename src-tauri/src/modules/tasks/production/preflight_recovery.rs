use super::*;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PreflightRecovery {
    pub key: String,
    pub stage: String,
    pub goal_id: String,
    pub state: String,
    pub attempts: u32,
    pub elapsed_ms: i64,
    pub started_at: String,
    pub last_event: String,
}
fn key(r: &ProductionRecord) -> String {
    format!(
        "{}:{}:{}",
        r.cycle,
        r.current_stage,
        r.documents
            .get("requirements")
            .map(|d| d.sha256.as_str())
            .unwrap_or("")
    )
}
fn human_wait(r: &ProductionRecord) -> bool {
    r.question_groups.iter().any(|g| g.status == "pending")
        || r.stages.values().any(|s| s == "awaiting-confirmation")
        || r.feedback.iter().any(|f| f["status"] == "awaiting-recheck")
}
fn enabled(r: &ProductionRecord) -> bool {
    r.policy["preflightRecovery"]["enabled"] == true
        && matches!(r.current_stage.as_str(), "resources" | "plan")
        && !human_wait(r)
        && !r
            .stages
            .get(&r.current_stage)
            .is_some_and(|s| matches!(s.as_str(), "passed" | "stopped"))
}
pub fn preflight_recovery_manual(r: &ProductionRecord) -> bool {
    enabled(r)
}
pub fn preflight_recovery_active(r: &ProductionRecord) -> bool {
    enabled(r)
        && r.workbench.execution.preflight.key == key(r)
        && r.workbench.execution.preflight.state == "running"
}
pub fn preflight_recovery_resumable(r: &ProductionRecord) -> bool {
    enabled(r)
        && r.workbench.execution.preflight.key == key(r)
        && r.workbench.execution.preflight.state == "paused"
        && r.workbench.execution.preflight.elapsed_ms < 20 * 60_000
}
fn elapsed(p: &PreflightRecovery) -> i64 {
    chrono::DateTime::parse_from_rfc3339(&p.started_at)
        .ok()
        .map(|t| {
            Utc::now()
                .signed_duration_since(t)
                .num_milliseconds()
                .max(0)
        })
        .unwrap_or(0)
}
pub(super) fn pause(r: &mut ProductionRecord) {
    let p = &mut r.workbench.execution.preflight;
    p.elapsed_ms += elapsed(p);
    p.started_at.clear();
    p.state = "paused".into();
}
pub(super) fn due(r: &ProductionRecord) -> bool {
    preflight_recovery_active(r) && elapsed(&r.workbench.execution.preflight) >= 10 * 60_000
}
impl TaskService {
    /// Authorization comes from a native-observed desktopCli runtime failure or
    /// the explicit user resume path, never from semantic progress/issue prose.
    pub fn production_prepare_preflight_recovery(
        &self,
        task: &str,
        event_id: &str,
        runtime_failed: bool,
    ) -> AppResult<bool> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(false);
        };
        if !enabled(&r) {
            return Ok(false);
        }
        let root = self.production_workspace(task)?;
        validation::approved(&r, &root, "requirements")?;
        let blocked = r.issues.iter().any(|i| {
            i["status"] != "resolved"
                && i["kind"] != "suggestion"
                && workspace::affects(i, &r.current_stage)
        });
        let p = &r.workbench.execution.preflight;
        if p.key == key(&r) && p.last_event == event_id {
            return Ok(p.state == "running");
        }
        if !blocked {
            if preflight_recovery_active(&r) {
                self.production_commit(task, r.revision, |r| {
                    let p = &mut r.workbench.execution.preflight;
                    p.elapsed_ms += elapsed(p);
                    p.started_at.clear();
                    p.state = "recovered".into();
                    p.last_event = event_id.into();
                    r.stage_updates.insert(r.current_stage.clone(),json!({"summary":"运行预检问题已恢复，当前阶段的设计与其他验证继续进行；历史失败记录保留。","nextAction":"继续原目标的剩余预检，按原流程提交文档审核。","updatedAt":now()}));
                    if r.stages
                        .get(&r.current_stage)
                        .is_some_and(|s| s == "blocked")
                    {
                        r.stages
                            .insert(r.current_stage.clone(), "in-progress".into());
                    }
                    Ok(())
                })?;
            }
            return Ok(false);
        }
        if !runtime_failed && p.key != key(&r) {
            return Ok(false);
        }
        self.production_commit(task, r.revision, |r| {
            if r.workbench.goals.is_empty() {
                super::workbench::observe(r, None);
            }
            let k = key(r);
            let stage = r.current_stage.clone();
            let goal = r.workbench.active_goal_id.clone().unwrap_or_default();
            let p = &mut r.workbench.execution.preflight;
            if p.key != k {
                *p = PreflightRecovery {
                    key: k,
                    stage,
                    goal_id: goal,
                    ..Default::default()
                };
            }
            if p.state == "running" {
                p.elapsed_ms += elapsed(p);
            }
            p.started_at.clear();
            p.last_event = event_id.into();
            if p.attempts >= 2 || p.elapsed_ms >= 20 * 60_000 {
                p.state = "exhausted".into();
                return Ok(());
            }
            p.attempts += 1;
            p.state = "running".into();
            p.started_at = now();
            Ok(())
        })?;
        Ok(self
            .database
            .with_connection(|c| load(c, task))?
            .is_some_and(|r| preflight_recovery_active(&r)))
    }
    pub fn production_resume_preflight(&self, task: &str) -> AppResult<()> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(());
        };
        if !preflight_recovery_resumable(&r) {
            return Ok(());
        }
        validation::approved(&r, &self.production_workspace(task)?, "requirements")?;
        self.production_commit(task, r.revision, |r| {
            r.workbench.execution.preflight.state = "running".into();
            r.workbench.execution.preflight.started_at = now();
            Ok(())
        })
    }
}
