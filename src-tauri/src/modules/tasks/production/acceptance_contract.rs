use super::*;

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    super::execution_loop::enabled(r)
}

/// 额外检查默认可选；必要检查必须引用已批准文本或实际产品完整性问题。
pub(super) fn register(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    if !enabled(r) {
        return Err(AppError::validation("当前任务未启用验收契约。"));
    }
    validation::approved(r, root, "requirements")?;
    validation::approved(r, root, "plan")?;
    let id = mutation::text(data, "id")?;
    let expected = mutation::text(data, "expected")?;
    if id.len() > 100 || expected.len() > 4000 {
        return Err(conflict());
    }
    let basis = mutation::text(data, "basis")?;
    let required = data["required"].as_bool().unwrap_or(false);
    if !required
        && r.policy["selfTestChecks"]
            .as_array()
            .is_some_and(|checks| checks.iter().any(|c| c == id))
    {
        return Err(AppError::validation("当前基础验收不能重新登记为可选优化。"));
    }
    match basis {
        "approved-requirement" => {
            let kind = mutation::text(data, "documentKind")?;
            if !["requirements", "plan"].contains(&kind) {
                return Err(conflict());
            }
            let quote = mutation::text(data, "quote")?;
            let document = r.documents.get(kind).ok_or_else(conflict)?;
            if quote.chars().count() < 4 || !document.content.contains(quote) {
                return Err(AppError::validation(
                    "检查须引用当前已批准文档中的实际要求。",
                ));
            }
        }
        "product-integrity" => {
            let issue = mutation::text(data, "issueId")?;
            if !r
                .issues
                .iter()
                .any(|i| i["id"] == issue && i["kind"] == "defect")
            {
                return Err(AppError::validation("完整性检查须关联已登记的实际缺陷。"));
            }
        }
        "optional" if !required => {}
        _ => {
            return Err(AppError::validation(
                "验收依据必须为已批准要求、实际完整性缺陷或可选优化。",
            ));
        }
    }
    if r.workbench.execution.acceptance_contract.len() >= 128
        && !r.workbench.execution.acceptance_contract.contains_key(id)
    {
        return Err(conflict());
    }
    if r.workbench
        .execution
        .acceptance_contract
        .get(id)
        .is_some_and(|old| old["required"] == true && old["scopeHash"] == autonomy::scope(r))
        && !required
    {
        return Err(AppError::validation("不能把必要验收直接改为可选。"));
    }
    let mut value = data.clone();
    value["scopeHash"] = json!(autonomy::scope(r));
    value["recordedAt"] = json!(now());
    r.workbench
        .execution
        .acceptance_contract
        .insert(id.into(), value);
    Ok(())
}

pub(super) fn required(r: &ProductionRecord, id: &str) -> bool {
    id == "playable-input"
        || r.policy["selfTestChecks"]
            .as_array()
            .is_some_and(|v| v.iter().any(|x| x == id))
        || r.workbench
            .execution
            .acceptance_contract
            .get(id)
            .is_some_and(|c| c["required"] == true && c["scopeHash"] == autonomy::scope(r))
}

/// 不适用只针对条件性扩展；核心输入、运行和保存检查不能因此消失。
pub(super) fn not_applicable(r: &ProductionRecord, root: &Path, data: &Value) -> AppResult<bool> {
    let id = data["id"].as_str().unwrap_or("");
    if id == "authority" && r.player_mode == "single" {
        return Ok(true);
    }
    if !enabled(r)
        || [
            "architecture",
            "lua",
            "core-loop",
            "input",
            "playable-input",
            "visual",
            "lifecycle",
            "save-reload",
        ]
        .contains(&id)
    {
        return Ok(false);
    }
    let Some(c) = r.workbench.execution.acceptance_contract.get(id) else {
        return Ok(false);
    };
    if c["scopeHash"] != autonomy::scope(r) || c["required"] == true {
        return Ok(false);
    }
    mutation::text(data, "applicabilityReason")?;
    validation::evidence_refs(r, root, &data["evidenceIds"], validation::version(r)?)?;
    Ok(true)
}

pub(super) fn validate_platform(r: &ProductionRecord, data: &Value) -> AppResult<()> {
    if !enabled(r)
        || data["diagnosisStatus"] != "confirmed"
        || data["repairTarget"] != "abya-source"
    {
        return Ok(());
    }
    for key in [
        "invocation",
        "runtimeState",
        "dataVersion",
        "verificationMethod",
    ] {
        mutation::text(&data["diagnosis"]["excludedCauses"], key)?;
    }
    Ok(())
}
