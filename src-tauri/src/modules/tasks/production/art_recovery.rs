use super::*;

/// 仅恢复当前周期中内容未变化、反馈已关闭的美术批准。
pub(super) fn check(r: &ProductionRecord, root: &Path) -> AppResult<()> {
    if !interaction::art_approval(r)
        || r.current_stage != "resources"
        || r.stages.get("resources").map(String::as_str) != Some("in-progress")
    {
        return Err(AppError::validation("当前无需恢复美术阶段。"));
    }
    if r.feedback.iter().any(|f| {
        (f["stage"] == "resources" || f["artifactId"].is_string()) && f["status"] != "resolved"
    }) {
        return Err(AppError::validation("请先复验并关闭所有待处理的美术反馈。"));
    }
    validation::approved(r, root, "requirements")?;
    validation::accepted_document(r, root, "resources")?;
    if r.question_groups
        .iter()
        .any(|g| g.stage == "resources" && g.status == "pending")
    {
        return Err(AppError::validation("请先回答本阶段的美术问题。"));
    }
    if r.issues
        .iter()
        .any(|i| super::stage_contracts::blocks(r, i, "resources", "complete-stage"))
    {
        return Err(AppError::validation(
            "美术阶段仍有未解决问题，请先处理并记录复验。",
        ));
    }
    Ok(())
}

/// 保留原始批准，只恢复美术及计划入口，不恢复下游实现与验收。
pub(super) fn restore(r: &mut ProductionRecord, root: &Path) -> AppResult<()> {
    check(r, root)?;
    r.stages.insert("resources".into(), "passed".into());
    r.stages.insert("plan".into(), "in-progress".into());
    r.current_stage = "plan".into();
    Ok(())
}
