use super::{ProductionRecord, files};
use crate::foundation::{AppError, AppResult};
use serde_json::Value;
use std::path::Path;

pub(super) fn approved(r: &ProductionRecord, root: &Path, kind: &str) -> AppResult<()> {
    if r.stages.get(kind).map(String::as_str) != Some("passed") {
        return Err(AppError::validation(format!("{kind} 当前版本尚未确认。")));
    }
    let d = r
        .documents
        .get(kind)
        .ok_or_else(|| AppError::validation(format!("缺少 {kind} 文档。")))?;
    let accepted = r
        .approvals
        .iter()
        .rev()
        .find(|a| a.kind == kind && a.document_revision == d.revision)
        .is_some_and(|a| a.decision == "accepted" && a.document_hash == d.sha256);
    if !accepted || files::text_file(root, &d.path)?.1 != d.sha256 {
        return Err(AppError::validation(format!(
            "{kind} 当前文档版本尚未确认或已变化，请重新提交确认。"
        )));
    }
    if kind == "delivery" && d.game_version != r.current_version {
        return Err(AppError::validation("作品版本变化，原交付确认已失效。"));
    }
    Ok(())
}

pub(super) fn clear_issues(r: &ProductionRecord, stage: &str) -> AppResult<()> {
    if r.issues.iter().any(|i| {
        i["status"] != "resolved"
            && i["kind"] != "suggestion"
            && (i["stage"] == stage || i["kind"] == "blocker")
    }) {
        return Err(AppError::validation(
            "仍有未关闭的问题或阻塞，请先处理并记录复验。",
        ));
    }
    Ok(())
}

pub(super) fn evidence_refs(
    r: &ProductionRecord,
    root: &Path,
    ids: &Value,
    version: &str,
) -> AppResult<()> {
    let ids = ids
        .as_array()
        .filter(|x| !x.is_empty())
        .ok_or_else(|| AppError::validation("必须关联实际证据。"))?;
    for id in ids {
        let e = r
            .evidence
            .iter()
            .find(|e| Some(e.id.as_str()) == id.as_str())
            .ok_or_else(|| AppError::validation("引用了未登记的证据。"))?;
        if !e.reviewed || e.version != version || e.cycle != r.cycle {
            return Err(AppError::validation("证据未审阅或属于其他版本/需求周期。"));
        }
        if files::hash_file(&files::safe_path(root, &e.path)?)?.0 != e.sha256 {
            return Err(AppError::validation("证据文件已变化，请重新取证登记。"));
        }
    }
    Ok(())
}

pub(super) fn milestone(
    r: &ProductionRecord,
    root: &Path,
    name: &str,
    version: &str,
) -> AppResult<()> {
    let ids = r
        .milestones
        .get(name)
        .ok_or_else(|| AppError::validation(format!("缺少 {name} 关键版本证据。")))?;
    evidence_refs(r, root, &serde_json::to_value(ids)?, version)?;
    let selected: Vec<_> = r.evidence.iter().filter(|e| ids.contains(&e.id)).collect();
    if !selected
        .iter()
        .any(|e| e.kind == "video" && e.capture_type == "full-cycle")
        || !selected.iter().any(|e| e.kind == "image")
        || !selected.iter().any(|e| e.capture_type == "state")
    {
        return Err(AppError::validation(format!(
            "{name} 需要完整循环录像、关键状态截图和状态断言证据。"
        )));
    }
    Ok(())
}

pub(super) fn version(r: &ProductionRecord) -> AppResult<&str> {
    r.current_version
        .as_deref()
        .ok_or_else(|| AppError::validation("请先登记实际存档与 Player 版本。"))
}

