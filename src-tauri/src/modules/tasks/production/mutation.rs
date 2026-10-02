use super::{Document, Evidence, ProductionRecord, files, now, validation};
use crate::foundation::{AppError, AppResult};
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn text<'a>(v: &'a Value, key: &str) -> AppResult<&'a str> {
    v[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::validation(format!("缺少 {key}。")))
}
fn ids(v: &Value, key: &str) -> AppResult<Vec<String>> {
    v[key]
        .as_array()
        .ok_or_else(|| AppError::validation(format!("{key} 必须是数组。")))?
        .iter()
        .map(|x| {
            x.as_str()
                .map(str::to_owned)
                .ok_or_else(|| AppError::validation("证据 ID 必须为字符串。"))
        })
        .collect()
}
fn set_stage(r: &mut ProductionRecord, stage: &str, status: &str) {
    r.stages.insert(stage.into(), status.into());
    r.current_stage = stage.into();
}

pub(super) fn apply(
    r: &mut ProductionRecord,
    root: &Path,
    operation: &str,
    data: &Value,
) -> AppResult<()> {
    if !data.is_object() {
        return Err(AppError::validation("data 必须为对象。"));
    }
    match operation {
        "submit-document" => submit(r, root, data),
        "register-evidence" => evidence(r, root, data),
        "set-milestone" => {
            let name = text(data, "name")?;
            if !["R0", "R8", "R12"].contains(&name) {
                return Err(AppError::validation("无效关键版本。"));
            }
            let values = ids(data, "evidenceIds")?;
            validation::evidence_refs(r, root, &json!(values), validation::version(r)?)?;
            r.milestones.insert(name.into(), values);
            Ok(())
        }
        "save-issue" => issue(r, root, data),
        "save-round" => round(r, root, data),
        "complete-stage" => complete(r, root, data),
        "save-knowledge" => {
            let entries = data["entries"]
                .as_array()
                .filter(|x| !x.is_empty())
                .ok_or_else(|| {
                    AppError::validation("请记录采用/候选，或明确无可复用经验及原因。")
                })?;
            if entries.len() > 300 {
                return Err(AppError::validation("知识记录过多。"));
            }
            for entry in entries {
                validation::nonempty(entry, "summary")?;
                validation::nonempty(entry, "status")?;
            }
            r.knowledge = entries.clone();
            Ok(())
        }
        "configure" => {
            let mut invalidate_requirements = false;
            let mut invalidate_plan = false;
            let requirements_approved = validation::approved(r, root, "requirements").is_ok();
            let plan_approved = validation::approved(r, root, "plan").is_ok();
            if let Some(mode) = data["questionMode"].as_str() {
                if requirements_approved {
                    return Err(AppError::validation(
                        "需求已确认，请先修订需求再改变提问设置。",
                    ));
                }
                if !["ask", "no-followup"].contains(&mode) {
                    return Err(AppError::validation("无效提问模式。"));
                }
                invalidate_requirements = r.question_mode != mode;
                r.question_mode = mode.into();
            }
            for (key, target) in [
                ("taskTemplate", &mut r.task_template),
                ("artTemplate", &mut r.art_template),
            ] {
                if let Some(id) = data[key].as_str() {
                    if (key == "taskTemplate" && requirements_approved)
                        || (key == "artTemplate" && plan_approved)
                    {
                        return Err(AppError::validation(
                            "请先提交受影响的文档修订，再改变模板。",
                        ));
                    }
                    super::catalog::validate_choice(root, key, id)?;
                    if target.as_deref() != Some(id) {
                        if key == "taskTemplate" {
                            invalidate_requirements = true;
                        } else {
                            invalidate_plan = true;
                        }
                    }
                    *target = Some(id.into());
                }
            }
            if invalidate_plan && r.documents.contains_key("plan") {
                set_stage(r, "plan", "in-progress");
            }
            if invalidate_requirements && r.documents.contains_key("requirements") {
                set_stage(r, "requirements", "in-progress");
            }
            Ok(())
        }
        _ => Err(AppError::validation(
            "不支持此制作操作；人工确认只能在 APP 中完成。",
        )),
    }
}

