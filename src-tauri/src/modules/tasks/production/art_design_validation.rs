use super::art_design_sources::{design_hash, enabled, inputs_hash};
use super::*;

pub(super) fn design_bindings(
    r: &ProductionRecord,
    root: &Path,
    ids: &[Value],
) -> AppResult<Value> {
    let mut bindings = serde_json::Map::new();
    for id in ids {
        let a = r
            .artifacts
            .iter()
            .find(|a| {
                a["id"] == *id
                    && a["cycle"] == r.cycle
                    && a["plannerReference"] != true
                    && ["mockup", "render", "gameplay"]
                        .iter()
                        .any(|s| a["sourceType"] == *s)
                    && a["temporary"] != true
                    && a["status"] != "withdrawn"
            })
            .ok_or_else(|| AppError::validation("落实位置必须引用本轮正式设计或实机图。"))?;
        let latest = r.artifacts.iter().rev().find(|b| {
            b["groupId"] == a["groupId"]
                && b["cycle"] == r.cycle
                && b["sourceType"] == a["sourceType"]
        });
        if latest.is_none_or(|b| b["id"] != *id) {
            return Err(AppError::validation("要求仍关联旧图，请核对最新方案。"));
        }
        let path = mutation::text(a, "path")?;
        let hash = files::hash_file(&files::safe_path(root, path)?)?.0;
        if a["sha256"] != hash {
            return Err(AppError::validation("设计文件已变化，请重新登记。"));
        }
        bindings.insert(path.into(), json!(hash));
    }
    Ok(Value::Object(bindings))
}

pub(super) fn gate(
    r: &ProductionRecord,
    root: &Path,
    kind: &str,
    scope: Option<&str>,
) -> AppResult<()> {
    if !enabled(r) || !["resources", "plan", "delivery", "implementation"].contains(&kind) {
        return Ok(());
    }
    let a = &r.art_design;
    for s in &a.sources {
        let review = a
            .source_reviews
            .iter()
            .find(|x| x["id"] == s["id"] && x["sourceHash"] == s["hash"])
            .ok_or_else(|| AppError::validation(format!("尚未核对策划输入：{}。", s["id"])))?;
        for id in review["requirementIds"].as_array().into_iter().flatten() {
            if !a
                .requirements
                .iter()
                .any(|q| q["id"] == *id && q["withdrawn"] != true && q["sourceHash"] == s["hash"])
            {
                return Err(AppError::validation(
                    "来源核对关联失效，请重新检查有效要求。",
                ));
            }
        }
    }
    for reference in r
        .artifacts
        .iter()
        .filter(|x| x["plannerReference"] == true && x["status"] != "withdrawn")
    {
        if !a
            .reference_analyses
            .iter()
            .any(|x| x["id"] == reference["id"] && x["sha256"] == reference["sha256"])
        {
            return Err(AppError::validation(
                "还有策划参考图未说明借鉴内容、应用位置和取舍。",
            ));
        }
        let current =
            files::hash_file(&files::safe_path(root, mutation::text(reference, "path")?)?)?.0;
        if reference["sha256"] != current {
            return Err(AppError::validation("策划参考图已变化。"));
        }
    }
    let input = inputs_hash(r);
    if a.direction["inputHash"] != input {
        return Err(AppError::validation(
            "请根据最新策划输入完善游戏体验、视觉规则、资产规格和能力预检。",
        ));
    }
    let scope = scope.or_else(|| {
        r.documents
            .get("resources")
            .and_then(|d| d.art_scope.as_deref())
    });
    for q in a.requirements.iter().filter(|q| q["withdrawn"] != true) {
        if !a
            .sources
            .iter()
            .any(|s| s["id"] == q["sourceId"] && s["hash"] == q["sourceHash"])
        {
            return Err(AppError::validation(format!(
                "要求 {} 来源已变化；保留历史并核对策划最新决定。",
                q["id"]
            )));
        }
        if q["kind"] == "clarify" {
            return Err(AppError::validation(format!("要求 {} 仍待澄清。", q["id"])));
        }
        let c = a
            .coverage
            .iter()
            .find(|c| c["id"] == q["id"] && c["inputHash"] == input && c["status"] == "addressed")
            .ok_or_else(|| {
                AppError::validation(format!("要求 {} 尚无当前方案的落实说明。", q["id"]))
            })?;
        let ids = super::art_design::list(c, "artifactIds", 32)?;
        if scope == Some("new-art") && ids.is_empty() {
            return Err(AppError::validation(format!(
                "要求 {} 需要关联实际设计图/状态图。",
                q["id"]
            )));
        }
        if c["bindings"] != design_bindings(r, root, &ids)? {
            return Err(AppError::validation("要求关联的设计版本已变化。"));
        }
        if ["implementation", "delivery"].contains(&kind) {
            let realized = a
                .realizations
                .iter()
                .find(|x| {
                    x["id"] == q["id"]
                        && x["status"] == "matched"
                        && x["designHash"] == design_hash(r)
                        && x["version"] == json!(r.current_version)
                        && x["cycle"] == r.cycle
                })
                .ok_or_else(|| {
                    AppError::validation(format!("要求 {} 尚无当前作品的还原对照。", q["id"]))
                })?;
            validation::evidence_refs(r, root, &realized["evidenceIds"], validation::version(r)?)?;
        }
    }
    Ok(())
}
