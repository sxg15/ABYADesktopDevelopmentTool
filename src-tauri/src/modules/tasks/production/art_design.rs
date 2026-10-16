use super::art_design_sources::{design_hash, enabled, inputs_hash, invalidate};
use super::*;

pub(super) fn text(v: &Value, key: &str, max: usize) -> AppResult<String> {
    let s = mutation::text(v, key)?;
    if s.chars().count() > max {
        return Err(AppError::validation(format!("{key} 最多 {max} 字。")));
    }
    Ok(s.into())
}
pub(super) fn list(v: &Value, key: &str, max: usize) -> AppResult<Vec<Value>> {
    let a = v[key]
        .as_array()
        .filter(|a| a.len() <= max)
        .ok_or_else(|| AppError::validation(format!("{key} 必须为最多 {max} 项的数组。")))?;
    if a.iter()
        .any(|x| x.as_str().is_none_or(|s| s.is_empty() || s.len() > 200))
    {
        return Err(AppError::validation(format!("{key} 必须包含有效 ID。")));
    }
    let mut unique = std::collections::BTreeSet::new();
    if !a.iter().all(|x| unique.insert(x.to_string())) {
        return Err(AppError::validation("ID 不能重复。"));
    }
    Ok(a.clone())
}
pub(super) fn put(rows: &mut Vec<Value>, item: Value) -> AppResult<()> {
    if let Some(old) = rows.iter_mut().find(|x| x["id"] == item["id"]) {
        *old = item;
    } else if rows.len() < 500 {
        rows.push(item);
    } else {
        return Err(AppError::validation("设计记录超过500项，请缩小本轮范围。"));
    }
    Ok(())
}

fn requirement(r: &mut ProductionRecord, data: &Value, human: bool) -> AppResult<()> {
    let s = source(r, data)?;
    let id = text(data, "id", 100)?;
    let quote = data["quote"].as_str().unwrap_or("");
    let raw = s["text"].as_str().unwrap_or("");
    if (quote.trim().is_empty() && !raw.trim().is_empty()) || !raw.contains(quote) {
        return Err(AppError::validation(
            "请保留来源中的完整原句；图片要求保留图片来源并另写解读。",
        ));
    }
    let kind = text(data, "kind", 32)?;
    if !["must", "preference", "prohibited", "clarify"].contains(&kind.as_str()) {
        return Err(AppError::validation(
            "要求性质须为 must/preference/prohibited/clarify。",
        ));
    }
    if !human
        && let Some(old) = r.art_design.requirements.iter().find(|x| x["id"] == id)
        && (old["quote"] != quote
            || old["kind"] != kind
            || old["sourceId"] != s["id"]
            || old["withdrawn"] == true)
    {
        return Err(AppError::validation(
            "AI 不能改写原话、降级或撤回既有要求；请新增要求或请策划明确修订。",
        ));
    }
    let mut item = json!({"id":id,"sourceId":s["id"],"sourceHash":s["hash"],"quote":quote,
        "sourceText":raw,"sourceAsset":s["asset"],"kind":kind,"withdrawn":false});
    for key in ["interpretation", "scope", "acceptance"] {
        item[key] = json!(text(data, key, 4000)?);
    }
    put(&mut r.art_design.requirements, item)
}

fn review_source(r: &mut ProductionRecord, data: &Value) -> AppResult<()> {
    let s = source(r, data)?;
    let ids = list(data, "requirementIds", 500)?;
    for id in &ids {
        if !r.art_design.requirements.iter().any(|q| {
            q["id"] == *id
                && q["sourceId"] == s["id"]
                && q["sourceHash"] == s["hash"]
                && q["withdrawn"] != true
        }) {
            return Err(AppError::validation(
                "来源核对必须关联该来源当前有效的要求。",
            ));
        }
    }
    let reason = text(data, "reason", 4000)?;
    put(
        &mut r.art_design.source_reviews,
        json!({"id":s["id"],"sourceHash":s["hash"],
        "requirementIds":ids,"reason":reason}),
    )
}

