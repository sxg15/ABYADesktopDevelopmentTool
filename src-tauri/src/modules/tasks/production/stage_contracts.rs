use super::*;

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["stageContractsVersion"] == 1
}
pub(super) fn design_stage(stage: &str) -> bool {
    matches!(stage, "resources" | "plan")
}
/// Agent issue prose cannot create a new design submission gate. Material and
/// document integrity gates remain in their existing authoritative validators.
pub(super) fn blocks(r: &ProductionRecord, i: &Value, stage: &str, operation: &str) -> bool {
    if i["status"] == "resolved" || i["kind"] == "suggestion" || !workspace::affects(i, stage) {
        return false;
    }
    if enabled(r) && design_stage(stage) {
        let gate = i["gateRef"].as_str().unwrap_or("");
        if let Some(id) = gate.strip_prefix("question:") {
            return r
                .question_groups
                .iter()
                .any(|g| g.id == id && g.stage == stage && g.status == "pending");
        }
        if operation != "submit-document"
            && let Some(id) = gate.strip_prefix("feedback:")
        {
            return r.feedback.iter().any(|f| {
                f["id"] == id
                    && f["stage"] == stage
                    && !matches!(
                        f["status"].as_str(),
                        Some("resolved" | "closed" | "withdrawn")
                    )
            });
        }
        return false;
    }
    !(["implementation", "review", "delivery", "closeout"].contains(&stage)
        && autonomy::workaround_bound(r, i))
        && i["blockedOperations"]
            .as_array()
            .is_none_or(|a| a.iter().any(|v| v == operation))
}
pub(super) fn normalize_issue(r: &ProductionRecord, data: &Value) -> AppResult<Value> {
    let mut result = data.clone();
    if !enabled(r) {
        return Ok(result);
    }
    let stage = mutation::text(data, "stage")?;
    if let Some(category) = data.get("category")
        && ![
            "agent-input",
            "contract-gap",
            "runtime-state",
            "runtime-unknown",
            "platform-confirmed",
            "design-decision",
            "unclassified",
        ]
        .iter()
        .any(|v| category == v)
    {
        return Err(AppError::validation("无效问题分类。"));
    }
    if result["category"].is_null() {
        result["category"] = json!("unclassified");
    }
    if result["category"] == "platform-confirmed" && data["diagnosisStatus"] != "confirmed" {
        return Err(AppError::validation(
            "已确认平台问题须提供当前构建的诊断及复验证据。",
        ));
    }
    if result["detectedStage"].is_null() {
        result["detectedStage"] = json!(stage);
    }
    if result["owningStage"].is_null() {
        result["owningStage"] = json!(stage);
    }
    if !r
        .stages
        .contains_key(result["owningStage"].as_str().unwrap_or(""))
    {
        return Err(AppError::validation("处理阶段无效。"));
    }
    if design_stage(stage) {
        // Persist the requested scope for audit, but evaluate only verified gates.
        result["requestedBlockedOperations"] = result["blockedOperations"].clone();
        if !blocks(r, &result, stage, "complete-stage") {
            result["blockingDisposition"] = json!("tracked-not-gating-design");
        } else {
            result["blockingDisposition"] = json!("verified-human-gate");
        }
    }
    Ok(result)
}
pub(super) fn migrate(r: &mut ProductionRecord) {
    if !authoring::has_prototype(r)
        && !r.workbench.execution.preflight.state.is_empty()
        && r.workbench.execution.preflight.state != "not-required"
    {
        let old = json!(r.workbench.execution.preflight);
        r.workbench.execution.events.push(
            json!({"kind":"design-preflight-retired","at":now(),"previous":old,
            "reason":"设计阶段不再要求运行预检；原恢复记录保留。"}),
        );
        r.workbench.execution.preflight.state = "not-required".into();
    }
    let dispositions: Vec<_> = r
        .issues
        .iter()
        .map(|i| blocks(r, i, i["stage"].as_str().unwrap_or(""), "complete-stage"))
        .collect();
    for (i, gated) in r.issues.iter_mut().zip(dispositions) {
        if matches!(i["stage"].as_str(), Some("resources" | "plan")) && i["status"] != "resolved" {
            i["blockingDisposition"] = json!(if gated {
                "verified-human-gate"
            } else {
                "tracked-not-gating-design"
            });
            i["scopeCorrection"] = json!({"reason":"阶段契约升级：运行问题不再自行阻止设计提交，原问题及验证状态保留。","at":now()});
        }
    }
    if design_stage(&r.current_stage)
        && r.stages
            .get(&r.current_stage)
            .is_some_and(|v| v == "blocked")
        && !r
            .issues
            .iter()
            .any(|i| blocks(r, i, &r.current_stage, "complete-stage"))
    {
        r.stages
            .insert(r.current_stage.clone(), "in-progress".into());
    }
}
