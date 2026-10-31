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
    if !matches!(
        u.kind.as_str(),
        "recovery" | "repair-plan" | "handoff-repair"
    ) {
        return vec![];
    }
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
    if super::execution_loop::enabled(r) {
        return super::execution_loop::budget_exhausted(r, u, extra);
    }
    if super::progress_budget::enabled(r) {
        return super::progress_budget::exhausted(r, u, extra);
    }
    u.elapsed_ms.saturating_sub(u.budget_epoch_ms) + extra >= limit(r, "workMinutes", 60) * 60_000
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
    super::progress_budget::refresh(r);
}
pub(super) fn migrate_budget(r: &mut ProductionRecord) {
    if r.current_stage != "implementation" {
        return;
    }
    let scope = autonomy::scope(r);
    if completion::enabled(r) {
        // Migration preserves charged time. Unobserved time after a stopped executor is not work.
        for u in &mut r.workbench.execution.units {
            if u.scope_hash == scope {
                if !u.attempt_started_at.is_empty() {
                    u.definition_history.push(json!({"at":now(),"unobservedSince":u.attempt_started_at,"reason":"升级前执行已停止；未观测间隔不计费。"}));
                }
                u.attempt_started_at.clear();
                if u.status == "running" {
                    u.status = "deferred".into();
                    u.defer_reason_code = "repairable".into();
                    u.reason =
                        "先核对中断前成果和原始操作收据，再继续原工作；不重放未知写入。".into();
                }
            }
        }
        r.workbench.execution.state = "paused".into();
        r.workbench.execution.active_unit_id = None;
        autonomy::event(
            &mut r.workbench.execution,
            "budget-policy-migrated",
            None,
            "保留原累计及共享问题预算，恢复时只审查当前目标；不按历史文案批量重置队列。",
        );
        return;
    }
    let previous = if completion::enabled(r) {
        r.workbench.execution.recovery_ms.clone()
    } else {
        std::mem::take(&mut r.workbench.execution.recovery_ms)
    };
    let units = &mut r.workbench.execution.units;
    for u in units
        .iter_mut()
        .filter(|u| u.cycle == r.cycle && u.scope_hash == scope)
    {
        if u.kind != "recovery"
            && u.status == "deferred"
            && (u.reason == "不同执行路线已达到恢复预算；暂缓本项，继续其他工作。"
                || u.reason == "本项或关联故障的累计恢复预算已用完，保留成果并继续独立工作。")
        {
            u.status = "pending".into();
            u.route = 0;
            u.attempts = 0;
            u.route_elapsed_ms = 0;
            u.budget_epoch_ms = u.elapsed_ms;
            u.attempt_started_at.clear();
            u.reason = "兼容升级已纠正检查点计费，沿用原ID核对并继续剩余工作。".into();
        }
    }
    for _ in 0..units.len() {
        let revived: Vec<_> = units
            .iter()
            .filter(|u| u.cycle == r.cycle && u.scope_hash == scope && u.status == "pending")
            .map(|u| u.id.clone())
            .collect();
        for u in units
            .iter_mut()
            .filter(|u| u.cycle == r.cycle && u.scope_hash == scope)
        {
            if u.status == "deferred"
                && u.reason == "前置工作未完成；本轮暂缓部署或验证，已有成果保留。"
                && u.depends_on.iter().any(|id| revived.contains(id))
            {
                u.status = "pending".into();
            }
        }
    }
    r.workbench.execution.state = "paused".into();
    r.workbench.execution.active_unit_id = None;
    r.workbench.execution.events.push(
        json!({"kind":"budget-policy-migrated","at":now(),"previousRecoveryMs":previous,
        "reason":"显式升级保留累计耗时及历史；检查点不再当故障，等待用户恢复原子流程。"}),
    );
}
impl TaskService {
    pub fn production_execution_checkpoint(&self, task: &str, event_id: &str) -> AppResult<bool> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(false);
        };
        if !autonomy::autonomous_implementation(&r) || r.workbench.execution.last_event == event_id
        {
            return Ok(false);
        }
        let Some(unit) = r.workbench.execution.units.iter().find(|u| {
            Some(&u.id) == r.workbench.execution.active_unit_id.as_ref() && u.status == "running"
        }) else {
            return Ok(false);
        };
        let exhausted_now = if super::progress_budget::enabled(&r) {
            let mut next = r.clone();
            account(&mut next);
            next.workbench
                .execution
                .units
                .iter()
                .find(|u| u.id == unit.id)
                .is_none_or(|u| exhausted(&next, u))
        } else {
            exhausted(&r, unit)
        };
        if unit.kind == "recovery" && !super::progress_budget::enabled(&r) || exhausted_now {
            return Ok(false);
        }
        self.production_commit(task, r.revision, |r| {
            account(r);
            r.workbench.execution.state = "ready".into();
            r.workbench.execution.last_event = event_id.into();
            let id = r.workbench.execution.active_unit_id.clone();
            autonomy::event(
                &mut r.workbench.execution,
                "checkpoint",
                id.as_deref(),
                "轮次预算检查点：保留当前工作和路线，先核对已有产出再续接；不计为技术失败。",
            );
            Ok(())
        })?;
        Ok(true)
    }
    pub fn production_execution_observation_gap(&self, task: &str) -> AppResult<()> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(());
        };
        if super::preflight_recovery::preflight_recovery_active(&r) {
            return self.production_commit(task, r.revision, |r| {
                r.workbench.execution.preflight.started_at.clear();
                r.workbench.execution.preflight.state = "paused".into();
                Ok(())
            });
        }
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
        if super::preflight_recovery::due(&r) {
            return Ok(true);
        }
        if !autonomy::autonomous_implementation(&r)
            || !matches!(r.workbench.execution.state.as_str(), "running" | "ready")
        {
            return Ok(false);
        }
        // Reporting a unit early must not allow the same native turn to run forever.
        let turn_ms = (limit(&r, "turnMinutes", 15)
            + if super::progress_budget::enabled(&r) {
                limit(&r, "checkpointGraceMinutes", 3)
            } else {
                0
            })
            * 60_000;
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
                    Utc::now().signed_duration_since(at).num_milliseconds() >= turn_ms
                });
        if dispatch_expired {
            return Ok(true);
        }
        Ok(r.workbench.execution.units.iter().any(|u| {
            u.status == "running"
                && Some(&u.id) == r.workbench.execution.active_unit_id.as_ref()
                && (running_ms(u) >= turn_ms
                    || u.kind == "recovery"
                        && u.route_elapsed_ms + running_ms(u)
                            >= limit(&r, "routeMinutes", 15) * 60_000
                    || exhausted(&r, u))
        }))
    }
}
