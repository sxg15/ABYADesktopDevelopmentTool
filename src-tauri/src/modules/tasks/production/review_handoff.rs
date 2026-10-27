use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["implementationExecution"]["reviewHandoffVersion"] == 1
}
/// A target label or a timeout is not a platform diagnosis. Recheck files at handoff.
fn source_defect(r: &ProductionRecord, i: &Value) -> bool {
    i["status"] != "resolved"
        && matches!(i["kind"].as_str(), Some("defect" | "blocker"))
        && i["repairTarget"] == "abya-source"
        && i["diagnosisStatus"] == "confirmed"
        && i["diagnosis"]["playerBuildHash"]
            .as_str()
            .is_some_and(|h| !h.is_empty())
        && i["diagnosis"]["playerBuildHash"] == r.version_details["playerBuildHash"]
        && ["reproduction", "comparison", "rootCause"].iter().all(|k| {
            i["diagnosis"][k]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty())
        })
        && i["evidenceIds"].as_array().is_some_and(|ids| {
            !ids.is_empty()
                && ids.iter().all(|id| {
                    r.evidence.iter().any(|e| {
                        Some(e.id.as_str()) == id.as_str()
                            && e.reviewed
                            && Some(&e.version) == r.current_version.as_ref()
                            && e.cycle == r.cycle
                    })
                })
        })
}
fn covered(ids: &Value, allowed: &BTreeSet<String>) -> bool {
    ids.as_array().is_some_and(|ids| {
        !ids.is_empty()
            && ids
                .iter()
                .all(|id| id.as_str().is_some_and(|id| allowed.contains(id)))
    })
}
pub(super) fn deficits(r: &ProductionRecord) -> Vec<String> {
    if !enabled(r) {
        return vec![];
    }
    let allowed: BTreeSet<_> = r
        .issues
        .iter()
        .filter(|i| source_defect(r, i))
        .filter_map(|i| i["id"].as_str().map(str::to_owned))
        .collect();
    let mut gaps: Vec<_> = r
        .issues
        .iter()
        .filter(|i| i["status"] != "resolved" && i["kind"] != "suggestion")
        .filter(|i| !source_defect(r, i))
        .map(|i| format!("issue:{}", i["id"].as_str().unwrap_or("unknown")))
        .collect();
    let mut checks: BTreeSet<_> = r.policy["selfTestChecks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
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
    checks.extend(
        latest
            .iter()
            .filter(|(_, t)| t["status"] == "failed" || t["status"] == "blocked")
            .map(|(id, _)| *id),
    );
    for id in checks {
        let passed = latest.get(id).is_some_and(|t| {
            t["status"] == "passed"
                || id == "authority" && r.player_mode == "single" && t["status"] == "not-applicable"
                || matches!(t["status"].as_str(), Some("failed" | "blocked"))
                    && covered(&t["blockedByIssueIds"], &allowed)
        });
        if !passed {
            gaps.push(format!("check:{id}"));
        }
    }
    let relevant = completion::relevant(r);
    let units: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| {
            relevant.contains(&u.id)
                && !matches!(
                    u.kind.as_str(),
                    "finalization" | "repair-plan" | "handoff-repair"
                )
        })
        .collect();
    let mut exempt: BTreeSet<_> = units
        .iter()
        .filter(|u| {
            matches!(u.status.as_str(), "deferred" | "awaiting-condition")
                && covered(&json!(u.issue_ids), &allowed)
        })
        .map(|u| u.id.clone())
        .collect();
    let succeeded: BTreeSet<_> = units
        .iter()
        .filter(|u| u.status == "succeeded")
        .map(|u| u.id.clone())
        .collect();
    for _ in 0..units.len() {
        for u in &units {
            if u.status == "deferred"
                && u.defer_reason_code == "dependency"
                && u.depends_on.iter().any(|id| exempt.contains(id))
                && u.depends_on
                    .iter()
                    .all(|id| exempt.contains(id) || succeeded.contains(id))
            {
                exempt.insert(u.id.clone());
            }
        }
    }
    gaps.extend(
        units
            .iter()
            .filter(|u| !exempt.contains(&u.id) && !succeeded.contains(&u.id))
            .map(|u| format!("work:{}", u.id)),
    );
    gaps.sort();
    gaps.dedup();
    gaps
}

