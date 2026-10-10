use super::*;

pub(super) fn affects(issue: &Value, stage: &str) -> bool {
    if let Some(stages) = issue["affectedStages"].as_array() {
        stages.iter().any(|s| s == stage)
    } else {
        issue["stage"] == stage || issue["kind"] == "blocker"
    }
}

pub(super) fn stage_statuses(r: &ProductionRecord) -> BTreeMap<String, String> {
    let mut states = r.stages.clone();
    for (stage, state) in &mut states {
        if *state == "passed" || *state == "not-started" && *stage != r.current_stage {
            continue;
        }
        if r.issues
            .iter()
            .any(|i| i["status"] != "resolved" && i["kind"] != "suggestion" && affects(i, stage))
        {
            *state = "blocked".into();
        } else if r
            .question_groups
            .iter()
            .any(|g| g.stage == *stage && g.status == "pending")
        {
            *state = "waiting-for-answers".into();
        } else if *stage == r.current_stage && *state == "not-started" {
            *state = "in-progress".into();
        }
    }
    states
}

pub(super) fn artifacts(r: &ProductionRecord, root: &Path) -> Vec<Value> {
    let mut items = r.artifacts.clone();
    // Known legacy documents only: never enumerate arbitrary workspace files.
    for (stage, path, title) in [
        ("requirements", "requirements-research.md", "玩法调研"),
        ("resources", "plan-preflight.md", "资源与环境检查"),
        ("plan", "plan.md", "执行计划"),
        ("plan", "preflight.json", "能力预检记录"),
        ("closeout", "pilot-process-log.md", "制作过程记录"),
        ("closeout", "closeout.md", "制作复盘"),
    ] {
        let path = format!("{ROOT}/{path}");
        if !items.iter().any(|a| a["path"] == path)
            && !r.documents.values().any(|d| d.path == path)
            && files::safe_path(root, &path).is_ok_and(|p| p.is_file())
        {
            items.push(json!({"id":path,"path":path,"stage":stage,"title":title,"status":"draft","legacy":true}));
        }
    }
    for item in &mut items {
        item["exists"] = json!(
            item["path"]
                .as_str()
                .is_some_and(|path| files::safe_path(root, path).is_ok_and(|p| p.is_file()))
        );
    }
    items
}

pub(super) fn register_artifact(
    r: &mut ProductionRecord,
    root: &Path,
    data: &Value,
) -> AppResult<()> {
    let stage = mutation::text(data, "stage")?;
    if !r.stages.contains_key(stage) {
        return Err(AppError::validation("请选择有效制作阶段。"));
    }
    let path = mutation::text(data, "path")?;
    if r.artifacts
        .iter()
        .any(|a| a["path"] == path && a["mediaType"].is_string())
    {
        return Err(AppError::validation(
            "已登记视觉产物不能被草稿覆盖，请登记新的视觉版本。",
        ));
    }
    let full = files::safe_path(root, path)?;
    if !full.is_file() {
        return Err(AppError::validation("文件尚未保存，请保存后再登记。"));
    }
    let title = mutation::text(data, "title")?;
    if title.chars().count() > 120 {
        return Err(AppError::validation("文件标题请控制在120字以内。"));
    }
    let item = json!({"id":path,"path":path,"stage":stage,"title":title,"summary":data["summary"].as_str().unwrap_or(""),"cycle":r.cycle,"status":"draft","updatedAt":now()});
    if let Some(old) = r.artifacts.iter_mut().find(|a| a["path"] == path) {
        *old = item;
    } else {
        r.artifacts.push(item);
    }
    Ok(())
}

pub(super) fn update_stage(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let stage = mutation::text(data, "stage")?;
    if stage != r.current_stage {
        return Err(AppError::validation(
            "请先完成当前阶段，再更新下一阶段的进展。",
        ));
    }
    if matches!(
        r.stages.get(stage).map(String::as_str),
        Some("passed" | "awaiting-confirmation")
    ) {
        return Err(AppError::validation(
            "当前阶段已提交确认，请先处理文档决定。",
        ));
    }
    if !["requirements", "resources", "plan"].contains(&stage) && stage != "closeout" {
        validation::approved(r, root, "plan")?;
    }
    let summary = mutation::text(data, "summary")?;
    let status = data["status"].as_str().unwrap_or("in-progress");
    if !["in-progress", "blocked", "stopped"].contains(&status) {
        return Err(AppError::validation(
            "进展状态应为进行中、遇到问题或已暂停。",
        ));
    }
    r.stage_updates.insert(stage.into(),json!({"summary":summary,"nextAction":data["nextAction"].as_str().unwrap_or(""),"updatedAt":now()}));
    r.stages.insert(stage.into(), status.into());
    Ok(())
}

pub(super) fn accept_templates(
    r: &mut ProductionRecord,
    root: &Path,
    d: &Document,
) -> AppResult<()> {
    for (key, id) in &d.template_choices {
        super::catalog::validate_choice(root, key, id)?;
        match key.as_str() {
            "taskTemplate" if d.kind == "requirements" => r.task_template = Some(id.clone()),
            "artTemplate" if d.kind == "plan" => r.art_template = Some(id.clone()),
            _ => return Err(AppError::validation("模板选择与文档阶段不匹配。")),
        }
    }
    Ok(())
}

pub(super) fn backfill_template(
    r: &mut ProductionRecord,
    root: &Path,
    data: &Value,
) -> AppResult<()> {
    let key = mutation::text(data, "key")?;
    let kind = match key {
        "taskTemplate" => "requirements",
        "artTemplate" => "plan",
        _ => return Err(AppError::validation("请选择任务模板或美术模板。")),
    };
    validation::approved(r, root, kind)?;
    let doc = r.documents.get(kind).unwrap();
    let id = mutation::text(data, "templateId")?;
    let declared = doc
        .template_choices
        .get(key)
        .is_some_and(|choice| choice == id)
        || doc
            .content
            .lines()
            .any(|line| line.trim() == format!("{key}: {id}"));
    if data["documentHash"] != doc.sha256 || !declared {
        return Err(AppError::validation(
            "已确认文档中没有明确的模板选择记录，请随文档确认所选模板。",
        ));
    }
    super::catalog::validate_choice(root, key, id)?;
    let target = if key == "taskTemplate" {
        &mut r.task_template
    } else {
        &mut r.art_template
    };
    if target.as_ref().is_some_and(|old| old != id) {
        return Err(AppError::validation(
            "已登记其他模板，请提交文档修订以更换。",
        ));
    }
    *target = Some(id.into());
    Ok(())
}

pub(super) fn record_transition(r: &mut ProductionRecord, previous: &str) -> AppResult<()> {
    let old: ProductionRecord = serde_json::from_str(previous)?;
    if r.stages
        .get(&r.current_stage)
        .is_some_and(|s| s == "not-started")
    {
        r.stages
            .insert(r.current_stage.clone(), "in-progress".into());
    }
    let before = stage_statuses(&old);
    for (stage, status) in stage_statuses(r) {
        if before.get(&stage) != Some(&status)
            || old.current_stage != r.current_stage && stage == r.current_stage
        {
            r.events.push(json!({"stage":stage,"status":status,"at":now(),"cycle":r.cycle,"revision":r.revision+1}));
        }
    }
    Ok(())
}
