use super::*;

/// 计划提交前将必要需求转换为可观察的测试；报告不授予通过状态。
pub(super) fn validate(r: &ProductionRecord, root: &Path) -> AppResult<()> {
    if r.policy["runtimeTestPlanVersion"] != 1 {
        return Ok(());
    }
    let relative = "artifacts/runtime-tests/plan.json";
    if !files::safe_path(root, relative)?.is_file() {
        return Err(AppError::validation(
            "请先按运行测试模板创建 artifacts/runtime-tests/plan.json，再提交执行计划。",
        ));
    }
    let (text, _) = files::text_file(root, relative)?;
    let plan: Value = serde_json::from_str(&text).map_err(|error| {
        AppError::validation(format!(
            "测试计划 JSON 格式无效（第{}行、第{}列）。",
            error.line(),
            error.column()
        ))
    })?;
    if plan["schema"] != "abya.test-plan/v1" {
        return Err(AppError::validation("请提供 abya.test-plan/v1 测试计划。"));
    }
    let cases = plan["cases"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 128)
        .ok_or_else(|| AppError::validation("测试计划须包含1至128个用例。"))?;
    let requirements = r.documents.get("requirements").ok_or_else(conflict)?;
    if plan["requirementsHash"] != requirements.sha256 {
        return Err(AppError::validation("测试计划须绑定当前已批准需求版本。"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for case in cases {
        let id = mutation::text(case, "id")?;
        let quote = mutation::text(case, "requirementQuote")?;
        if !ids.insert(id) || quote.chars().count() < 4 || !requirements.content.contains(quote) {
            return Err(AppError::validation(
                "测试用例须有唯一编号并引用当前需求原文。",
            ));
        }
        for key in ["precondition", "actions", "expected", "evidence"] {
            super::art_design::text(case, key, 8000)?;
        }
        if !case["roles"].as_array().is_some_and(|a| {
            !a.is_empty() && a.iter().all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        }) {
            return Err(AppError::validation("测试用例须声明实际角色。"));
        }
        if case.get("passed").is_some() || case.get("status").is_some() {
            return Err(AppError::validation(
                "测试计划不能预填通过结论；执行结果另行登记。",
            ));
        }
    }
    Ok(())
}
