use super::*;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExecutionLedger {
    pub condition_signals: BTreeMap<String, Value>,
    pub verification_package: Value,
    pub preflight: super::preflight_recovery::PreflightRecovery,
    pub validation_snapshots: BTreeMap<String, Value>,
    pub recovery_ms: BTreeMap<String, i64>,
    pub summary: Value,
    pub scope_hash: String,
    pub parked_condition: String,
    pub validation_runs: BTreeMap<String, Value>,
    pub receipts: BTreeMap<String, Value>,
    pub units: Vec<WorkUnit>,
    pub active_unit_id: Option<String>,
    pub state: String,
    pub events: Vec<Value>,
    pub last_progress: String,
    pub last_event: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkUnit {
    pub progress_seen: std::collections::BTreeSet<String>,
    pub budget_grants: Vec<Value>,
    pub execution_limit_ms: i64,
    pub settlement_used: bool,
    pub validation_failures: std::collections::BTreeSet<String>,
    pub milestone: String,
    pub validation_outcome: String,
    pub definition_revision: u32,
    pub definition_history: Vec<Value>,
    pub defer_reason_code: String,
    pub next_condition: String,
    pub repair_for: String,
    pub input_signature: String,
    pub budget_epoch_ms: i64,
    pub kind: String,
    pub attempt_started_at: String,
    pub elapsed_ms: i64,
    pub route_elapsed_ms: i64,
    pub scope_hash: String,
    pub id: String,
    pub goal_id: String,
    pub cycle: u32,
    pub title: String,
    pub depends_on: Vec<String>,
    pub issue_ids: Vec<String>,
    pub routes: Vec<String>,
    pub route: usize,
    pub attempts: u32,
    pub status: String,
    pub reason: String,
    pub condition_hash: String,
    pub result_paths: Vec<String>,
    pub result_hash: String,
    pub started_at: String,
    pub updated_at: String,
}
pub fn autonomous_implementation(r: &ProductionRecord) -> bool {
    r.policy["implementationExecution"]["mode"] == "autonomous"
        && r.current_stage == "implementation"
        && matches!(
            r.stages.get("implementation").map(String::as_str),
            Some("in-progress" | "not-started" | "blocked")
        )
}
pub(super) fn scope(r: &ProductionRecord) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                r.cycle,
                r.documents.get("requirements").map(|d| &d.sha256),
                r.documents.get("plan").map(|d| &d.sha256)
            ])
            .to_string()
        )
    )
}
pub(super) fn condition(r: &ProductionRecord) -> String {
    if completion::enabled(r) {
        let ids = completion::relevant(r);
        let units: Vec<_> = r
            .workbench
            .execution
            .units
            .iter()
            .filter(|u| ids.contains(&u.id))
            .map(|u| {
                json!([
                    u.id,
                    u.definition_revision,
                    u.status,
                    u.result_hash,
                    u.defer_reason_code,
                    unit_condition(r, u)
                ])
            })
            .collect();
        return format!(
            "{:x}",
            Sha256::digest(
                json!([
                    scope(r),
                    r.workbench.active_goal_id,
                    r.current_version,
                    r.workbench.execution.condition_signals,
                    r.version_details["archiveHash"],
                    r.version_details["playerBuildHash"],
                    completion::check_signature(r),
                    super::review_handoff::deficits(r),
                    r.workbench
                        .execution
                        .validation_runs
                        .iter()
                        .map(|(id, v)| json!([id, v["inputHash"]]))
                        .collect::<Vec<_>>(),
                    r.workbench.execution.verification_package["sha256"],
                    units
                ])
                .to_string()
            )
        );
    }
    let issues: Vec<_> = r
        .issues
        .iter()
        .map(|i| json!([i["id"], i["status"] == "resolved", i["repairTarget"]]))
        .collect();
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                scope(r),
                r.version_details["archiveHash"],
                r.version_details["playerBuildHash"],
                issues
            ])
            .to_string()
        )
    )
}
pub(super) fn unit_condition(r: &ProductionRecord, u: &WorkUnit) -> String {
    let issues = r
        .issues
        .iter()
        .filter(|i| {
            i["id"]
                .as_str()
                .is_some_and(|id| u.issue_ids.iter().any(|v| v == id))
        })
        .map(|i| json!([i["id"], i["status"] == "resolved"]))
        .collect::<Vec<_>>();
    let dependencies = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|d| u.depends_on.contains(&d.id))
        .map(|d| json!([d.id, d.result_hash, d.status]))
        .collect::<Vec<_>>();
    let payload = if completion::enabled(r) {
        json!([
            scope(r),
            r.version_details["playerBuildHash"],
            issues,
            dependencies,
            r.workbench.execution.condition_signals.get(&u.id)
        ])
    } else {
        json!([
            scope(r),
            r.version_details["playerBuildHash"],
            issues,
            dependencies
        ])
    };
    format!("{:x}", Sha256::digest(payload.to_string()))
}
pub(super) fn progress(r: &ProductionRecord) -> String {
    let outputs: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| u.cycle == r.cycle)
        .map(|u| json!([u.id, u.status, u.result_hash]))
        .collect();
    let tests: Vec<_> = r
        .self_tests
        .iter()
        .map(|t| json!([t["id"], t["status"], t["version"], t["evidenceIds"]]))
        .collect();
    format!(
        "{:x}",
        Sha256::digest(json!([r.current_version, outputs, tests]).to_string())
    )
}
pub(super) fn event(ledger: &mut ExecutionLedger, kind: &str, unit: Option<&str>, reason: &str) {
    ledger
        .events
        .push(json!({"kind":kind,"unitId":unit,"reason":reason,"at":now()}));
    if ledger.events.len() > 1000 {
        ledger.events.drain(..ledger.events.len() - 1000);
    }
    ledger.updated_at = now();
}
fn check_enabled(r: &ProductionRecord) -> AppResult<()> {
    if !autonomous_implementation(r) {
        return Err(AppError::validation("当前阶段未启用自主执行策略。"));
    }
    Ok(())
}
pub(super) fn apply(
    r: &mut ProductionRecord,
    root: &Path,
    op: &str,
    data: &Value,
) -> AppResult<()> {
    check_enabled(r)?;
    validation::approved(r, root, "requirements")?;
    validation::approved(r, root, "plan")?;
    match op {
        "revise-work-unit" | "classify-work-unit" | "reconcile-work-unit" => {
            super::completion_repair::apply(r, root, op, data)
        }
        "conclude-implementation" => conclude(r, root, data),
        "save-work-unit" => save_unit(r, data),
        "report-work-unit" => report_unit(r, root, data),
        "record-issue-handling" => handling(r, root, data),
        _ => Err(AppError::validation("未知自主执行操作。")),
    }
}
fn strings(data: &Value, key: &str, max: usize) -> AppResult<Vec<String>> {
    let values = data[key]
        .as_array()
        .ok_or_else(|| AppError::validation(format!("{key} 必须为数组。")))?;
    if values.len() > max {
        return Err(AppError::validation("工作项字段超出容量。"));
    }
    values
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| !s.trim().is_empty() && s.len() <= 1000)
                .map(str::to_owned)
                .ok_or_else(|| AppError::validation("工作项字段必须为非空短文本。"))
        })
        .collect()
}
fn save_unit(r: &mut ProductionRecord, data: &Value) -> AppResult<()> {
    let milestone = data["milestone"].as_str().unwrap_or("unspecified");
    if !["core", "extension", "unspecified"].contains(&milestone) {
        return Err(AppError::validation(
            "milestone 应为 core、extension 或 unspecified。",
        ));
    }
    let kind = data["kind"].as_str().unwrap_or("implementation");
    if !["implementation", "deployment", "validation", "recovery"].contains(&kind) {
        return Err(AppError::validation("无效工作项类型。"));
    }
    let id = mutation::text(data, "id")?;
    if id.len() > 100
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-:".contains(c))
    {
        return Err(AppError::validation("无效工作项 ID。"));
    }
    let goal = mutation::text(data, "goalId")?;
    let title = mutation::text(data, "title")?;
    if title.chars().count() > 160 {
        return Err(AppError::validation("工作项名称过长。"));
    }
    if r.workbench.goals.is_empty() {
        super::workbench::observe(r, None);
    }
    if !r
        .workbench
        .goals
        .iter()
        .any(|g| g.id == goal && g.cycle == r.cycle && g.stage == "implementation")
    {
        return Err(AppError::validation(
            "工作项必须绑定当前 Lua 阶段的原子目标。",
        ));
    }
    let dependencies = strings(data, "dependsOn", 64)?;
    let issues = strings(data, "issueIds", 32)?;
    if kind == "recovery" && issues.is_empty() {
        return Err(AppError::validation(
            "恢复工作必须绑定问题，以共享故障预算。",
        ));
    }
    let routes = strings(data, "routes", 4)?;
    if routes.is_empty() {
        return Err(AppError::validation("至少提供一条执行路线。"));
    }
    if issues
        .iter()
        .any(|id| !r.issues.iter().any(|i| i["id"] == *id))
    {
        return Err(AppError::validation("问题 ID 不存在。"));
    }
    let scope_hash = scope(r);
    let ledger = &mut r.workbench.execution;
    if let Some(old) = ledger.units.iter().find(|u| u.id == id) {
        if old.goal_id == goal
            && old.cycle == r.cycle
            && old.scope_hash == scope_hash
            && old.title == title
            && old.depends_on == dependencies
            && old.issue_ids == issues
            && old.routes == routes
            && (old.milestone == milestone
                || old.milestone.is_empty() && milestone == "unspecified")
            && (old.kind == kind || old.kind.is_empty() && kind == "implementation")
        {
            return Ok(());
        }
        return Err(AppError::validation(
            "工作项定义已存在；变更方案请追加新 ID，保留原记录。",
        ));
    }
    if ledger.units.len() >= 256 {
        return Err(AppError::validation("工作项已达到容量上限。"));
    }
    if dependencies.iter().any(|id| {
        !ledger
            .units
            .iter()
            .any(|u| u.id == *id && u.cycle == r.cycle && u.scope_hash == scope_hash)
    }) {
        return Err(AppError::validation(
            "依赖必须先登记且属于当前周期，禁止悬空或循环依赖。",
        ));
    }
    ledger.units.push(WorkUnit {
        milestone: milestone.into(),
        kind: kind.into(),
        scope_hash,
        id: id.into(),
        goal_id: goal.into(),
        cycle: r.cycle,
        title: title.into(),
        depends_on: dependencies,
        issue_ids: issues,
        routes,
        status: "pending".into(),
        updated_at: now(),
        ..Default::default()
    });
    if !ledger
        .units
        .iter()
        .any(|u| u.status == "running" && Some(&u.id) == ledger.active_unit_id.as_ref())
    {
        ledger.state = "ready".into();
    }
    event(ledger, "planned", Some(id), title);
    Ok(())
}
fn report_unit(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let id = mutation::text(data, "id")?;
    let status = mutation::text(data, "status")?;
    let reason = mutation::text(data, "reason")?;
    let loop_enabled = completion::enabled(r);
    if loop_enabled {
        completion::validate_defer(data, status)?;
    }
    if reason.chars().count() > 1000
        || !["succeeded", "failed", "deferred", "awaiting-condition"].contains(&status)
    {
        return Err(AppError::validation("无效工作项结果。"));
    }
    let paths = strings(data, "resultPaths", 16)?;
    let mut digest = Sha256::new();
    for path in &paths {
        digest.update(path);
        digest.update(files::hash_file(&files::safe_path(root, path)?)?.0);
    }
    if status == "succeeded" && paths.is_empty() {
        return Err(AppError::validation(
            "工作项完成须关联实际产出文件；这不代替验收。",
        ));
    }
    let conditions = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| u.id == id)
        .map(|u| unit_condition(r, u))
        .ok_or_else(|| AppError::validation("工作项不存在。"))?;
    super::autonomy_budget::account(r);
    let scope_hash = scope(r);
    let completed_signature = completion::signature(r);
    if loop_enabled {
        let current = r
            .workbench
            .execution
            .units
            .iter()
            .find(|u| u.id == id)
            .ok_or_else(conflict)?;
        if data["deferReasonCode"] == "budget-exhausted" && !autonomy_budget::exhausted(r, current)
        {
            return Err(AppError::validation("不能以说明文字代替实际预算计费。"));
        }
        if current.kind == "repair-plan"
            && status == "succeeded"
            && r.workbench
                .execution
                .units
                .iter()
                .any(|u| u.id == current.repair_for && completion::is_repairable(u))
        {
            return Err(AppError::validation(
                "诊断完成后须修订原工作项或提供明确外部条件；只写总结不能结束修复。",
            ));
        }
        if current.kind == "finalization"
            && status == "succeeded"
            && !super::verification_package::valid(r, root)
        {
            return Err(AppError::validation(
                "最终核对完成须有当前可验证包；有缺口请如实conclude-implementation，不声明成功。",
            ));
        }
        if current.kind == "finalization" && status == "succeeded" {
            super::review_handoff::gate(r, root)?;
        }
    }
    let ledger = &mut r.workbench.execution;
    let unit = ledger
        .units
        .iter_mut()
        .find(|u| u.id == id && u.cycle == r.cycle && u.scope_hash == scope_hash)
        .ok_or_else(|| AppError::validation("工作项不存在。"))?;
    if unit.status == "succeeded" {
        return Err(AppError::validation(
            "已完成工作项不能覆盖；请追加回归工作项。",
        ));
    }
    if ledger.active_unit_id.as_deref() != Some(id) || unit.status != "running" {
        return Err(AppError::validation("只能报告调度器当前派发的工作项。"));
    }
    unit.status = status.into();
    if loop_enabled && status == "succeeded" && unit.validation_outcome == "failed" {
        unit.status = "deferred".into();
    }
    unit.reason = reason.into();
    unit.result_paths = paths;
    unit.result_hash = format!("{:x}", digest.finalize());
    unit.condition_hash = conditions;
    unit.updated_at = now();
    if loop_enabled {
        unit.defer_reason_code = data["deferReasonCode"].as_str().unwrap_or("").into();
        if unit.validation_outcome == "failed" && status == "succeeded" {
            unit.defer_reason_code = "repairable".into();
        }
        unit.next_condition = data["nextCondition"].as_str().unwrap_or("").into();
        if unit.kind == "finalization" {
            unit.input_signature = completed_signature;
        }
    }
    let actual_status = unit.status.clone();
    event(ledger, &actual_status, Some(id), reason);
    Ok(())
}
fn handling(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let id = mutation::text(data, "issueId")?;
    let mode = mutation::text(data, "mode")?;
    if ![
        "diagnosing",
        "repairing",
        "workaround",
        "deferred",
        "awaiting-condition",
    ]
    .contains(&mode)
    {
        return Err(AppError::validation("无效问题处理方式。"));
    }
    let reason = mutation::text(data, "reason")?;
    let impact = mutation::text(data, "impact")?;
    if reason.len() > 4000 || impact.len() > 4000 {
        return Err(AppError::validation("处理说明过长。"));
    }
    let scope_hash = scope(r);
    let units = strings(data, "affectedUnitIds", 64)?;
    if units.iter().any(|id| {
        !r.workbench
            .execution
            .units
            .iter()
            .any(|u| u.id == *id && u.cycle == r.cycle && u.scope_hash == scope_hash)
    }) {
        return Err(AppError::validation("受影响工作项不存在。"));
    }
    let mut handling = json!({"mode":mode,"reason":reason,"impact":impact,"affectedUnitIds":units,"updatedAt":now()});
    if mode == "workaround" {
        let scope = mutation::text(data, "scope")?;
        if !["equivalent", "candidate"].contains(&scope) {
            return Err(AppError::validation(
                "替代方案须区分 equivalent 或 candidate。",
            ));
        }
        handling["scope"] = json!(scope);
        if scope == "equivalent" {
            validation::evidence_refs(r, root, &data["evidenceIds"], validation::version(r)?)?;
            handling["evidenceIds"] = data["evidenceIds"].clone();
            handling["verifiedVersion"] = json!(r.current_version);
            handling["verifiedCycle"] = json!(r.cycle);
            handling["verifiedScope"] = json!(scope_hash);
        }
    }
    let issue = r
        .issues
        .iter_mut()
        .find(|i| i["id"] == id)
        .ok_or_else(|| AppError::validation("问题不存在。"))?;
    if issue["status"] == "resolved" {
        return Err(AppError::validation("问题已解决。"));
    }
    handling["issueFingerprint"] = json!(issue_fingerprint(issue));
    issue["executionHandling"] = handling;
    let conditions = r
        .workbench
        .execution
        .units
        .iter()
        .map(|u| (u.id.clone(), unit_condition(r, u)))
        .collect::<BTreeMap<_, _>>();
    if matches!(mode, "deferred" | "awaiting-condition") {
        let enhanced = completion::enabled(r);
        let active = r.workbench.execution.active_unit_id.clone();
        super::autonomy_budget::account(r);
        for unit in r
            .workbench
            .execution
            .units
            .iter_mut()
            .filter(|u| units.contains(&u.id) && u.status != "succeeded")
        {
            if enhanced && Some(&unit.id) == active.as_ref() {
                continue;
            }
            unit.status = mode.into();
            if enhanced {
                unit.defer_reason_code = if mode == "awaiting-condition" {
                    "unknown-result"
                } else {
                    "repairable"
                }
                .into();
            }
            unit.reason = reason.into();
            unit.condition_hash = conditions.get(&unit.id).cloned().unwrap_or_default();
            unit.updated_at = now();
        }
    }
    event(
        &mut r.workbench.execution,
        "issue-handling",
        None,
        &format!("{id}: {mode}"),
    );
    Ok(())
}
// Execution convergence is distinct from implementation acceptance.
pub(super) fn conclude(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    check_enabled(r)?;
    super::autonomy_budget::account(r);
    let summary = mutation::text(data, "summary")?;
    if summary.len() > 4000 {
        return Err(AppError::validation("收敛摘要过长。"));
    }
    let scope_hash = scope(r);
    if !r
        .workbench
        .execution
        .units
        .iter()
        .any(|u| u.cycle == r.cycle && u.scope_hash == scope_hash)
    {
        return Err(AppError::validation(
            "尚未登记任何实现工作项，不能直接宣称执行收敛。",
        ));
    }
    let active = r.workbench.execution.active_unit_id.clone();
    let enhanced = completion::enabled(r);
    let relevant = completion::relevant(r);
    let sig = completion::signature(r);
    if enhanced
        && !r.workbench.execution.units.iter().any(|u| {
            u.kind == "finalization"
                && Some(&u.goal_id) == r.workbench.active_goal_id.as_ref()
                && u.input_signature == sig
        })
    {
        return Err(AppError::validation(
            "先报告当前工作项；调度器将自动派发本目标的最终核对，不能提前结束。",
        ));
    }
    if r.workbench.execution.units.iter().any(|u| {
        (!enhanced || relevant.contains(&u.id))
            && u.cycle == r.cycle
            && u.scope_hash == scope_hash
            && Some(&u.id) != active.as_ref()
            && matches!(u.status.as_str(), "pending" | "running" | "failed")
    }) {
        return Err(AppError::validation(
            "尚有未处理工作项，请先完成或记录暂缓原因。",
        ));
    }
    let version = r.current_version.clone();
    let playable = version.as_ref().is_some_and(|v| {
        ["lua", "core-loop", "save-reload"].iter().all(|id| {
            r.self_tests
                .iter()
                .rev()
                .find(|t| t["id"] == *id && t["version"] == *v && t["cycle"] == r.cycle)
                .is_some_and(|t| {
                    t["status"] == "passed"
                        && validation::evidence_refs(r, root, &t["evidenceIds"], v).is_ok()
                })
        })
    }) && super::versions::verify(&r.version_details).is_ok();
    let playable = if enhanced {
        super::verification_package::valid(r, root)
    } else {
        playable
    };
    let handoff = super::review_handoff::gate(r, root);
    let ready_for_review = playable && handoff.is_ok();
    let external = enhanced
        && r.workbench.execution.units.iter().any(|u| {
            relevant.contains(&u.id)
                && !matches!(u.kind.as_str(), "repair-plan" | "finalization")
                && u.status != "succeeded"
                && u.defer_reason_code != "dependency"
        })
        && r.workbench
            .execution
            .units
            .iter()
            .filter(|u| {
                relevant.contains(&u.id)
                    && !matches!(u.kind.as_str(), "repair-plan" | "finalization")
                    && u.status != "succeeded"
                    && u.defer_reason_code != "dependency"
            })
            .all(|u| {
                matches!(
                    u.defer_reason_code.as_str(),
                    "external-condition" | "unknown-result" | "budget-exhausted"
                )
            });
    let conditions = condition(r);
    let active_condition = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| Some(&u.id) == active.as_ref())
        .map(|u| unit_condition(r, u))
        .unwrap_or_default();
    let ledger = &mut r.workbench.execution;
    ledger.parked_condition = conditions.clone();
    if let Some(unit) = ledger
        .units
        .iter_mut()
        .find(|u| Some(&u.id) == active.as_ref())
    {
        if !enhanced || unit.status != "succeeded" {
            unit.status = if enhanced && ready_for_review {
                "succeeded"
            } else {
                "deferred"
            }
            .into();
        }
        if enhanced {
            unit.input_signature = sig.clone();
        }
        unit.reason = summary.into();
        unit.condition_hash = active_condition;
        unit.updated_at = now();
    }
    ledger.active_unit_id = None;
    ledger.state = if enhanced && !ready_for_review {
        if external && handoff.is_ok() {
            "externalBlocked"
        } else {
            "repairRequired"
        }
    } else {
        "settledWithGaps"
    }
    .into();
    event(ledger, "converged", None, summary);
    r.stage_updates.insert("implementation".into(),json!({"summary":summary,"nextAction":"在统一结果中核对未完成项和替代方案；不反复发送继续。","updatedAt":now()}));
    if ready_for_review {
        r.stages
            .insert("implementation".into(), "execution-converged".into());
        r.current_stage = "review".into();
        r.stages.insert("review".into(), "awaiting-feedback".into());
    }
    if let Err(error) = handoff {
        r.stage_updates.insert(
            "implementation".into(),
            json!({"summary":summary,
            "nextAction":error.message,"updatedAt":now()}),
        );
        r.current_stage = "implementation".into();
        r.stages
            .insert("implementation".into(), "in-progress".into());
    }
    if enhanced {
        r.workbench.execution.parked_condition = condition(r);
    }
    Ok(())
}
// Only prove fulfillment through explicit goal bindings and completed implementation gates.
pub(super) fn fulfill_requests(r: &mut ProductionRecord) {
    for goal in
        r.workbench.goals.iter().filter(|g| {
            g.stage == "implementation" && g.cycle == r.cycle && g.source_type == "request"
        })
    {
        let units = r
            .workbench
            .execution
            .units
            .iter()
            .filter(|u| u.goal_id == goal.id && u.cycle == r.cycle)
            .collect::<Vec<_>>();
        if units.is_empty() || units.iter().any(|u| u.status != "succeeded") {
            continue;
        }
        if let Some(request) = r
            .workbench
            .requests
            .iter_mut()
            .find(|v| v["id"] == goal.source_id)
        {
            request["status"] = json!("completed");
            request["fulfilledBy"] = json!({"version":r.current_version,"workUnitIds":units.iter().map(|u|&u.id).collect::<Vec<_>>(),"at":now()});
        }
    }
}
// A verified task-level equivalent workaround does not claim the platform defect is fixed.
pub(super) fn workaround_bound(r: &ProductionRecord, issue: &Value) -> bool {
    let h = &issue["executionHandling"];
    r.policy["implementationExecution"]["mode"] == "autonomous"
        && matches!(
            issue["repairTarget"].as_str(),
            Some("abya-source" | "abya-build")
        )
        && h["issueFingerprint"] == issue_fingerprint(issue)
        && h["mode"] == "workaround"
        && h["scope"] == "equivalent"
        && r.current_version
            .as_ref()
            .is_some_and(|v| h["verifiedVersion"] == *v)
        && h["verifiedCycle"] == r.cycle
        && h["verifiedScope"] == scope(r)
        && h["evidenceIds"]
            .as_array()
            .is_some_and(|ids| !ids.is_empty())
}
pub(super) fn verify_workarounds(r: &ProductionRecord, root: &Path, stage: &str) -> AppResult<()> {
    if !["implementation", "review", "delivery", "closeout"].contains(&stage) {
        return Ok(());
    }
    for issue in r
        .issues
        .iter()
        .filter(|i| i["status"] != "resolved" && workaround_bound(r, i))
    {
        validation::evidence_refs(
            r,
            root,
            &issue["executionHandling"]["evidenceIds"],
            validation::version(r)?,
        )?;
    }
    Ok(())
}

fn issue_fingerprint(issue: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                issue["id"],
                issue["kind"],
                issue["stage"],
                issue["description"],
                issue["affectedStages"],
                issue["repairTarget"]
            ])
            .to_string()
        )
    )
}
