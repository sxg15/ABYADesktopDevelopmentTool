use super::*;
use sha2::{Digest, Sha256};
pub(super) fn record(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let id = mutation::text(data, "id")?;
    let version = mutation::text(data, "version")?;
    let fixture = data["purpose"] == "fixture";
    let snapshot = r
        .workbench
        .execution
        .validation_snapshots
        .get(version)
        .cloned();
    if id.len() > 100
        || (!fixture && r.current_version.as_deref() != Some(version))
        || fixture && snapshot.is_none()
    {
        return Err(AppError::validation("验证批次 ID 或版本无效。"));
    }
    if fixture {
        let saved = snapshot.as_ref().unwrap();
        if saved["cycle"] != r.cycle || saved["scopeHash"] != autonomy::scope(r) {
            return Err(AppError::validation("夹具属于旧范围或周期。"));
        }
    }
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(data)?));
    if let Some(old) = r.workbench.execution.validation_runs.get(id) {
        return if old["inputHash"] == hash {
            Ok(())
        } else {
            Err(conflict())
        };
    }
    if r.workbench.execution.validation_runs.len() >= 500 {
        return Err(AppError::validation("验证批次容量已满。"));
    }
    let checks = data["checks"]
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 32)
        .ok_or_else(|| AppError::validation("需提供 1–32 项验证结论。"))?;
    let art = data["artChecks"]
        .as_array()
        .filter(|v| v.len() <= 64)
        .ok_or_else(|| AppError::validation("artChecks 必须为至多64项数组。"))?;
    let mut isolated = r.clone();
    if fixture {
        isolated.current_version = Some(version.into());
        isolated.version_details = snapshot.clone().unwrap();
    }
    for check in checks {
        let mut check = check.clone();
        check["runId"] = json!(id);
        if fixture {
            if check.get("evidenceIds").is_some() {
                validation::evidence_refs(&isolated, root, &check["evidenceIds"], version)?;
            }
            super::iteration::save_test(&mut isolated, root, &check)?;
        } else {
            super::iteration::save_test(r, root, &check)?;
        }
    }
    for check in art {
        let mut check = check.clone();
        check["runId"] = json!(id);
        if fixture {
            super::art_design::apply(&mut isolated, root, "save-art-realization", &check, false)?;
        } else {
            super::art_design::apply(r, root, "save-art-realization", &check, false)?;
        }
    }
    r.workbench.execution.validation_runs.insert(id.into(),json!({"id":id,"version":version,"purpose":if fixture{"fixture"}else{"candidate"},"scopeHash":autonomy::scope(r),"cycle":r.cycle,"inputHash":hash,"checks":checks,"artChecks":art,"at":now()}));
    if completion::enabled(r)
        && let Some(u) = r
            .workbench
            .execution
            .units
            .iter_mut()
            .find(|u| Some(&u.id) == r.workbench.execution.active_unit_id.as_ref())
    {
        for c in checks {
            if let Some(id) = c["id"].as_str() {
                if c["status"] == "failed" || c["status"] == "blocked" {
                    u.validation_failures.insert(id.into());
                } else {
                    u.validation_failures.remove(id);
                }
            }
        }
        u.validation_outcome = if u.validation_failures.is_empty() {
            "passed"
        } else {
            "failed"
        }
        .into();
    }
    Ok(())
}
impl TaskService {
    pub(super) fn production_validate_mutation_files(
        &self,
        task: &str,
        operation: &str,
        data: &Value,
    ) -> AppResult<()> {
        if operation != "record-validation-run" || data["purpose"] != "fixture" {
            return Ok(());
        }
        let r = self
            .database
            .with_connection(|c| load(c, task))?
            .ok_or_else(conflict)?;
        let version = mutation::text(data, "version")?;
        let saved = r
            .workbench
            .execution
            .validation_snapshots
            .get(version)
            .ok_or_else(|| AppError::validation("夹具版本不存在。"))?;
        versions::verify(saved)
    }
}
pub(super) fn timing(r: &ProductionRecord) -> Value {
    let mut buckets: BTreeMap<&str, i64> = BTreeMap::new();
    let mut previous = None;
    let mut state = "unknown";
    for e in &r.workbench.execution.events {
        let Some(at) = e["at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        else {
            continue;
        };
        if let Some(old) = previous {
            let elapsed = at.signed_duration_since(old).num_milliseconds().max(0);
            *buckets.entry(state).or_default() += elapsed;
        }
        match e["kind"].as_str() {
            Some("scheduled" | "resumed") => state = "observedExecution",
            Some("parked" | "converged") => state = "waitingDependencies",
            Some("paused") => state = "paused",
            _ => {}
        }
        previous = Some(at);
    }
    json!({"completedIntervalsMs":buckets,"currentState":state,"lastEventAt":previous.map(|t|t.to_rfc3339()),"note":"互斥观察区间，不是模型净工时；未观测和进程退出后的时间不计为执行。"})
}
