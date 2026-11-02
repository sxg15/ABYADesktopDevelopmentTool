use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["implementationExecution"]["progressBudgetVersion"] == 1
}
fn family(u: &autonomy::WorkUnit) -> String {
    if matches!(
        u.kind.as_str(),
        "repair-plan" | "handoff-repair" | "recovery"
    ) {
        let mut issues = u.issue_ids.clone();
        issues.sort();
        issues.dedup();
        format!(
            "{}:{}:{}:{}",
            u.scope_hash,
            u.goal_id,
            u.repair_for,
            json!(issues)
        )
    } else {
        format!("{}:{}", u.scope_hash, u.id)
    }
}
pub(super) fn exhausted(r: &ProductionRecord, u: &autonomy::WorkUnit, extra: i64) -> bool {
    let base = autonomy_budget::limit(r, "workMinutes", 60) * 60_000;
    let members: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|v| family(v) == family(u))
        .collect();
    let total: i64 = members.iter().map(|v| v.elapsed_ms).sum();
    let credits: i64 = members
        .iter()
        .flat_map(|v| &v.budget_grants)
        .filter_map(|g| g["extraMs"].as_i64())
        .sum();
    total + extra >= base + credits
}
// Only persisted candidate content, reviewed checks and actual dependency completion
// are progress. Narrative files, new IDs and timestamps are deliberately excluded.
fn markers(r: &ProductionRecord, u: &autonomy::WorkUnit) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let content: Vec<_> = r.version_details["contentManifest"]["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| {
            f["path"].as_str().is_some_and(|p| {
                let p = p.to_lowercase();
                p.ends_with(".lua") || p.ends_with(".pblevel") || p.ends_with(".pbui")
            })
        })
        .map(|f| json!([f["path"], f["sha256"]]))
        .collect();
    if !content.is_empty() {
        result.insert(format!(
            "source:{:x}",
            Sha256::digest(json!(content).to_string())
        ));
    }
    let mut latest = BTreeMap::new();
    for t in r
        .self_tests
        .iter()
        .filter(|t| t["version"].as_str() == r.current_version.as_deref() && t["cycle"] == r.cycle)
    {
        if let Some(id) = t["id"].as_str() {
            latest.insert(id, t);
        }
    }
    for (id, t) in latest {
        if t["status"] == "passed"
            && t["evidenceIds"].as_array().is_some_and(|ids| {
                !ids.is_empty()
                    && ids.iter().all(|id| {
                        r.evidence.iter().any(|e| {
                            Some(e.id.as_str()) == id.as_str()
                                && e.reviewed
                                && e.cycle == r.cycle
                                && Some(&e.version) == r.current_version.as_ref()
                        })
                    })
            })
        {
            result.insert(format!(
                "check:{id}:{}",
                r.version_details["archiveHash"].as_str().unwrap_or("")
            ));
        }
    }
    for run in r.workbench.execution.validation_runs.values().filter(|v| {
        v["workUnitId"] == u.id && v["cycle"] == r.cycle && v["scopeHash"] == u.scope_hash
    }) {
        for check in run["checks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["status"] == "passed")
        {
            // A repeated passing check on the same saved content is not new progress.
            let version = run["version"].as_str().unwrap_or("");
            let archive = if run["purpose"] == "fixture" {
                &r.workbench
                    .execution
                    .validation_snapshots
                    .get(version)
                    .unwrap_or(&Value::Null)["archiveHash"]
            } else {
                &r.version_details["archiveHash"]
            };
            result.insert(format!("validation:{}:{}", check["id"], archive));
        }
    }
    for id in &u.depends_on {
        if let Some(d) = r
            .workbench
            .execution
            .units
            .iter()
            .find(|d| &d.id == id && d.status == "succeeded")
        {
            result.insert(format!("dependency:{id}:{}", d.result_hash));
        }
    }
    for id in &u.issue_ids {
        if r.issues
            .iter()
            .any(|i| i["id"] == *id && i["status"] == "resolved")
        {
            result.insert(format!("resolved:{id}"));
        }
    }
    result
}
pub(super) fn initialize(r: &mut ProductionRecord, id: &str) {
    if super::execution_loop::enabled(r) {
        if let Some(u) = r.workbench.execution.units.iter().find(|u| u.id == id) {
            let remaining = if u.kind == "finalization" {
                (autonomy_budget::limit(r, "settlementMinutes", 15) * 60_000 - u.elapsed_ms).max(0)
            } else {
                (super::execution_loop::goal_limit(r)
                    - super::execution_loop::goal_used(r, &u.goal_id))
                .max(0)
            };
            let u = r
                .workbench
                .execution
                .units
                .iter_mut()
                .find(|u| u.id == id)
                .unwrap();
            u.execution_limit_ms = u.elapsed_ms + remaining;
        }
        return;
    }
    if !enabled(r) {
        return;
    }
    let Some(u) = r.workbench.execution.units.iter().find(|u| u.id == id) else {
        return;
    };
    if u.execution_limit_ms > 0 {
        return;
    }
    let baseline = markers(r, u);
    let limit = autonomy_budget::limit(r, "workMinutes", 60) * 60_000;
    let u = r
        .workbench
        .execution
        .units
        .iter_mut()
        .find(|u| u.id == id)
        .unwrap();
    u.progress_seen = baseline;
    u.execution_limit_ms = limit;
}
pub(super) fn refresh(r: &mut ProductionRecord) {
    if super::execution_loop::enabled(r) {
        return;
    }
    if !enabled(r) {
        return;
    }
    let Some(id) = r.workbench.execution.active_unit_id.clone() else {
        return;
    };
    initialize(r, &id);
    let Some(u) = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| u.id == id && u.status == "running")
    else {
        return;
    };
    if super::execution_loop::budget_exhausted(r, u, 0) {
        return;
    }
    let members: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|v| family(v) == family(u))
        .collect();
    let total: i64 = members.iter().map(|v| v.elapsed_ms).sum();
    let credits: i64 = members
        .iter()
        .flat_map(|v| &v.budget_grants)
        .filter_map(|g| g["extraMs"].as_i64())
        .sum();
    let ceiling = autonomy_budget::limit(r, "workMinutes", 60) * 60_000 + credits;
    if total + 5 * 60_000 < ceiling {
        return;
    }
    let observed = markers(r, u);
    let seen: BTreeSet<_> = members
        .iter()
        .flat_map(|v| v.progress_seen.iter().cloned())
        .collect();
    let fresh: Vec<_> = observed.difference(&seen).cloned().collect();
    let grants: Vec<_> = members.iter().flat_map(|v| &v.budget_grants).collect();
    let grant_count = grants.iter().filter(|g| g["kind"] == "progress").count();
    let sources = grants
        .iter()
        .filter(|g| g["kind"] == "progress" && g["sourceOnly"] == true)
        .count();
    let settled = members.iter().any(|v| v.settlement_used);
    let max = autonomy_budget::limit(r, "progressGrantLimit", 6) as usize;
    let slice = autonomy_budget::limit(r, "progressSliceMinutes", 20) * 60_000;
    let settlement = autonomy_budget::limit(r, "settlementMinutes", 15) * 60_000;
    let u = r
        .workbench
        .execution
        .units
        .iter_mut()
        .find(|u| u.id == id)
        .unwrap();
    let source_only = fresh.iter().all(|m| m.starts_with("source:"));
    let progress = !fresh.is_empty() && grant_count < max && (!source_only || sources < 2);
    if progress || !settled {
        let kind = if progress { "progress" } else { "settlement" };
        let duration = if progress { slice } else { settlement };
        let extra = (total - ceiling).max(0) + duration;
        u.execution_limit_ms = u.execution_limit_ms.max(u.elapsed_ms) + duration;
        if !progress {
            u.settlement_used = true;
        }
        u.budget_grants.push(json!({"kind":kind,"extraMs":extra,"at":now(),"sourceOnly":source_only,"markers":fresh,"elapsedMs":u.elapsed_ms,"limitMs":u.execution_limit_ms}));
    }
    u.progress_seen.extend(observed);
}

