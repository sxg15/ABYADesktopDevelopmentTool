use super::*;
use sha2::{Digest, Sha256};

pub(super) fn hash(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["artDesignVersion"] == 1
}

pub(super) fn refresh(r: &mut ProductionRecord, task: &super::super::DevelopmentTask) {
    if !enabled(r) {
        return;
    }
    let mut sources = vec![];
    let mut add = |id: String, kind: &str, text: String, asset: Value| {
        if text.trim().is_empty() && asset.is_null() {
            return;
        }
        let digest = hash(&json!([id, kind, text, asset]));
        sources.push(json!({"id":id,"kind":kind,"text":text,"asset":asset,"hash":digest}));
    };
    add(
        "task-description".into(),
        "task",
        task.description.clone(),
        Value::Null,
    );
    for g in &r.question_groups {
        if g.status == "cancelled"
            || !["requirements", "resources", "plan"].contains(&g.stage.as_str())
        {
            continue;
        }
        if let Some(a) = g.answers.last() {
            for q in &g.questions {
                if let Some(answer) = a.answers.get(&q.id) {
                    add(
                        format!("answer:{}:{}", g.id, q.id),
                        "answer",
                        format!("{}\n{}", q.text, answer),
                        Value::Null,
                    );
                }
            }
        }
    }
    for a in r
        .artifacts
        .iter()
        .filter(|a| a["plannerReference"] == true && a["status"] != "withdrawn")
    {
        add(
            format!("reference:{}", a["id"].as_str().unwrap_or("")),
            "reference",
            a["summary"].as_str().unwrap_or("").into(),
            json!({"id":a["id"],"path":a["path"],"sha256":a["sha256"],"title":a["title"]}),
        );
    }
    for f in r
        .feedback
        .iter()
        .filter(|f| f["stage"] == "resources" || f["artifactId"].is_string())
    {
        add(
            format!("feedback:{}", f["id"].as_str().unwrap_or("")),
            "feedback",
            f["description"].as_str().unwrap_or("").into(),
            Value::Null,
        );
        // Each human amendment remains a separate source; closing feedback does not erase it.
        for (i, h) in f["history"].as_array().into_iter().flatten().enumerate() {
            if h["actor"] == "user" && h["status"] == "open" {
                add(
                    format!("feedback:{}:amendment:{i}", f["id"].as_str().unwrap_or("")),
                    "feedback",
                    h["reason"].as_str().unwrap_or("").into(),
                    Value::Null,
                );
            }
        }
    }
    for (i, a) in r.approvals.iter().enumerate().filter(|(_, a)| {
        a.decision == "rejected" && ["requirements", "resources", "plan"].contains(&a.kind.as_str())
    }) {
        add(
            format!("decision:{i}"),
            "decision",
            a.feedback.clone(),
            Value::Null,
        );
    }
    for n in &r.art_design.planner_notes {
        add(
            format!("note:{}", n["id"].as_str().unwrap_or("")),
            "note",
            n["text"].as_str().unwrap_or("").into(),
            Value::Null,
        );
    }
    for n in &r.art_design.conversation_inputs {
        add(
            format!("conversation:{}", n["id"].as_str().unwrap_or("")),
            "agent-transcribed-conversation",
            format!(
                "来源（AI 摘录，待策划核对）：{}\n{}",
                n["locator"].as_str().unwrap_or(""),
                n["text"].as_str().unwrap_or("")
            ),
            Value::Null,
        );
    }
    let changed = r.art_design.sources != sources;
    r.art_design.sources = sources;
    if changed {
        invalidate(r);
    }
}

pub(super) fn invalidate(r: &mut ProductionRecord) {
    if !enabled(r) || !r.documents.contains_key("resources") {
        return;
    }
    for stage in [
        "resources",
        "plan",
        "implementation",
        "review",
        "delivery",
        "closeout",
    ] {
        r.stages.insert(stage.into(), "not-started".into());
    }
    if r.stages.get("requirements").is_some_and(|s| s == "passed") {
        r.current_stage = "resources".into();
        r.stages.insert("resources".into(), "in-progress".into());
    }
}

pub(super) fn inputs_hash(r: &ProductionRecord) -> String {
    hash(&json!([
        r.cycle,
        r.art_design.sources,
        r.art_design.requirements
    ]))
}

pub(super) fn design_hash(r: &ProductionRecord) -> String {
    let a = &r.art_design;
    hash(&json!([
        r.cycle,
        a.sources,
        a.requirements,
        a.source_reviews,
        a.reference_analyses,
        a.direction,
        a.coverage,
        a.case_uses
    ]))
}