pub(super) fn document_gate(r: &ProductionRecord, root: &Path, kind: &str) -> AppResult<()> {
    clear_issues(r, kind)?;
    match kind {
        "plan" => {
            approved(r, root, "requirements")?;
            if r.stages.get("resources").map(String::as_str) != Some("passed") {
                return Err(AppError::validation("请先完成现有资源与美术要求整理。"));
            }
        }
        "delivery" => {
            if r.issues
                .iter()
                .any(|i| i["status"] != "resolved" && i["kind"] != "suggestion")
            {
                return Err(AppError::validation("交付前仍有未关闭的必要问题。"));
            }
            approved(r, root, "requirements")?;
            approved(r, root, "plan")?;
            let final_round = r
                .rounds
                .iter()
                .find(|x| x["cycle"] == r.cycle && x["number"] == 12 && x["status"] == "closed")
                .ok_or_else(|| AppError::validation("尚未完成十二轮整体迭代。"))?;
            round(r, root, final_round)?;
            milestone(r, root, "R12", version(r)?)?;
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn warnings(r: &ProductionRecord, root: &Path) -> Vec<String> {
    let mut result = Vec::new();
    for (kind, d) in &r.documents {
        if files::text_file(root, &d.path).map_or(true, |(_, hash)| hash != d.sha256) {
            result.push(format!(
                "{kind} 文件已修改或不可读，需重新提交；旧确认不可用于推进。"
            ));
        }
        if kind == "delivery" && d.game_version != r.current_version {
            result.push("delivery 作品版本已变化，需重新验证并提交交付。".into());
        }
    }
    if r.legacy_record.is_some() {
        result.push("已保留旧工作流资料；新流程没有补造旧确认或轮次。".into());
    }
    result
}

pub(super) fn round(r: &ProductionRecord, root: &Path, value: &Value) -> AppResult<()> {
    super::versions::verify(&r.version_details)?;
    let version = version(r)?;
    if value["outputVersion"] != version {
        return Err(AppError::validation("本轮输出与当前作品版本不一致。"));
    }
    clear_issues(r, "review")?;
    let p = &r.policy;
    let checks = value["checks"]
        .as_array()
        .ok_or_else(|| AppError::validation("缺少本轮检查清单。"))?;
    let dimensions = p["dimensions"].as_array().unwrap();
    if checks.len() != dimensions.len() {
        return Err(AppError::validation("每轮必须覆盖全部九个检查维度。"));
    }
    for dimension in dimensions {
        let matching: Vec<_> = checks
            .iter()
            .filter(|c| c["dimensionId"] == dimension["id"])
            .collect();
        if matching.len() != 1 {
            return Err(AppError::validation("检查维度缺失或重复。"));
        }
        let c = matching[0];
        match c["status"].as_str() {
            Some("passed") => evidence_refs(r, root, &c["evidenceIds"], version)?,
            Some("not-applicable")
                if !["requirements", "lifecycle", "persistence"]
                    .contains(&dimension["id"].as_str().unwrap()) =>
            {
                nonempty(c, "observations")?;
            }
            _ => return Err(AppError::validation("必要检查尚未通过，不能关闭本轮。")),
        }
    }
    if value["number"].as_u64().unwrap_or(0) >= 9 {
        let answers = value["questions"]
            .as_array()
            .ok_or_else(|| AppError::validation("后四轮需要全部固定问答。"))?;
        let questions = p["questions"].as_array().unwrap();
        if answers.len() != questions.len() {
            return Err(AppError::validation("每个最终问答轮必须回答全部 16 题。"));
        }
        for question in questions {
            let matching: Vec<_> = answers
                .iter()
                .filter(|a| a["questionId"] == question["id"])
                .collect();
            if matching.len() != 1 {
                return Err(AppError::validation("固定题目缺失或重复。"));
            }
            let a = matching[0];
            for field in [
                "conclusion",
                "counterexample",
                "problem",
                "change",
                "recheck",
            ] {
                nonempty(a, field)?;
            }
            if a["status"] == "passed" {
                evidence_refs(r, root, &a["evidenceIds"], version)?;
            } else if a["status"] != "not-applicable" || question["id"] != "F06" {
                return Err(AppError::validation("固定问题尚未通过。"));
            }
        }
    }
    evidence_refs(r, root, &value["evidenceIds"], version)?;
    let start = value["startedAt"]
        .as_str()
        .ok_or_else(|| AppError::validation("缺少本轮开始时间。"))?;
    for id in value["evidenceIds"].as_array().unwrap() {
        let evidence = r
            .evidence
            .iter()
            .find(|e| Some(e.id.as_str()) == id.as_str())
            .unwrap();
        if evidence.recorded_at.as_str() < start {
            return Err(AppError::validation(
                "本轮实际巡检需关联本轮开始后登记的证据，旧证据不能冒充新执行。",
            ));
        }
    }
    Ok(())
}

pub(super) fn nonempty(value: &Value, field: &str) -> AppResult<()> {
    if value[field].as_str().is_none_or(|s| s.trim().is_empty()) {
        Err(AppError::validation(format!("缺少 {field}。")))
    } else {
        Ok(())
    }
}
