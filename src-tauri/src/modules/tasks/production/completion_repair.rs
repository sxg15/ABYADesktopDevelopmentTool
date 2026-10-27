use super::*;
use sha2::{Digest, Sha256};
pub(super) fn apply(
    r: &mut ProductionRecord,
    root: &Path,
    op: &str,
    data: &Value,
) -> AppResult<()> {
    if !completion::enabled(r) {
        return Err(AppError::validation("请先升级任务的自动完成策略。"));
    }
    let id = mutation::text(data, "id")?;
    let reason = mutation::text(data, "reason")?;
    let ids = completion::relevant(r);
    let old = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| u.id == id && ids.contains(id))
        .cloned()
        .ok_or_else(conflict)?;
    if matches!(old.status.as_str(), "running" | "succeeded" | "superseded") {
        return Err(AppError::validation("不能修改运行中或已完成工作项。"));
    }
    if old.definition_revision as u64
        != data["expectedDefinitionRevision"]
            .as_u64()
            .unwrap_or(u64::MAX)
    {
        return Err(conflict());
    }
    let paths = data["resultPaths"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 16)
        .ok_or_else(|| AppError::validation("修订须引用实际诊断文件。"))?;
    let mut evidence_hashes = BTreeMap::new();
    for p in paths {
        let p = p.as_str().ok_or_else(conflict)?;
        evidence_hashes.insert(p, files::hash_file(&files::safe_path(root, p)?)?.0);
    }
    let mut readback_hashes = BTreeMap::new();
    let mut next = old.clone();
    let reconciliation = op == "reconcile-work-unit";
    if reconciliation {
        if old.defer_reason_code != "unknown-result" && old.status != "awaiting-condition" {
            return Err(AppError::validation(
                "只有待对账工作使用此入口；普通修复用修订接口。",
            ));
        }
        let proof = &data["reconciliation"];
        if !["applied", "not-executed", "partial"]
            .iter()
            .any(|v| proof["outcome"] == *v)
        {
            return Err(AppError::validation("需明确原操作的对账结论。"));
        }
        let operations = proof["operationIds"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 16)
            .ok_or_else(conflict)?;
        if operations.iter().any(|v| {
            v.as_str().is_none_or(|s| {
                s.is_empty()
                    || s.len() > 128
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
        }) {
            return Err(conflict());
        }
        let readbacks = proof["readbackPaths"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 16)
            .ok_or_else(|| AppError::validation("对账需原始成功读回回执，说明文档不能替代。"))?;
        for p in readbacks {
            let path = files::safe_path(root, p.as_str().ok_or_else(conflict)?)?;
            if std::fs::metadata(&path)?.len() > 16 * 1024 * 1024 {
                return Err(AppError::validation("读回回执过大。"));
            }
            let raw = std::fs::read(path)?;
            let v: Value =
                serde_json::from_slice(raw.strip_prefix(&[239, 187, 191]).unwrap_or(&raw))?;
            if v["success"] != true || v["requestId"].as_str().is_none_or(str::is_empty) {
                return Err(AppError::validation("对账材料须保留成功读回及请求身份。"));
            }
            readback_hashes.insert(p.as_str().unwrap(), format!("{:x}", Sha256::digest(&raw)));
        }
    }
    if op == "classify-work-unit" {
        completion::validate_defer(data, "deferred")?;
        let code = mutation::text(data, "deferReasonCode")?;
        if code == "budget-exhausted" && !autonomy_budget::exhausted(r, &old) {
            return Err(AppError::validation("实际预算尚未耗尽。"));
        }
        if old.defer_reason_code == "unknown-result" && code != "unknown-result" {
            return Err(AppError::validation(
                "未知写入保留对账状态，不以分类修改代替对账。",
            ));
        }
        next.defer_reason_code = code.into();
        next.next_condition = data["nextCondition"].as_str().unwrap_or("").into();
        next.status = if code == "unknown-result" {
            "awaiting-condition"
        } else {
            "deferred"
        }
        .into();
    } else {
        if !reconciliation
            && (old.defer_reason_code == "unknown-result" || old.status == "awaiting-condition")
        {
            return Err(AppError::validation(
                "先核对未知结果或外部条件；不能直接重置派发。",
            ));
        }
        if autonomy_budget::exhausted(r, &old) {
            return Err(AppError::validation(
                "实际工作或关联恢复预算已耗尽，修订不能重置额度。",
            ));
        }
        let strings = |key: &str, max: usize| -> AppResult<Vec<String>> {
            let a = data[key]
                .as_array()
                .filter(|a| a.len() <= max)
                .ok_or_else(conflict)?;
            a.iter()
                .map(|v| {
                    v.as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 1000)
                        .map(str::to_owned)
                        .ok_or_else(conflict)
                })
                .collect()
        };
        next.routes = strings("routes", 4)?;
        next.depends_on = strings("dependsOn", 64)?;
        if next.routes.is_empty()
            || (next.routes == old.routes && next.depends_on == old.depends_on)
        {
            return Err(AppError::validation("修订必须包含实际不同的路线或依赖。"));
        }
        if next.depends_on.iter().any(|d| d == id || !ids.contains(d)) {
            return Err(AppError::validation("依赖必须属于当前目标的相关图。"));
        }
        let mut pending = next.depends_on.clone();
        let mut seen = std::collections::BTreeSet::new();
        while let Some(d) = pending.pop() {
            if d == id {
                return Err(AppError::validation("工作依赖不能形成环。"));
            }
            if seen.insert(d.clone())
                && let Some(u) = r.workbench.execution.units.iter().find(|u| u.id == d)
            {
                pending.extend(u.depends_on.clone());
            }
        }
        next.status = "pending".into();
        next.route = 0;
        next.attempts = 0;
        next.route_elapsed_ms = 0;
        next.defer_reason_code.clear();
        next.next_condition.clear();
        next.definition_revision += 1;
        next.validation_outcome.clear();
        next.validation_failures.clear();
    }
    if next.definition_history.len() >= 32 {
        return Err(AppError::validation(
            "工作项修订历史已达上限，请检查重复规划。",
        ));
    }
    next.definition_history.push(json!({"at":now(),"revision":old.definition_revision,"routes":old.routes,"dependsOn":old.depends_on,"status":old.status,"resultPaths":old.result_paths,"resultHash":old.result_hash,"elapsedMs":old.elapsed_ms,"reason":reason,"evidencePaths":paths}));
    if reconciliation {
        next.definition_history.last_mut().unwrap()["reconciliation"] =
            data["reconciliation"].clone();
    }
    if reconciliation {
        next.definition_history.last_mut().unwrap()["reconciliation"]["readbackHashes"] =
            json!(readback_hashes);
    }
    next.definition_history.last_mut().unwrap()["evidenceHashes"] = json!(evidence_hashes);
    next.reason = reason.into();
    next.updated_at = now();
    *r.workbench
        .execution
        .units
        .iter_mut()
        .find(|u| u.id == id)
        .unwrap() = next;
    autonomy::event(&mut r.workbench.execution, "work-revised", Some(id), reason);
    Ok(())
}
