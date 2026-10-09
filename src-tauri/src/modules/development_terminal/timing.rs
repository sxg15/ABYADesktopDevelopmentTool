use crate::foundation::AppResult;
use crate::modules::tasks::TaskService;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::path::Path;

fn timestamp(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v.as_str()?)
        .ok()
        .map(|t| t.timestamp_millis())
}
fn interval(start: &Value, end: &Value) -> Option<(i64, i64)> {
    let (a, b) = (timestamp(start)?, timestamp(end)?);
    (b >= a).then_some((a, b))
}
fn union_ms(mut spans: Vec<(i64, i64)>) -> Option<i64> {
    if spans.is_empty() {
        return None;
    }
    spans.sort_unstable();
    let (mut a, mut b) = spans[0];
    let mut total = 0;
    for (c, d) in spans.into_iter().skip(1) {
        if c <= b {
            b = b.max(d);
        } else {
            total += b - a;
            a = c;
            b = d;
        }
    }
    Some(total + b - a)
}
fn read(root: &Path, path: &Path) -> Option<Value> {
    let path = path.canonicalize().ok()?;
    if !path.starts_with(root) || path.metadata().ok()?.len() > 16 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub fn task_timing(tasks: &TaskService, task_id: &str) -> AppResult<Value> {
    let task = tasks.get(task_id)?;
    let root = Path::new(&task.workspace_path).canonicalize()?;
    let now = json!(Utc::now().to_rfc3339());
    let (mut execution, mut tools, mut answers, mut approvals, mut pauses, mut connections) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut missing = 0;
    for folder in ["conversations", "grok-conversations"] {
        if let Ok(entries) = std::fs::read_dir(root.join(folder)) {
            for entry in entries.flatten() {
                let dir = entry.path();
                if uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err() {
                    continue;
                }
                let Some(metadata) = read(&root, &dir.join("conversation.json")) else {
                    continue;
                };
                if metadata["taskId"] != task_id {
                    continue;
                }
                if let Some(workflow) = read(&root, &dir.join("workflow.json")) {
                    if workflow["taskId"] != task_id {
                        continue;
                    }
                    for turn in workflow["turns"].as_array().into_iter().flatten() {
                        if let Some(span) = interval(&turn["startedAt"], &turn["completedAt"]) {
                            execution.push(span);
                        } else {
                            missing += 1;
                        }
                        for activity in turn["activities"].as_array().into_iter().flatten() {
                            if activity["kind"] == "analysis"
                                || activity["source"] == "desktopCliReport"
                            {
                                continue;
                            }
                            if let Some(span) =
                                interval(&activity["startedAt"], &activity["completedAt"])
                            {
                                tools.push(span);
                            }
                        }
                    }
                }
                let log = dir.join("connection-events.jsonl");
                if let Ok(path) = log.canonicalize()
                    && path.starts_with(&root)
                    && path.metadata().is_ok_and(|m| m.len() <= 8 * 1024 * 1024)
                {
                    let mut open = None;
                    let mut paused = None;
                    for event in std::fs::read_to_string(path)?
                        .lines()
                        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                    {
                        match event["event"].as_str() {
                            Some("open-requested") => {
                                open = Some(event["at"].clone());
                            }
                            Some("paused") => {
                                if paused.is_none() {
                                    paused = Some(event["at"].clone());
                                }
                            }
                            Some("terminal-connected") => {
                                if let Some(start) = open.take()
                                    && let Some(span) = interval(&start, &event["at"])
                                {
                                    connections.push(span);
                                }
                                if let Some(start) = paused.take()
                                    && let Some(span) = interval(&start, &event["at"])
                                {
                                    pauses.push(span);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    if let Some(r) = tasks.production_get(task_id)?.record {
        for g in &r.question_groups {
            if let Some(a) = g.answers.first()
                && let Some(span) = interval(&json!(g.published_at), &json!(a.submitted_at))
            {
                answers.push(span);
            }
        }
        for d in r.documents.values() {
            if let Some(a) = r
                .approvals
                .iter()
                .find(|a| a.kind == d.kind && a.document_revision == d.revision)
                && let Some(span) = interval(&json!(d.submitted_at), &json!(a.decided_at))
            {
                approvals.push(span);
            }
        }
    }
    Ok(
        json!({"observedAt":now,"executionMs":union_ms(execution),"toolMs":union_ms(tools),"answerWaitMs":union_ms(answers),"approvalWaitMs":union_ms(approvals),"pauseMs":union_ms(pauses),"connectionMs":union_ms(connections),"unfinishedTurns":missing,"note":"各项按已有起止记录计算，同类重叠区间已合并。执行区间包含工具和等待，各项不能直接相加；未结束和缺失记录保留未知。"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_and_replayed_intervals_are_not_counted_twice() {
        assert_eq!(union_ms(vec![(1, 5), (2, 8), (1, 5), (12, 15)]), Some(10));
        assert_eq!(union_ms(vec![]), None);
    }
}
