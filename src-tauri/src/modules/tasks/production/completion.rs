use super::autonomy::WorkUnit;
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["implementationExecution"]["completionLoopVersion"] == 1
}
pub(super) fn check_signature(r: &ProductionRecord) -> Value {
    let mut checks = BTreeMap::new();
    for t in r
        .self_tests
        .iter()
        .filter(|t| t["version"].as_str() == r.current_version.as_deref() && t["cycle"] == r.cycle)
    {
        if let Some(id) = t["id"].as_str() {
            checks.insert(
                id,
                json!([t["status"], t["evidenceIds"], t["blockedByIssueIds"]]),
            );
        }
    }
    json!(checks)
}
pub(super) fn relevant(r: &ProductionRecord) -> BTreeSet<String> {
    let scope = autonomy::scope(r);
    let mut ids: BTreeSet<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| {
            u.cycle == r.cycle
                && (!super::execution_loop::enabled(r) || u.status != "superseded")
                && u.scope_hash == scope
                && (!enabled(r) || Some(&u.goal_id) == r.workbench.active_goal_id.as_ref())
        })
        .map(|u| u.id.clone())
        .collect();
    for _ in 0..r.workbench.execution.units.len() {
        let before = ids.len();
        for u in &r.workbench.execution.units {
            if ids.contains(&u.id) {
                ids.extend(u.depends_on.clone());
            }
        }
        if before == ids.len() {
            break;
        }
    }
    ids
}
pub(super) fn signature(r: &ProductionRecord) -> String {
    let ids = relevant(r);
    let units: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| {
            ids.contains(&u.id) && !matches!(u.kind.as_str(), "finalization" | "repair-plan")
        })
        .map(|u| json!([u.id, u.definition_revision, u.result_hash]))
        .collect();
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                autonomy::scope(r),
                r.workbench.active_goal_id,
                r.current_version,
                r.version_details["archiveHash"],
                r.workbench.execution.condition_signals,
                completion::check_signature(r),
                units
            ])
            .to_string()
        )
    )
}
pub(super) fn final_id(r: &ProductionRecord) -> String {
    let goal = r.workbench.active_goal_id.as_deref().unwrap_or("");
    format!(
        "final-{}-{:x}",
        r.cycle,
        Sha256::digest(format!("{}:{goal}:{}", autonomy::scope(r), signature(r)))
    )
}
pub(super) fn blocked_state(state: &str) -> bool {
    matches!(state, "repairRequired" | "externalBlocked")
}
pub(super) fn is_repairable(u: &WorkUnit) -> bool {
    !matches!(u.kind.as_str(), "repair-plan" | "finalization")
        && u.status == "deferred"
        && matches!(u.defer_reason_code.as_str(), "" | "repairable")
}
/// One diagnostic dispatch for each actual failed result/definition; a summary rewrite cannot buy another.
pub(super) fn ensure_repair(r: &mut ProductionRecord) {
    if super::execution_loop::enabled(r) {
        return;
    }
    let ids = relevant(r);
    let scope = autonomy::scope(r);
    let target = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| {
            ids.contains(&u.id)
                && (is_repairable(u)
                    || u.status == "awaiting-condition" && u.defer_reason_code == "unknown-result")
                && !super::autonomy_budget::exhausted(r, u)
                && !r.workbench.execution.units.iter().any(|p| {
                    p.repair_for == u.id
                        && p.input_signature
                            == format!(
                                "{}:{}:{}",
                                u.definition_revision,
                                u.result_hash,
                                autonomy::unit_condition(r, u)
                            )
                })
        })
        .cloned();
    if let Some(u) = target {
        if r.workbench.execution.units.len() >= 256 {
            return;
        }
        let signature = format!(
            "{}:{}:{}",
            u.definition_revision,
            u.result_hash,
            autonomy::unit_condition(r, &u)
        );
        let id = format!(
            "repair-{:x}",
            Sha256::digest(format!("{scope}:{}:{signature}", u.id))
        );
        r.workbench.execution.units.push(WorkUnit{ id,kind:"repair-plan".into(),scope_hash:scope,cycle:r.cycle,
            goal_id:r.workbench.active_goal_id.clone().unwrap_or_default(),title:format!("诊断并修复：{}",u.title),
            repair_for:u.id,input_signature:signature,issue_ids:u.issue_ids,status:"pending".into(),
            routes:vec!["读取实际失败证据；Lua/接线问题设计新的可验证修复，使用revise-work-unit修改原项的路线与依赖后交还执行。未知写入先只读核对原请求和实际目标，用reconcile-work-unit提供原始读回回执与实际不同的剩余路线；不得重放原写入。只有确实外部缺失、未知结果或预算耗尽才用classify-work-unit记录证据和下一条件；不能仅写总结后结束。优先接通真实输入→状态变化→画面→保存重载。".into()],updated_at:now(),..Default::default()});
    }
}
pub(super) fn validate_defer(data: &Value, status: &str) -> AppResult<()> {
    if !matches!(status, "deferred" | "awaiting-condition") {
        return Ok(());
    }
    let code = mutation::text(data, "deferReasonCode")?;
    if ![
        "repairable",
        "dependency",
        "external-condition",
        "unknown-result",
        "budget-exhausted",
    ]
    .contains(&code)
    {
        return Err(AppError::validation("需要明确暂缓类型。"));
    }
    if code == "external-condition" || code == "unknown-result" {
        mutation::text(data, "nextCondition")?;
        if data["resultPaths"].as_array().is_none_or(Vec::is_empty) {
            return Err(AppError::validation("外部条件或未知结果须附实际证据文件。"));
        }
    }
    Ok(())
}
pub(super) fn migrate(r: &mut ProductionRecord) {
    if r.current_stage != "implementation" {
        return;
    }
    let ids = relevant(r);
    let states: BTreeMap<_, _> = r
        .workbench
        .execution
        .units
        .iter()
        .map(|u| (u.id.clone(), u.status.clone()))
        .collect();
    for u in &mut r.workbench.execution.units {
        if !ids.contains(&u.id) || !u.defer_reason_code.is_empty() {
            continue;
        }
        if u.status == "awaiting-condition" {
            u.defer_reason_code = "unknown-result".into();
        } else if u.status == "deferred" {
            u.defer_reason_code = if u
                .depends_on
                .iter()
                .any(|d| states.get(d).is_none_or(|s| s != "succeeded"))
            {
                "dependency"
            } else {
                "repairable"
            }
            .into();
        }
    }
    r.workbench.execution.state = "paused".into();
    r.workbench.execution.active_unit_id = None;
    autonomy::event(
        &mut r.workbench.execution,
        "completion-policy-migrated",
        None,
        "保留原工作、批准和预算；恢复时只重评当前目标及依赖，先形成真实可玩检查点。",
    );
}
pub(super) fn record_signal(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    if !enabled(r) {
        return Err(AppError::validation("请先升级自动完成策略。"));
    }
    let id = mutation::text(data, "unitId")?;
    mutation::text(data, "reason")?;
    if !relevant(r).contains(id) {
        return Err(AppError::validation("工作项不在当前目标依赖范围。"));
    }
    let paths = data["paths"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 16)
        .ok_or_else(conflict)?;
    let mut hashes = BTreeMap::new();
    for p in paths {
        let p = p.as_str().ok_or_else(conflict)?;
        hashes.insert(p, files::hash_file(&files::safe_path(root, p)?)?.0);
    }
    r.workbench
        .execution
        .condition_signals
        .insert(id.into(), json!(hashes));
    Ok(())
}
impl TaskService {
    /// Explicit APP resume can request a bounded diagnostic without clearing historical budgets or unknown writes.
    pub fn production_resume_completion(
        &self,
        task: &str,
        goal: &str,
        intent: &str,
    ) -> AppResult<()> {
        let mut r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if r.workbench.active_goal_id.as_deref() != Some(goal) {
            return Err(conflict());
        }
        if super::execution_loop::enabled(&r) {
            return self.production_resume_in_place(task);
        }
        let mut probe = r.clone();
        if super::progress_budget::reassess(&mut probe) {
            self.production_commit(task, r.revision, |r| {
                super::progress_budget::reassess(r);
                Ok(())
            })?;
            r = self.production_get(task)?.record.ok_or_else(conflict)?;
        }
        if !enabled(&r) || !blocked_state(&r.workbench.execution.state) {
            return Ok(());
        }
        if r.workbench.active_goal_id.as_deref() != Some(goal) {
            return Err(conflict());
        }
        if super::progress_budget::enabled(&r)
            && r.workbench.execution.units.iter().any(|u| {
                relevant(&r).contains(&u.id)
                    && u.status == "pending"
                    && u.depends_on.iter().all(|id| {
                        r.workbench
                            .execution
                            .units
                            .iter()
                            .any(|d| &d.id == id && d.status == "succeeded")
                    })
            })
        {
            return self.production_commit(task, r.revision, |r| {
                r.workbench.execution.state = "ready".into();
                Ok(())
            });
        }
        let key = format!("manual:{}", signature(&r));
        let id = format!("diagnose-{intent}");
        if r.workbench.execution.units.iter().any(|u| u.id == id) {
            return Ok(());
        }
        if r.workbench
            .execution
            .units
            .iter()
            .filter(|u| u.input_signature == key)
            .count()
            >= 2
        {
            return Err(AppError::validation(
                "相同成果已做两次诊断；请提供新的代码、能力证据或外部条件，预算和未知写入不会被重置。",
            ));
        }
        self.production_commit(task,r.revision,|r|{
            if r.workbench.execution.units.len()>=256{return Err(AppError::validation("工作项容量已满。"));}
            let scope=autonomy::scope(r);
            r.workbench.execution.units.push(WorkUnit{id,kind:"repair-plan".into(),goal_id:goal.into(),cycle:r.cycle,scope_hash:scope,
                input_signature:key,title:"核对恢复条件并继续原目标".into(),status:"pending".into(),
                routes:vec!["先核对原结果、候选及未知写入。用实际新证据record-work-signal，只在不同且已授权的修复路线成立时revise-work-unit；预算保留，未知写入不重放。没有新条件时列出具体缺口与下一条件，不新建继续目标或重跑旧实验。".into()],updated_at:now(),..Default::default()});
            r.workbench.execution.state="ready".into();Ok(())
        })
    }
}