pub(super) fn migrate(r: &mut ProductionRecord) {
    if !enabled(r) || r.current_stage != "implementation" {
        return;
    }
    let ids = completion::relevant(r);
    let settlement = autonomy_budget::limit(r, "settlementMinutes", 15) * 60_000;
    let originals: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| {
            ids.contains(&u.id)
                && !matches!(
                    u.kind.as_str(),
                    "repair-plan" | "handoff-repair" | "finalization"
                )
                && u.status != "succeeded"
        })
        .map(|u| u.id.clone())
        .collect();
    for id in originals {
        initialize(r, &id);
        let u = r
            .workbench
            .execution
            .units
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap();
        u.attempt_started_at.clear();
        if u.defer_reason_code == "budget-exhausted" && !u.settlement_used {
            let extra = (u.elapsed_ms + settlement - u.execution_limit_ms).max(0);
            u.execution_limit_ms += extra;
            u.settlement_used = true;
            u.budget_grants.push(json!({"kind":"migration-settlement","extraMs":extra,"at":now(),"elapsedMs":u.elapsed_ms,"limitMs":u.execution_limit_ms}));
            u.status = "pending".into();
            u.defer_reason_code.clear();
            u.route_elapsed_ms = 0;
            u.attempts = 0;
            u.route = 0;
            u.routes.insert(0,"升级后的有限收尾：先读取本项 observed-result-handoff/result 和原始回执，核对已保存成果及未知写入。完成已验证方案的最小接线、保存读回和必要复验，沿原 ID 结案；不重做已通过内容。独立外部能力问题单列，不等待它阻止当前可做部分。执行时间仍累计，不伪造通过。".into());
            u.reason = "沿原项核对已有成果并完成有限收尾；累计耗时和历史完整保留。".into();
        }
    }
    r.workbench.execution.active_unit_id = None;
    r.workbench.execution.state = "paused".into();
    autonomy::event(
        &mut r.workbench.execution,
        "progress-budget-migrated",
        None,
        "原工作项获得一次有限成果收尾；不重置累计耗时、批准、未知写入或试玩门槛。",
    );
}

