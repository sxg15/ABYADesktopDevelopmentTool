use super::*;

fn dependencies(r: &ProductionRecord, criterion: &str) -> AppResult<Value> {
    let rule = r
        .workbench
        .execution
        .acceptance_contract
        .get(criterion)
        .ok_or_else(conflict)?;
    if rule["scopeHash"] != autonomy::scope(r) {
        return Err(conflict());
    }
    let paths = rule["dependencies"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 128)
        .ok_or_else(|| AppError::validation("增量复验须声明该要求的实际逻辑、输入与资源依赖。"))?;
    let files = r.version_details["contentManifest"]["files"]
        .as_array()
        .ok_or_else(conflict)?;
    let mut hashes = BTreeMap::new();
    for item in paths {
        let path = item
            .as_str()
            .or_else(|| item["path"].as_str())
            .ok_or_else(conflict)?;
        let projection = item["projection"].as_str().unwrap_or("full");
        let file = files
            .iter()
            .find(|f| f["path"] == path)
            .ok_or_else(|| AppError::validation("声明的依赖不在当前冻结版本中。"))?;
        let key = match projection {
            "full" => "sha256",
            "content" => "contentSha256",
            "ui-bindings"
                if ["core-loop", "lua", "architecture", "authority", "lifecycle"]
                    .contains(&criterion) =>
            {
                "nonLayoutSha256"
            }
            _ => return Err(AppError::validation("该检查不能使用此依赖投影。")),
        };
        if !file[key].is_string() {
            return Err(AppError::validation("当前版本没有可验证的依赖投影。"));
        }
        hashes.insert(format!("{path}#{projection}"), file[key].clone());
    }
    Ok(json!(hashes))
}

pub(super) fn bind(r: &mut ProductionRecord, data: &Value) -> AppResult<()> {
    if !execution_loop::enabled(r) {
        return Ok(());
    }
    let Some(criterion) = data["checkId"].as_str() else {
        return Ok(());
    };
    let id = mutation::text(data, "id")?;
    let files = dependencies(r, criterion)?;
    r.workbench.execution.evidence_bindings.insert(id.into(),json!({
        "checkId":criterion,"files":files,"playerBuildHash":r.version_details["playerBuildHash"],
        "scopeHash":autonomy::scope(r),"version":r.current_version
    }));
    Ok(())
}

/// 只对已绑定依赖且未变化的范围复用；构建改变或未知内容影响一律要求复验。
pub(super) fn verify_reuse(r: &ProductionRecord, data: &Value) -> AppResult<()> {
    if !execution_loop::enabled(r) {
        return Ok(());
    }
    versions::verify(&r.version_details)?;
    let source = mutation::text(data, "sourceId")?;
    let binding = r
        .workbench
        .execution
        .evidence_bindings
        .get(source)
        .ok_or_else(|| AppError::validation("旧证据没有明确依赖范围，请复验或先核对适用依据。"))?;
    let id = mutation::text(binding, "checkId")?;
    if binding["scopeHash"] != autonomy::scope(r)
        || binding["playerBuildHash"] != r.version_details["playerBuildHash"]
        || binding["files"] != dependencies(r, id)?
    {
        return Err(AppError::validation(
            "批准范围、构建或检查依赖已变化，不能继承通过结果。",
        ));
    }
    if r.version_details["validationImpact"]["unknownImpact"] == true {
        return Err(AppError::validation(
            "存在无法确定影响的内容变化，应补实际复验。",
        ));
    }
    if r.version_details["validationImpact"]["mustRetest"]
        .as_array()
        .is_some_and(|checks| checks.iter().any(|v| v == id))
    {
        return Err(AppError::validation("该检查属于此次变更的必要复验范围。"));
    }
    Ok(())
}

pub(super) fn validate_check(r: &ProductionRecord, data: &Value) -> AppResult<()> {
    if !execution_loop::enabled(r) || data["status"] != "passed" {
        return Ok(());
    }
    for id in data["evidenceIds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if r.workbench
            .execution
            .evidence_bindings
            .get(id)
            .is_some_and(|b| b["checkId"] != data["id"])
        {
            return Err(AppError::validation(
                "依赖限定的证据不能替代其他检查范围的验证。",
            ));
        }
    }
    Ok(())
}