fn submit(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let kind = text(data, "kind")?;
    if !["requirements", "plan", "delivery", "closeout"].contains(&kind) {
        return Err(AppError::validation("无效文档种类。"));
    }
    validation::document_gate(r, root, kind)?;
    if kind == "closeout" {
        validation::approved(r, root, "delivery")?;
    }
    let path = text(data, "path")?;
    if !path.ends_with(".md") {
        return Err(AppError::validation("请提交 Markdown 文档。"));
    }
    let (content, sha256) = files::text_file(root, path)?;
    if r.documents.get(kind).is_some_and(|d| {
        d.sha256 == sha256 && (kind != "delivery" || d.game_version == r.current_version)
    }) && r
        .stages
        .get(kind)
        .is_some_and(|s| ["passed", "awaiting-confirmation"].contains(&s.as_str()))
    {
        return Ok(());
    }
    let revision = r.documents.get(kind).map_or(1, |d| d.revision + 1);
    if ["requirements", "plan"].contains(&kind) && revision > 1 {
        r.cycle += 1;
        r.current_round = 0;
        r.milestones.clear();
        r.checks.clear();
        let start = if kind == "requirements" { 0 } else { 2 };
        for stage in r.policy["stages"].as_array().unwrap().iter().skip(start) {
            r.stages
                .insert(stage["id"].as_str().unwrap().into(), "not-started".into());
        }
    }
    r.documents.insert(
        kind.into(),
        Document {
            kind: kind.into(),
            revision,
            path: path.into(),
            sha256,
            content,
            game_version: r.current_version.clone(),
            submitted_at: now(),
        },
    );
    set_stage(
        r,
        kind,
        if kind == "closeout" {
            "in-progress"
        } else {
            "awaiting-confirmation"
        },
    );
    Ok(())
}

pub(super) fn set_version(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    validation::approved(r, root, "requirements")?;
    validation::approved(r, root, "plan")?;
    let id = text(data, "id")?;
    for key in ["archiveGuid", "levelGuid", "archiveHash", "playerBuildHash"] {
        text(data, key)?;
    }
    for key in ["archiveHash", "playerBuildHash"] {
        if data[key].as_str().unwrap().len() != 64
            || !data[key]
                .as_str()
                .unwrap()
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(AppError::validation("存档和 Player 标识须为实际 SHA-256。"));
        }
    }
    if r.current_version.as_deref() == Some(id) && r.version_details != *data {
        return Err(AppError::validation("版本内容改变时必须使用新的版本 ID。"));
    }
    if r.current_version.as_deref() != Some(id)
        && r.stages.get("review").is_some_and(|s| s == "passed")
    {
        for round in &mut r.rounds {
            if round["cycle"] == r.cycle && round["number"] == 12 {
                round["status"] = json!("in-progress");
                round["closedAt"] = Value::Null;
            }
        }
        set_stage(r, "review", "in-progress");
        r.stages.insert("delivery".into(), "not-started".into());
        r.stages.insert("closeout".into(), "not-started".into());
    }
    r.current_version = Some(id.into());
    r.version_details = data.clone();
    Ok(())
}

