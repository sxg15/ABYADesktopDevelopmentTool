use super::*;

pub(super) fn art_approval(r: &ProductionRecord) -> bool {
    r.policy["stages"]
        .as_array()
        .is_some_and(|stages| stages.iter().any(|s| s["approval"] == "resources"))
}

pub(super) fn approve(r: &ProductionRecord, root: &Path, kind: &str) -> AppResult<()> {
    review(r, root, kind)?;
    super::art_design_validation::gate(r, root, kind, None)?;
    if kind == "resources"
        && r.feedback
            .iter()
            .any(|f| f["stage"] == "resources" && f["status"] != "resolved")
    {
        let pending = r
            .feedback
            .iter()
            .filter(|f| f["stage"] == "resources" && f["status"] != "resolved")
            .count();
        return Err(AppError::validation(format!(
            "还有 {pending} 条美术反馈未关闭。请进入“修改与复验”，确认修改结果后再批准方案；仍不满意可选择“需要修改”。"
        )));
    }
    validation::clear_issues(r, kind)
}

pub(super) fn review(r: &ProductionRecord, root: &Path, kind: &str) -> AppResult<()> {
    if !r.policy["stages"]
        .as_array()
        .is_some_and(|stages| stages.iter().any(|s| s["approval"] == kind))
    {
        return Err(AppError::validation("此成果仅供阅读，无需批准。"));
    }
    let d = r
        .documents
        .get(kind)
        .ok_or_else(|| AppError::validation("AI 尚未提交可审查成果。"))?;
    if r.stages.get(kind).map(String::as_str) != Some("awaiting-confirmation") {
        return Err(AppError::validation("此版本当前不处于待审查状态。"));
    }
    if files::text_file(root, &d.path)?.1 != d.sha256 {
        return Err(AppError::validation("文件已变化，请等待 AI 重新提交。"));
    }
    visuals::verify_bindings(d, root)?;
    current_art(r, root, d)?;
    if kind == "delivery" && d.game_version != r.current_version {
        return Err(conflict());
    }
    Ok(())
}

pub(super) fn current_art(r: &ProductionRecord, root: &Path, d: &Document) -> AppResult<()> {
    if super::art_design_sources::enabled(r)
        && ["resources", "plan", "delivery"].contains(&d.kind.as_str())
        && d.art_design_hash.as_deref() != Some(&super::art_design_sources::design_hash(r))
    {
        return Err(AppError::validation(
            "策划要求、参考或设计记录已变化，请更新并重新提交本版方案。",
        ));
    }
    if art_approval(r)
        && ["resources", "plan"].contains(&d.kind.as_str())
        && d.artifact_bindings != visuals::bindings(r, root)?
    {
        return Err(AppError::validation(
            "已有新的视觉方案，请重新提交并审查文档。",
        ));
    }
    Ok(())
}

pub(super) fn feedback(r: &ProductionRecord, root: &Path, artifact: Option<&str>) -> AppResult<()> {
    feedback_check(r, root, artifact, true)
}

pub(super) fn visual_candidate(
    r: &ProductionRecord,
    root: &Path,
    original: &str,
    candidate: &str,
) -> AppResult<()> {
    let old = r
        .artifacts
        .iter()
        .find(|a| a["id"] == original)
        .ok_or_else(|| AppError::validation("原视觉方案不存在。"))?;
    let latest = r
        .artifacts
        .iter()
        .rev()
        .find(|a| {
            a["groupId"] == old["groupId"] && a["cycle"] == r.cycle && a["sourceType"] != "feedback"
        })
        .ok_or_else(|| AppError::validation("尚无当前视觉修订。"))?;
    if latest["id"] != candidate {
        return Err(AppError::validation(
            "这条反馈尚未关联最新视觉修订，请让 AI 更新复验结果后再处理。",
        ));
    }
    let path = mutation::text(latest, "path")?;
    if latest["sha256"] != files::hash_file(&files::safe_path(root, path)?)?.0 {
        return Err(AppError::validation("视觉修订文件已变化。"));
    }
    Ok(())
}