fn reference(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let id = text(data, "artifactId", 200)?;
    let a = r
        .artifacts
        .iter()
        .find(|a| a["id"] == id && a["sourceType"] == "reference" && a["status"] != "withdrawn")
        .ok_or_else(|| AppError::validation("请选择有效参考图。"))?;
    if a["sha256"] != files::hash_file(&files::safe_path(root, mutation::text(a, "path")?)?)?.0 {
        return Err(AppError::validation("参考图片已变化。"));
    }
    let role = text(data, "role", 32)?;
    if !["overall", "asset", "layout", "mood", "negative"].contains(&role.as_str()) {
        return Err(AppError::validation("参考用途无效。"));
    }
    let mut item = json!({"id":id,"sha256":a["sha256"],"role":role});
    for key in [
        "observations",
        "borrow",
        "avoid",
        "application",
        "conflicts",
    ] {
        item[key] = json!(text(data, key, 4000)?);
    }
    put(&mut r.art_design.reference_analyses, item)
}

fn coverage(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let id = text(data, "requirementId", 100)?;
    if !r
        .art_design
        .requirements
        .iter()
        .any(|q| q["id"] == id && q["withdrawn"] != true)
    {
        return Err(AppError::validation("要求不存在或已撤回。"));
    }
    let ids = list(data, "artifactIds", 32)?;
    let bindings = super::art_design_validation::design_bindings(r, root, &ids)?;
    let note = text(data, "designLocation", 4000)?;
    let status = text(data, "status", 32)?;
    if !["addressed", "conflict", "not-designed"].contains(&status.as_str()) {
        return Err(AppError::validation("设计落实状态无效。"));
    }
    let input_hash = inputs_hash(r);
    put(
        &mut r.art_design.coverage,
        json!({"id":id,"artifactIds":ids,"bindings":bindings,
        "designLocation":note,"status":status,"inputHash":input_hash}),
    )
}

fn realization(r: &mut ProductionRecord, root: &Path, data: &Value) -> AppResult<()> {
    let id = text(data, "requirementId", 100)?;
    if !r
        .art_design
        .requirements
        .iter()
        .any(|q| q["id"] == id && q["withdrawn"] != true)
    {
        return Err(AppError::validation("要求不存在或已撤回。"));
    }
    let ids = list(data, "evidenceIds", 32)?;
    validation::evidence_refs(r, root, &json!(ids), validation::version(r)?)?;
    if !r
        .evidence
        .iter()
        .any(|e| ids.iter().any(|id| *id == e.id) && ["image", "video"].contains(&e.kind.as_str()))
    {
        return Err(AppError::validation(
            "美术还原至少需要一项实际查看的图片或视频证据。",
        ));
    }
    let target = text(data, "target", 2000)?;
    let comparison = text(data, "comparison", 4000)?;
    let differences = text(data, "differences", 4000)?;
    let status = text(data, "status", 32)?;
    if !["matched", "mismatch", "blocked"].contains(&status.as_str()) {
        return Err(AppError::validation("还原状态无效。"));
    }
    let design_hash = design_hash(r);
    put(
        &mut r.art_design.realizations,
        json!({"id":id,"evidenceIds":ids,"target":target,
        "comparison":comparison,"differences":differences,"status":status,
        "designHash":design_hash,"version":r.current_version,"cycle":r.cycle}),
    )
}