fn evidence(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let version = r
        .current_version
        .clone()
        .unwrap_or_else(|| "preflight".into());
    let id = text(data, "id")?;
    if r.evidence.iter().any(|e| e.id == id) {
        return Err(AppError::validation(
            "证据 ID 已存在，请为新证据使用新 ID。",
        ));
    }
    let kind = text(data, "kind")?;
    let capture = text(data, "captureType")?;
    if !["image", "video", "json", "text"].contains(&kind)
        || ![
            "full-cycle",
            "inspection",
            "action",
            "frames",
            "state",
            "log",
        ]
        .contains(&capture)
    {
        return Err(AppError::validation("无效证据类型。"));
    }
    let relative = text(data, "path")?;
    let path = files::safe_path(root, relative)?;
    let (sha256, bytes) = files::hash_file(&path)?;
    if bytes == 0 {
        return Err(AppError::validation("证据文件为空。"));
    }
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let valid_type = match kind {
        "image" => ["png", "jpg", "jpeg", "webp"].contains(&extension.as_str()),
        "video" => ["mp4", "mkv", "webm", "avi"].contains(&extension.as_str()),
        "json" => extension == "json",
        _ => ["txt", "log", "md"].contains(&extension.as_str()),
    };
    if !valid_type {
        return Err(AppError::validation("文件格式与证据类型不一致。"));
    }
    if kind == "video" {
        let metadata = path.parent().unwrap().join("recording.json");
        if metadata.exists() {
            let native: Value = serde_json::from_slice(&std::fs::read(metadata)?)?;
            let source_matches = native["sourceVersion"] == version
                || (version == "preflight"
                    && native["sourceVersion"].is_null()
                    && capture != "full-cycle");
            if native["schema"] == "abya.recording/v1"
                && (native["status"] != "completed"
                    || native["taskId"] != r.task_id
                    || !source_matches
                    || native["frameCount"].as_u64().unwrap_or(0) == 0)
            {
                return Err(AppError::validation(
                    "本机录像未正常结束、无帧或不属于当前任务/版本。",
                ));
            }
        }
    }
    r.evidence.push(Evidence {
        id: id.into(),
        path: relative.into(),
        sha256,
        bytes,
        kind: kind.into(),
        capture_type: capture.into(),
        version,
        reviewed: data["reviewed"].as_bool().unwrap_or(false),
        description: text(data, "description")?.into(),
        instance_id: data["instanceId"].as_str().map(str::to_owned),
        recorded_at: now(),
        cycle: r.cycle,
    });
    Ok(())
}

fn issue(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    for key in ["fix", "recheck", "resumeWhen"] {
        if !data[key].is_null() && !data[key].is_string() {
            return Err(AppError::validation(format!("{key} 必须为文字。")));
        }
    }
    let id = text(data, "id")?;
    let kind = text(data, "kind")?;
    let status = text(data, "status")?;
    let stage = text(data, "stage")?;
    text(data, "description")?;
    if !r.stages.contains_key(stage)
        || !["defect", "blocker", "checkpoint", "suggestion"].contains(&kind)
        || !["open", "resolved"].contains(&status)
    {
        return Err(AppError::validation("无效问题状态或阶段。"));
    }
    if let Some(old) = r.issues.iter().find(|i| i["id"] == id)
        && old["kind"] != "suggestion"
        && kind == "suggestion"
    {
        return Err(AppError::validation("不能把必需缺陷改为建议以绕过检查。"));
    }
    if status == "resolved" {
        for field in ["fix", "recheck"] {
            text(data, field)?;
        }
        if ["defect", "checkpoint"].contains(&kind) {
            validation::evidence_refs(r, root, &data["evidenceIds"], validation::version(r)?)?;
        }
    }
    if kind == "blocker" {
        text(data, "resumeWhen")?;
    }
    let mut value = data.clone();
    value["updatedAt"] = json!(now());
    if let Some(i) = r.issues.iter().position(|i| i["id"] == id) {
        r.issues[i] = value;
    } else {
        r.issues.push(value);
    }
    Ok(())
}

