use super::model::*;
use crate::modules::development_terminal::{ConversationAttention, TerminalProvider};
use crate::modules::tasks::{DevelopmentTask, ProductionView, TaskStatus};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn project(
    task: &DevelopmentTask,
    view: &ProductionView,
    conversations: &[ConversationAttention],
) -> TaskAttentionSnapshot {
    let mut snapshot = TaskAttentionSnapshot {
        task_id: task.id.clone(),
        ..Default::default()
    };
    if task.status != TaskStatus::Active {
        return snapshot;
    }
    let record = view.record.as_ref().filter(|r| r.task_id == task.id);
    let stage = record.map_or("requirements", |r| r.current_stage.as_str());
    let target = AttentionTarget {
        task_id: task.id.clone(),
        action: "stage".into(),
        stage: stage.into(),
        provider: None,
        conversation_id: None,
        group_id: None,
        document_revision: None,
        document_hash: None,
    };
    let mut add = |kind: &str, target: AttentionTarget, identity: Value, count: usize| {
        let key = format!(
            "{:x}",
            Sha256::digest(json!([task.id, kind, identity]).to_string().as_bytes())
        );
        let stage_name = view.policy["stages"]
            .as_array()
            .and_then(|stages| stages.iter().find(|s| s["id"] == target.stage))
            .and_then(|s| s["name"].as_str())
            .unwrap_or("")
            .to_string();
        snapshot.items.push(TaskAttention {
            key,
            task_id: task.id.clone(),
            task_title: task.title.clone(),
            kind: kind.into(),
            tone: match kind {
                "terminal" => "terminal",
                "failed" | "disconnected" | "needsReview" => "error",
                _ => "human",
            }
            .into(),
            stage_name,
            zh: text(kind, &target, count, false),
            en: text(kind, &target, count, true),
            target,
        });
    };
    let conversations = conversations
        .iter()
        .filter(|c| c.task_id == task.id)
        .collect::<Vec<_>>();
    for c in &conversations {
        if c.state == "waitingForTerminal" {
            let mut t = target.clone();
            t.action = "terminal".into();
            t.provider = Some(c.provider);
            t.conversation_id = Some(c.conversation_id.clone());
            add(
                "terminal",
                t,
                json!([c.provider, c.conversation_id, c.turn_id, c.interaction_id]),
                0,
            );
        }
    }
    let mut has_stage_action = false;
    if let Some(r) = record {
        for group in &r.question_groups {
            if group.status != "pending" {
                continue;
            }
            has_stage_action = true;
            let mut t = target.clone();
            t.action = "question".into();
            t.stage = group.stage.clone();
            t.group_id = Some(group.id.clone());
            t.provider = match group.provider.as_str() {
                "codex" => Some(TerminalProvider::Codex),
                _ => None,
            };
            t.conversation_id = Some(group.conversation_id.clone());
            add(
                "question",
                t,
                json!([
                    group.id,
                    group.cycle,
                    group.answers.last().map(|a| a.revision)
                ]),
                group.questions.len(),
            );
        }
        if let Some(stages) = view.policy["stages"].as_array() {
            for stage in stages {
                let Some(kind) = stage["id"].as_str() else {
                    continue;
                };
                let Some(doc) = r.documents.get(kind) else {
                    continue;
                };
                if view.actions["review"][kind].as_str() != Some("") {
                    continue;
                }
                has_stage_action = true;
                let mut t = target.clone();
                t.action = "review".into();
                t.stage = kind.into();
                t.document_revision = Some(doc.revision);
                t.document_hash = Some(doc.sha256.clone());
                add(
                    "review",
                    t,
                    json!([kind, doc.revision, doc.sha256, r.cycle]),
                    0,
                );
            }
        }
    }
    for c in &conversations {
        if matches!(c.state.as_str(), "failed" | "disconnected" | "needsReview") {
            let mut t = target.clone();
            t.action = "terminal".into();
            t.provider = Some(c.provider);
            t.conversation_id = Some(c.conversation_id.clone());
            add(
                &c.state,
                t,
                json!([c.provider, c.conversation_id, c.turn_id, c.request_id]),
                0,
            );
        }
    }
    if !conversations.iter().any(|c| c.is_running())
        && let Some(r) = record
    {
        if view.actions["feedback"]["gameplay"].as_str() == Some("")
            && matches!(stage, "review" | "delivery")
        {
            add(
                "playtest",
                target.clone(),
                json!([r.cycle, r.current_version]),
                0,
            );
        } else if !has_stage_action
            && (view
                .stage_statuses
                .get(stage)
                .is_some_and(|s| s == "blocked")
                || conversations
                    .iter()
                    .any(|c| matches!(c.state.as_str(), "blocked" | "waitingForApproval")))
        {
            // Do not re-key on drafts, progress timestamps or unrelated record revisions.
            let issues = r
                .issues
                .iter()
                .filter(|i| i["status"] != "resolved")
                .map(|i| json!([i["id"], i["status"], i["stage"], i["resumeWhen"]]))
                .collect::<Vec<_>>();
            add("blocked", target, json!([r.cycle, stage, issues]), 0);
        }
    }
    snapshot
}

