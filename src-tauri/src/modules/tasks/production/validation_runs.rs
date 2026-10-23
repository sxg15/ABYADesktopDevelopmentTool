use super::*;
use sha2::{Digest, Sha256};
pub(super) fn record(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let id = mutation::text(data, "id")?;
    let version = mutation::text(data, "version")?;
    if id.len() > 100 || r.current_version.as_deref() != Some(version) {
        return Err(AppError::validation("验证批次 ID 或版本无效。"));
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
    for check in checks {
        let mut check = check.clone();
        check["runId"] = json!(id);
        super::iteration::save_test(r, root, &check)?;
    }
    for check in art {
        let mut check = check.clone();
        check["runId"] = json!(id);
        super::art_design::apply(r, root, "save-art-realization", &check, false)?;
    }
    r.workbench.execution.validation_runs.insert(id.into(),json!({"id":id,"version":version,"cycle":r.cycle,"inputHash":hash,"checks":checks,"artChecks":art,"at":now()}));
    Ok(())
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