fn round(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    for field in ["checks", "questions"] {
        if let Some(items) = data.get(field) {
            let items = items
                .as_array()
                .ok_or_else(|| AppError::validation(format!("{field} 必须为数组。")))?;
            for item in items {
                text(
                    item,
                    if field == "checks" {
                        "dimensionId"
                    } else {
                        "questionId"
                    },
                )?;
                text(item, "status")?;
                for key in [
                    "observations",
                    "conclusion",
                    "counterexample",
                    "problem",
                    "change",
                    "recheck",
                ] {
                    if !item[key].is_null() && !item[key].is_string() {
                        return Err(AppError::validation(format!("{key} 必须为文字。")));
                    }
                }
            }
        }
    }
    validation::approved(r, root, "requirements")?;
    validation::approved(r, root, "plan")?;
    if r.stages.get("implementation").map(String::as_str) != Some("passed") {
        return Err(AppError::validation("先完成完整初版 R0。"));
    }
    let number = data["number"]
        .as_u64()
        .filter(|n| (1..=12).contains(n))
        .ok_or_else(|| AppError::validation("轮次须为 1—12。"))?;
    let index = r
        .rounds
        .iter()
        .position(|v| v["cycle"] == r.cycle && v["number"] == number);
    let closing = data["close"].as_bool().unwrap_or(false);
    if index.is_none() {
        if closing
            || number != u64::from(r.current_round) + 1
            || r.rounds
                .iter()
                .any(|v| v["cycle"] == r.cycle && v["status"] != "closed")
        {
            return Err(AppError::validation(
                "先开始下一轮，并关闭当前轮；不可跳轮或一次创建已完成轮次。",
            ));
        }
    } else if number != u64::from(r.current_round)
        || index.is_some_and(|i| r.rounds[i]["status"] == "closed")
    {
        return Err(AppError::validation("已关闭的轮次保留为历史，不能重写。"));
    }
    let version = validation::version(r)?.to_owned();
    let started = index
        .map(|i| r.rounds[i]["startedAt"].clone())
        .unwrap_or_else(|| json!(now()));
    let input_version = index
        .map(|i| r.rounds[i]["inputVersion"].clone())
        .unwrap_or_else(|| json!(&version));
    let mut value = data.clone();
    value.as_object_mut().unwrap().remove("close");
    value["cycle"] = json!(r.cycle);
    value["startedAt"] = started;
    value["inputVersion"] = input_version;
    value["outputVersion"] = json!(&version);
    value["status"] = json!(if closing { "closed" } else { "in-progress" });
    value["closedAt"] = if closing { json!(now()) } else { Value::Null };
    if closing {
        validation::round(r, root, &value)?;
        if [8, 12].contains(&number) {
            validation::milestone(r, root, &format!("R{number}"), &version)?;
        }
    }
    if let Some(i) = index {
        r.rounds[i] = value;
    } else {
        r.rounds.push(value);
    }
    r.current_round = number as u8;
    set_stage(
        r,
        "review",
        if closing && number == 12 {
            "passed"
        } else {
            "in-progress"
        },
    );
    if closing && number == 12 {
        r.current_stage = "delivery".into();
    }
    Ok(())
}

fn complete(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let stage = text(data, "stage")?;
    validation::clear_issues(r, stage)?;
    match stage {
        "resources" => {
            validation::approved(r, root, "requirements")?;
            text(data, "summary")?;
            set_stage(r, "resources", "passed");
            r.current_stage = "plan".into();
        }
        "implementation" => {
            super::versions::verify(&r.version_details)?;
            validation::approved(r, root, "requirements")?;
            validation::approved(r, root, "plan")?;
            let version = validation::version(r)?;
            validation::milestone(r, root, "R0", version)?;
            let checks = data["checks"]
                .as_array()
                .ok_or_else(|| AppError::validation("缺少初版校验。"))?;
            for name in ["architecture", "lua", "save-reload", "runtime"] {
                let c = checks
                    .iter()
                    .find(|c| c["id"] == name && c["status"] == "passed")
                    .ok_or_else(|| AppError::validation(format!("缺少 {name} 检查及证据。")))?;
                validation::evidence_refs(r, root, &c["evidenceIds"], version)?;
            }
            r.checks = checks.clone();
            set_stage(r, "implementation", "passed");
            r.current_stage = "review".into();
        }
        "closeout" => {
            validation::approved(r, root, "delivery")?;
            validation::document_gate(r, root, "delivery")?;
            let doc = r
                .documents
                .get("closeout")
                .ok_or_else(|| AppError::validation("缺少收尾文档。"))?;
            if files::text_file(root, &doc.path)?.1 != doc.sha256 || r.knowledge.is_empty() {
                return Err(AppError::validation("请更新收尾文档与知识分析。"));
            }
            set_stage(r, "closeout", "passed");
        }
        _ => return Err(AppError::validation("该阶段须通过对应文档确认或轮次检查。")),
    }
    Ok(())
}