fn text(kind: &str, target: &AttentionTarget, count: usize, en: bool) -> AttentionText {
    let pick = |zh: &str, english: &str| {
        if en {
            english.to_string()
        } else {
            zh.to_string()
        }
    };
    let (title, instruction, detail, destination, button) = match kind {
        "terminal" => (
            pick("终端需要你处理", "Your input is needed in the terminal"),
            pick(
                "一项输入或确认正在等待你的处理。",
                "An input or approval request needs your response.",
            ),
            pick(
                "请在终端阅读当前请求；处理完成后，AI 才能继续执行。",
                "Read the terminal request and respond so AI can continue.",
            ),
            pick("处理位置：原会话终端", "Respond in: Original terminal"),
            pick("前往终端处理", "Open terminal request"),
        ),
        "question" => (
            pick("需要你回答阶段问题", "Stage questions need your answers"),
            if en {
                format!("Answer {count} questions so AI can continue.")
            } else {
                format!("需要你回答 {count} 个问题，提交后 AI 将继续处理。")
            },
            pick(
                "答案在 APP 内提交，无需到终端重复回复。",
                "Submit answers here; no duplicate reply in the terminal is needed.",
            ),
            pick("处理位置：阶段问答", "Respond in: Stage questions"),
            pick("开始回答", "Answer questions"),
        ),
        "review" => (
            pick(
                if target.stage == "resources" {
                    "需要你审查美术需求"
                } else {
                    "需要你审查阶段成果"
                },
                "Stage results need your review",
            ),
            if en {
                format!(
                    "Version {} is submitted. Read it and choose a decision.",
                    target.document_revision.unwrap_or(0)
                )
            } else {
                format!(
                    "文档第 {} 版已提交，请阅读后给出结论。",
                    target.document_revision.unwrap_or(0)
                )
            },
            pick(
                "AI 正在等待你的审查结果；无需到终端重复回复。",
                "AI is waiting for your review; no duplicate terminal reply is needed.",
            ),
            pick("处理位置：制作流程", "Review in: Production"),
            pick("查看并审查", "Read and review"),
        ),
        "playtest" => (
            pick("需要你试玩并反馈", "Ready for your playtest"),
            pick(
                "当前作品已可验收，请试玩后提交反馈。",
                "Play the current version and submit feedback.",
            ),
            pick(
                "请核对实际体验，满意后再确认最终交付。",
                "Verify the experience before accepting the final delivery.",
            ),
            pick("处理位置：制作流程", "Respond in: Production"),
            pick("前往试玩与反馈", "Open playtest and feedback"),
        ),
        "blocked" => (
            pick("当前阶段需要核对", "Check this stage"),
            pick(
                "请查看成果和阻塞说明，确认下一步处理方式。",
                "Check results and blockers for the next action.",
            ),
            pick(
                "尚未满足审查条件的成果不会开放确认。",
                "Approval remains unavailable until review requirements are met.",
            ),
            pick("处理位置：制作流程", "Check in: Production"),
            pick("查看阶段详情", "View stage details"),
        ),
        _ => (
            match kind {
                "failed" => pick("本轮执行遇到错误", "This run encountered an error"),
                "disconnected" => pick("原对话需要接回", "Reconnect the original conversation"),
                _ => pick("执行结果需要核对", "Execution needs review"),
            },
            pick(
                "请先核对原会话状态，再决定如何继续。",
                "Check the original conversation before continuing.",
            ),
            pick(
                "已有答案与阶段记录保留，请勿重复提交。",
                "Answers and stage records are saved; do not resubmit them.",
            ),
            pick("处理位置：终端", "Check in: Terminal"),
            pick(
                if kind == "disconnected" {
                    "查看连接状态"
                } else {
                    "查看原因"
                },
                "Check details",
            ),
        ),
    };
    AttentionText {
        title,
        instruction,
        detail,
        destination,
        button,
    }
}