/// Add one bounded repair obligation per material condition, never per summary rewrite.
pub(super) fn ensure_repairs(r: &mut ProductionRecord) {
    if !enabled(r) {
        return;
    }
    let relevant = completion::relevant(r);
    if r.workbench.execution.units.iter().any(|u| {
        relevant.contains(&u.id)
            && u.kind != "finalization"
            && (u.status == "running"
                || matches!(u.status.as_str(), "pending" | "failed")
                    && u.depends_on.iter().all(|id| {
                        r.workbench
                            .execution
                            .units
                            .iter()
                            .any(|d| &d.id == id && d.status == "succeeded")
                    }))
    }) {
        return;
    }
    let gaps = deficits(r);
    if gaps.is_empty() || r.workbench.execution.units.len() >= 256 {
        return;
    }
    let issues: Vec<_> = r
        .issues
        .iter()
        .filter(|i| i["status"] != "resolved" && i["kind"] != "suggestion")
        .map(|i| {
            json!([
                i["id"],
                i["status"],
                i["repairTarget"],
                i["diagnosis"],
                i["evidenceIds"]
            ])
        })
        .collect();
    let signature = format!(
        "{:x}",
        Sha256::digest(
            json!([
                autonomy::scope(r),
                r.workbench.active_goal_id,
                r.current_version,
                r.version_details["archiveHash"],
                completion::check_signature(r),
                gaps,
                issues
            ])
            .to_string()
        )
    );
    let id = format!("handoff-repair-{signature}");
    if r.workbench.execution.units.iter().any(|u| u.id == id) {
        return;
    }
    let issue_ids = r
        .issues
        .iter()
        .filter(|i| i["status"] != "resolved" && i["kind"] != "suggestion" && !source_defect(r, i))
        .filter_map(|i| i["id"].as_str().map(str::to_owned))
        .collect();
    r.workbench.execution.units.push(autonomy::WorkUnit {
        id, kind: "handoff-repair".into(), goal_id: r.workbench.active_goal_id.clone().unwrap_or_default(),
        cycle: r.cycle, scope_hash: autonomy::scope(r), issue_ids, input_signature: signature,
        title: "自动修复普通缺陷并补齐试玩前自测".into(), status: "pending".into(),
        routes: vec![format!("逐项处理 {}。普通 Lua/接线/资源/输入/Bot/测试问题必须在实现阶段修复并复验，使用原工作项 revise-work-unit/reconcile-work-unit，保留 ID 与累计预算。只有确证 Abya-PB 源码根因时，登记 repairTarget=abya-source、confirmed、当前构建复现/对照/根因和证据；受其影响的失败检查明确 blockedByIssueIds。疑似、超时、未测、工具权限或预算耗尽不是源码缺陷。不得要求玩家处理技术问题，不得改成建议或虚假通过。", gaps.join("、")),
            "检查实际结果，选择不同修复路线并验证。未知写入先对账；无法继续时留在 Lua 技术恢复状态，保留包和证据，不能把普通缺陷交给试玩。".into()],
        updated_at: now(), ..Default::default()
    });
}

pub(super) fn gate(r: &ProductionRecord, root: &Path) -> AppResult<()> {
    if !enabled(r) {
        return Ok(());
    }
    let gaps = deficits(r);
    if !gaps.is_empty() {
        return Err(AppError::validation(format!(
            "Lua 实现尚需自动修复或验证，不能转交试玩：{}。只有当前版本已证实的 Abya-PB 源码缺陷可带入试玩。",
            gaps.join("、")
        )));
    }
    let version = validation::version(r)?;
    for i in r.issues.iter().filter(|i| source_defect(r, i)) {
        validation::evidence_refs(r, root, &i["evidenceIds"], version)?;
    }
    let mut latest = BTreeMap::new();
    for t in r
        .self_tests
        .iter()
        .filter(|t| t["version"] == version && t["cycle"] == r.cycle)
    {
        latest.insert(t["id"].as_str().unwrap_or(""), t);
    }
    for t in latest.values().filter(|t| t["status"] == "passed") {
        validation::evidence_refs(r, root, &t["evidenceIds"], version)?;
    }
    Ok(())
}

pub(super) fn migrate(r: &mut ProductionRecord) {
    if r.current_stage != "review"
        || r.stages
            .get("implementation")
            .is_none_or(|s| s != "execution-converged")
    {
        return;
    }
    if let Some(goal) = r
        .workbench
        .goals
        .iter()
        .rev()
        .find(|g| g.stage == "implementation" && g.cycle == r.cycle)
    {
        let old_goal = r.workbench.active_goal_id.clone();
        r.workbench.active_goal_id = Some(goal.id.clone());
        let gaps = deficits(r);
        if gaps.is_empty() {
            r.workbench.active_goal_id = old_goal;
            return;
        }
        r.current_stage = "implementation".into();
        r.stages
            .insert("implementation".into(), "in-progress".into());
        r.stages.insert("review".into(), "not-started".into());
        r.workbench.execution.state = "paused".into();
        r.workbench.execution.active_unit_id = None;
        ensure_repairs(r);
        autonomy::event(
            &mut r.workbench.execution,
            "review-handoff-migrated",
            None,
            "普通缺陷和未测项返回原 Lua 目标；原包、证据、预算及批准保留，未自动开始执行。",
        );
    }
}