fn case_draft(r: &mut ProductionRecord, data: &Value) -> AppResult<()> {
    let id = text(data, "id", 100)?;
    let ids = list(data, "artifactIds", 16)?;
    let references = if data.get("referenceIds").is_some() {
        list(data, "referenceIds", 16)?
    } else {
        vec![]
    };
    if ids.len() + references.len() > 16
        || references.iter().any(|id| {
            !r.artifacts.iter().any(|a| {
                a["id"] == *id && a["sourceType"] == "reference" && a["status"] != "withdrawn"
            })
        })
    {
        return Err(AppError::validation(
            "案例可包含有效参考，但参考与成果合计最多16个。",
        ));
    }
    if ids.is_empty()
        || ids.iter().any(|id| {
            !r.artifacts.iter().any(|a| {
                a["id"] == *id
                    && a["plannerReference"] != true
                    && a["sourceType"] != "reference"
                    && a["sourceType"] != "feedback"
                    && a["temporary"] != true
                    && a["cycle"] == r.cycle
                    && a["status"] != "withdrawn"
            })
        })
    {
        return Err(AppError::validation(
            "案例须选择本轮正式设计/实机成果，不能收录参考或临时素材。",
        ));
    }
    let mut item = json!({"id":id,"artifactIds":ids,"referenceIds":references,"designHash":design_hash(r),"cycle":r.cycle,
        "parentCaseId":data["parentCaseId"],"parentVersion":data["parentVersion"]});
    for key in [
        "title",
        "context",
        "plannerRequirements",
        "visualRules",
        "decisions",
        "production",
        "applicability",
        "limitations",
        "sources",
    ] {
        item[key] = json!(text(data, key, 8000)?);
    }
    let tags = list(data, "tags", 20)?;
    item["tags"] = json!(tags);
    put(&mut r.art_design.case_drafts, item)
}
fn source(r: &ProductionRecord, data: &Value) -> AppResult<Value> {
    r.art_design
        .sources
        .iter()
        .find(|s| s["id"] == data["sourceId"] && s["hash"] == data["sourceHash"])
        .cloned()
        .ok_or_else(|| AppError::validation("要求来源已变化，请读取当前原话后重试。"))
}

pub(super) fn apply(
    r: &mut ProductionRecord,
    root: &Path,
    op: &str,
    data: &Value,
    human: bool,
) -> AppResult<()> {
    if !enabled(r) {
        return Err(AppError::validation(
            "请通过 APP 显式升级到美术设计流程 2.2。",
        ));
    }
    let before = design_hash(r);
    match op {
        "record-art-input" => {
            let id = text(data, "id", 100)?;
            let original = text(data, "text", 8000)?;
            let locator = text(data, "locator", 1000)?;
            let item = json!({"id":id,"text":original,"locator":locator});
            if let Some(old) = r
                .art_design
                .conversation_inputs
                .iter()
                .find(|x| x["id"] == id)
            {
                if *old != item {
                    return Err(AppError::validation("已摘录原话不可覆盖，请追加新输入。"));
                }
            } else {
                put(&mut r.art_design.conversation_inputs, item)?;
            }
        }
        "add-art-note" if human => {
            let note = text(data, "text", 8000)?;
            if r.art_design.planner_notes.len() >= 200 {
                return Err(AppError::validation("补充要求过多。"));
            }
            r.art_design
                .planner_notes
                .push(json!({"id":uuid::Uuid::new_v4(),"text":note,"at":now()}));
        }
        "withdraw-art-requirement" if human => {
            let reason = text(data, "reason", 2000)?;
            let row = r
                .art_design
                .requirements
                .iter_mut()
                .find(|x| x["id"] == data["id"])
                .ok_or_else(|| AppError::validation("要求不存在。"))?;
            row["withdrawn"] = json!(true);
            row["withdrawReason"] = json!(reason);
            row["withdrawnAt"] = json!(now());
        }
        "save-art-requirement" => requirement(r, data, human)?,
        "review-art-source" => review_source(r, data)?,
        "analyze-art-reference" => reference(r, root, data)?,
        "save-art-direction" => {
            let mut direction = json!({"inputHash":inputs_hash(r)});
            for key in [
                "playerExperience",
                "tasksAndInformation",
                "platformAndCamera",
                "visualRules",
                "representativeScene",
                "assetProduction",
                "feasibility",
                "acceptance",
                "librarySearch",
            ] {
                direction[key] = json!(text(data, key, 8000)?);
            }
            r.art_design.direction = direction;
        }
        "save-art-coverage" => coverage(r, root, data)?,
        "save-art-realization" => realization(r, root, data)?,
        "save-art-case-draft" => case_draft(r, data)?,
        _ => return Err(AppError::validation("无效美术设计操作。")),
    }
    if before != design_hash(r) {
        invalidate(r);
    }
    Ok(())
}
