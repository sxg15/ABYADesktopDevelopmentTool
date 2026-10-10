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

fn subtract_ms(spans: &[(i64, i64)], excluded: &[(i64, i64)]) -> Option<i64> {
    if spans.is_empty() {
        return None;
    }
    let mut remaining = spans.to_vec();
    for &(c, d) in excluded {
        remaining = remaining
            .into_iter()
            .flat_map(|(a, b)| {
                if d <= a || c >= b {
                    return vec![(a, b)];
                }
                let mut parts = Vec::new();
                if a < c {
                    parts.push((a, c));
                }
                if d < b {
                    parts.push((d, b));
                }
                parts
            })
            .collect();
    }
    Some(union_ms(remaining).unwrap_or(0))
}

pub fn task_timing(tasks: &TaskService, task_id: &str) -> AppResult<Value> {
    let task = tasks.get(task_id)?;
    let root = Path::new(&task.workspace_path).canonicalize()?;
    let now = json!(Utc::now().to_rfc3339());
    let (mut execution, mut tools, mut answers, mut approvals, mut pauses, mut connections) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut missing = 0;
    let (mut human, mut blocked, mut rework) = (vec![], vec![], vec![]);
    let (mut elapsed, mut first_playable) = (None, None);
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
        let end = if r.stages.get("closeout").is_some_and(|s| s == "passed") {
            json!(r.updated_at)
        } else {
            now.clone()
        };
        elapsed = interval(&json!(task.created_at), &end).map(|(a, b)| b - a);
        first_playable = r
            .events
            .iter()
            .find(|e| e["stage"] == "implementation" && e["status"] == "passed")
            .and_then(|e| interval(&json!(task.created_at), &e["at"]).map(|(a, b)| b - a));
        for (i, event) in r.events.iter().enumerate() {
            let next = r
                .events
                .iter()
                .skip(i + 1)
                .find(|e| e["stage"] == event["stage"])
                .map(|e| &e["at"])
                .unwrap_or(&end);
            if let Some(span) = interval(&event["at"], next) {
                if event["status"] == "blocked" {
                    blocked.push(span);
                }
                if event["status"] == "awaiting-confirmation"
                    || event["status"] == "awaiting-feedback"
                {
                    human.push(span);
                }
            }
        }
        for f in &r.feedback {
            let history = f["history"].as_array().cloned().unwrap_or_default();
            for (i, h) in history.iter().enumerate() {
                let next = history.get(i + 1).map(|h| &h["at"]).unwrap_or(&end);
                if let Some(span) = interval(&h["at"], next) {
                    if h["status"] == "in-progress" {
                        rework.push(span);
                    }
                    if h["status"] == "awaiting-recheck" {
                        human.push(span);
                    }
                }
            }
        }
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
    human.extend(answers.iter().copied());
    human.extend(approvals.iter().copied());
    let mut excluded = human.clone();
    excluded.extend(blocked.iter().copied());
    excluded.extend(pauses.iter().copied());
    let active_work = subtract_ms(&execution, &excluded);
    Ok(
        json!({"observedAt":now,"elapsedMs":elapsed,"firstPlayableMs":first_playable,"humanWaitMs":union_ms(human),"blockedMs":union_ms(blocked),"feedbackReworkMs":union_ms(rework),"activeWorkMs":active_work,"executionMs":union_ms(execution),"toolMs":union_ms(tools),"answerWaitMs":union_ms(answers),"approvalWaitMs":union_ms(approvals),"pauseMs":union_ms(pauses),"connectionMs":union_ms(connections),"unfinishedTurns":missing,"note":"推进时间仅从有完整起止的执行区间扣除已知人工等待、阻塞和暂停，不是纯模型工时。工具、返工与执行互有重叠，不可相加；阶段等待统计至观察时刻。缺失或未结束执行保持未知。"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_and_replayed_intervals_are_not_counted_twice() {
        assert_eq!(union_ms(vec![(1, 5), (2, 8), (1, 5), (12, 15)]), Some(10));
        assert_eq!(union_ms(vec![]), None);
        assert_eq!(
            subtract_ms(&[(0, 100), (20, 80)], &[(10, 30), (25, 40), (70, 90)]),
            Some(50)
        );
        assert_eq!(subtract_ms(&[], &[(1, 2)]), None);
        assert_eq!(subtract_ms(&[(1, 2)], &[(0, 4)]), Some(0));
    }
}