pub(super) fn art_recheck(r: &ProductionRecord, root: &Path, feedback: &Value) -> AppResult<()> {
    if feedback["status"] != "awaiting-recheck" {
        return Err(AppError::validation(
            "AI 尚未提交这条反馈的修复结果，请等待 AI 修改并提供复验说明。",
        ));
    }
    let candidate = feedback["candidateArtifactId"]
        .as_str()
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| AppError::validation("AI 尚未提供这条反馈的复验图片，请先补齐修改结果。"))?;
    visual_candidate(r, root, mutation::text(feedback, "artifactId")?, candidate)
}

fn feedback_check(
    r: &ProductionRecord,
    root: &Path,
    artifact: Option<&str>,
    verify_version: bool,
) -> AppResult<()> {
    if let Some(id) = artifact {
        let a = r
            .artifacts
            .iter()
            .find(|a| {
                a["id"] == id
                    && a["sourceType"] != "feedback"
                    && a["plannerReference"] != true
                    && a["cycle"] == r.cycle
            })
            .ok_or_else(|| AppError::validation("没有当前周期可反馈的成果。"))?;
        let path = mutation::text(a, "path")?;
        if a["sha256"] != files::hash_file(&files::safe_path(root, path)?)?.0 {
            return Err(AppError::validation("成果文件已变化或不可读取。"));
        }
        let submitted = r.documents.values().any(|d| {
            d.artifact_bindings
                .get(path)
                .is_some_and(|hash| a["sha256"] == *hash)
                && ["passed", "awaiting-confirmation"]
                    .contains(&r.stages.get(&d.kind).map(String::as_str).unwrap_or(""))
                && files::text_file(root, &d.path).is_ok_and(|(_, hash)| hash == d.sha256)
                && visuals::verify_bindings(d, root).is_ok()
                && current_art(r, root, d).is_ok()
        });
        if !submitted {
            return Err(AppError::validation("此成果仍为草稿，尚未随文档提交审查。"));
        }
    } else {
        validation::version(r)?;
        if r.stages.get("implementation").map(String::as_str) != Some("passed") {
            return Err(AppError::validation(
                "尚无可验收作品，请等待 AI 提交完整初版。",
            ));
        }
        if verify_version {
            versions::verify(&r.version_details)?;
        } else {
            // Polling must not rehash a complete Player install every few seconds.
            for key in ["archiveSource", "playerSource"] {
                if !r.version_details[key]
                    .as_str()
                    .is_some_and(|p| Path::new(p).exists())
                {
                    return Err(AppError::validation(
                        "当前作品文件不可用，请等待 AI 重新登记版本。",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn actions(r: &ProductionRecord, root: &Path, active: bool) -> Value {
    let reason = |result: AppResult<()>| -> String {
        if !active {
            return "任务已结束或归档，恢复任务后才能操作。".into();
        }
        result.err().map(|e| e.message).unwrap_or_default()
    };
    let mut review_actions = BTreeMap::new();
    let mut approve_actions = BTreeMap::new();
    for kind in r.stages.keys() {
        review_actions.insert(kind.clone(), reason(review(r, root, kind)));
        approve_actions.insert(
            kind.clone(),
            reason(
                approve(r, root, kind)
                    .and_then(|()| {
                        if kind == "delivery" {
                            Ok(())
                        } else {
                            validation::document_gate(r, root, kind)
                        }
                    })
                    .and_then(|()| validation::clear_issues(r, kind)),
            ),
        );
    }
    let mut feedback_actions = BTreeMap::new();
    feedback_actions.insert(
        "gameplay".to_string(),
        reason(feedback_check(r, root, None, false)),
    );
    for a in &r.artifacts {
        if let Some(id) = a["id"].as_str() {
            feedback_actions.insert(id.to_string(), reason(feedback(r, root, Some(id))));
        }
    }
    let recheck_actions: BTreeMap<_, _> = r
        .feedback
        .iter()
        .filter(|f| f["artifactId"].is_string())
        .filter_map(|f| {
            f["id"]
                .as_str()
                .map(|id| (id.to_owned(), reason(art_recheck(r, root, f))))
        })
        .collect();
    json!({"review":review_actions,"approve":approve_actions,"feedback":feedback_actions,"recheck":recheck_actions})
}
