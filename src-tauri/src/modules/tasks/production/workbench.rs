use super::*;

#[allow(clippy::too_many_arguments)]
fn upsert(
    r: &mut ProductionRecord,
    stage: &str,
    kind: &str,
    source: &str,
    title: &str,
    status: &str,
    summary: &str,
    decision: Option<String>,
    revision: u64,
) -> String {
    let id = format!("{kind}:{source}");
    if let Some(g) = r.workbench.goals.iter_mut().find(|g| g.id == id) {
        if g.status != status || g.summary != summary || g.decision != decision {
            g.status = status.into();
            g.summary = summary.chars().take(4000).collect();
            g.decision = decision;
            g.record_revision = revision;
            g.updated_at = now();
        }
        return id;
    }
    let number = r
        .workbench
        .goals
        .iter()
        .filter(|g| g.stage == stage)
        .map(|g| g.number)
        .max()
        .unwrap_or(0)
        + 1;
    r.workbench.goals.push(WorkbenchGoal {
        id: id.clone(),
        stage: stage.into(),
        number,
        title: title.chars().take(120).collect(),
        source_type: kind.into(),
        source_id: source.into(),
        status: status.into(),
        summary: summary.chars().take(4000).collect(),
        decision,
        record_revision: revision,
        created_at: now(),
        updated_at: now(),
        cycle: r.cycle,
    });
    id
}

/// This journal references production revisions; it never decides a gate or approval.
pub(super) fn observe(r: &mut ProductionRecord, before: Option<&ProductionRecord>) {
    let original_count = r.workbench.goals.len();
    let revision = r.revision + u64::from(before.is_some());
    let mut current = None;
    for group in r.question_groups.clone() {
        let id = upsert(
            r,
            &group.stage,
            "question",
            &group.id,
            &group.title,
            &group.status,
            &group
                .questions
                .iter()
                .map(|q| q.text.clone())
                .collect::<Vec<_>>()
                .join("\n"),
            group
                .answers
                .last()
                .map(|a| serde_json::to_string(&a.answers).unwrap_or_default()),
            revision,
        );
        if group.stage == r.current_stage && group.status == "pending" {
            current = Some(id);
        }
    }
    for (stage, doc) in r.documents.clone() {
        let source = format!("{stage}:{}:{}", doc.revision, doc.sha256);
        for old in r.workbench.goals.iter_mut().filter(|g| {
            g.stage == stage
                && g.source_type == "document"
                && g.source_id != source
                && g.status == "awaiting-confirmation"
        }) {
            old.status = "superseded".into();
        }
        let decision = r
            .approvals
            .iter()
            .rev()
            .find(|a| {
                a.kind == stage
                    && a.document_revision == doc.revision
                    && a.document_hash == doc.sha256
            })
            .cloned();
        let status = decision
            .as_ref()
            .map(|a| a.decision.as_str())
            .unwrap_or("awaiting-confirmation");
        let title = format!("{} · 第 {} 版", stage_label(&stage), doc.revision);
        let id = upsert(
            r,
            &stage,
            "document",
            &format!("{stage}:{}:{}", doc.revision, doc.sha256),
            &title,
            status,
            &doc.content,
            decision.as_ref().map(|a| a.feedback.clone()),
            revision,
        );
        if current.is_none()
            && stage == r.current_stage
            && r.stages
                .get(&stage)
                .is_some_and(|s| s == "awaiting-confirmation")
        {
            current = Some(id);
        }
    }
    for feedback in r.feedback.clone() {
        let stage = feedback["stage"].as_str().unwrap_or("review");
        let id = upsert(
            r,
            stage,
            "feedback",
            feedback["id"].as_str().unwrap_or(""),
            "修改与复验",
            feedback["status"].as_str().unwrap_or("open"),
            feedback["description"].as_str().unwrap_or(""),
            feedback["history"]
                .as_array()
                .and_then(|h| h.iter().rev().find(|v| v["actor"] == "user"))
                .map(Value::to_string),
            revision,
        );
        if current.is_none() && stage == r.current_stage && feedback["status"] != "resolved" {
            current = Some(id);
        }
    }
    for request in r.workbench.requests.clone() {
        let stage = request["stage"].as_str().unwrap_or("requirements");
        let id = upsert(
            r,
            stage,
            "request",
            request["id"].as_str().unwrap_or(""),
            request["text"].as_str().unwrap_or("用户目标"),
            request["status"].as_str().unwrap_or("pending"),
            request["summary"]
                .as_str()
                .or_else(|| request["text"].as_str())
                .unwrap_or(""),
            None,
            revision,
        );
        if current.is_none() && stage == r.current_stage && request["status"] != "completed" {
            current = Some(id);
        }
    }
    if current.is_none() {
        let existing = r
            .workbench
            .goals
            .iter()
            .rev()
            .find(|g| {
                g.stage == r.current_stage
                    && g.cycle == r.cycle
                    && g.source_type == "work"
                    && g.status == "in-progress"
            })
            .map(|g| g.id.clone());
        if let Some(id) = existing {
            current = Some(id);
        } else if r.stages.get(&r.current_stage).is_none_or(|s| s != "passed") {
            let stage = r.current_stage.clone();
            let summary = r
                .stage_updates
                .get(&stage)
                .and_then(|v| v["summary"].as_str())
                .unwrap_or("")
                .to_string();
            current = Some(upsert(
                r,
                &stage,
                "work",
                &format!("{}:{}:{}", stage, r.cycle, r.workbench.goals.len()),
                stage_label(&stage),
                "in-progress",
                &summary,
                None,
                revision,
            ));
        }
    }
    if let Some(old) = r
        .workbench
        .active_goal_id
        .clone()
        .filter(|id| Some(id) != current.as_ref())
        && let Some(g) = r
            .workbench
            .goals
            .iter_mut()
            .find(|g| g.id == old && g.source_type == "work" && g.status == "in-progress")
    {
        g.status = "submitted".into();
        g.updated_at = now();
        // The snapshot before the new entity replaced it is the historical evidence.
        if before.is_some() {
            g.record_revision = r.revision;
        }
    }
    r.workbench.active_goal_id = current;
    if before.is_none() {
        for goal in r.workbench.goals.iter_mut().skip(original_count) {
            goal.created_at = r.updated_at.clone();
            goal.updated_at = r.updated_at.clone();
        }
    }
}

pub(super) fn stage_label(stage: &str) -> &str {
    match stage {
        "requirements" => "需求整理",
        "resources" => "资源美术",
        "plan" => "方案预检",
        "implementation" => "Lua 实现",
        "review" => "试玩与修改",
        "delivery" => "交付验收",
        "closeout" => "收尾复盘",
        _ => stage,
    }
}