/// Reconsider a parked original unit only when persisted material progress earned
/// credits. This does not resume native execution or reconcile unknown writes.
pub(super) fn reassess(r: &mut ProductionRecord) -> bool {
    if super::execution_loop::enabled(r) {
        return false;
    }
    if !enabled(r) || !autonomy::autonomous_implementation(r) {
        return false;
    }
    let relevant = completion::relevant(r);
    let targets: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| {
            relevant.contains(&u.id)
                && u.status == "deferred"
                && u.defer_reason_code == "budget-exhausted"
                && !matches!(
                    u.kind.as_str(),
                    "repair-plan" | "handoff-repair" | "finalization"
                )
        })
        .map(|u| u.id.clone())
        .collect();
    let mut changed = false;
    for id in targets {
        let index = r
            .workbench
            .execution
            .units
            .iter()
            .position(|u| u.id == id)
            .unwrap();
        let old = &r.workbench.execution.units[index];
        let mut probe = r.clone();
        probe.workbench.execution.active_unit_id = Some(id);
        probe.workbench.execution.units[index].status = "running".into();
        probe.workbench.execution.units[index].settlement_used = true;
        refresh(&mut probe);
        let next = &probe.workbench.execution.units[index];
        if next.budget_grants.len() > old.budget_grants.len() && !exhausted(&probe, next, 0) {
            let mut next = next.clone();
            next.status = "pending".into();
            next.defer_reason_code.clear();
            next.settlement_used = old.settlement_used;
            next.attempt_started_at.clear();
            next.route = 0;
            next.attempts = 0;
            next.route_elapsed_ms = 0;
            r.workbench.execution.units[index] = next;
            changed = true;
        }
    }
    changed
}
