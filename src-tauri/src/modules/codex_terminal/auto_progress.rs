use super::continuation_queue::{self, ContinueRequest};
use super::*;
use crate::modules::tasks::ProductionView;

#[derive(Default)]
pub(super) struct AutoProgress {
    pub budget_interrupt: bool,
    pub turn: String,
    pub handled: bool,
    pub completion_seen: bool,
    pub last_progress: Option<String>,
}

pub(super) fn eligible(view: &ProductionView) -> bool {
    let Some(r) = &view.record else {
        return false;
    };
    continuation_queue::waiting(Some(view)).is_none()
        && (crate::modules::tasks::autonomous_implementation(r)
            || crate::modules::tasks::preflight_recovery_active(r)
            || matches!(
                r.stages.get(&r.current_stage).map(String::as_str),
                Some("in-progress" | "not-started")
            ))
        && (crate::modules::tasks::autonomous_implementation(r)
            || crate::modules::tasks::preflight_recovery_active(r)
            || view
                .stage_statuses
                .get(&r.current_stage)
                .is_none_or(|s| s != "blocked"))
        && (crate::modules::tasks::autonomous_implementation(r)
            || !r.feedback.iter().any(|f| f["status"] == "awaiting-recheck"))
}

// Excludes polling times, draft saves, record revisions and internal retries.
fn progress(view: &ProductionView, workflow: &CodexWorkflowSnapshot) -> String {
    use sha2::{Digest, Sha256};
    let r = view.record.as_ref().expect("eligible production");
    if r.policy["implementationExecution"]["inPlaceRecoveryVersion"] == 1 {
        let documents: Vec<_> = r
            .documents
            .iter()
            .map(|(kind, d)| (kind, &d.sha256))
            .collect();
        let media: std::collections::BTreeSet<_> = r
            .artifacts
            .iter()
            .filter(|a| {
                matches!(a["kind"].as_str(), Some("image" | "video"))
                    || a["path"].as_str().is_some_and(|p| p.ends_with(".lua"))
            })
            .filter_map(|a| a["sha256"].as_str())
            .collect();
        return format!(
            "{:x}",
            Sha256::digest(
                json!([
                    r.current_stage,
                    r.stages,
                    documents,
                    media,
                    r.version_details["archiveHash"]
                ])
                .to_string()
            )
        );
    }
    let updates: Vec<_> = r
        .stage_updates
        .iter()
        .map(|(stage, v)| (stage, &v["summary"], &v["nextAction"]))
        .collect();
    let documents: Vec<_> = r
        .documents
        .iter()
        .map(|(stage, d)| (stage, &d.sha256))
        .collect();
    let artifacts: Vec<_> = r
        .artifacts
        .iter()
        .map(|a| (&a["id"], &a["sha256"], &a["status"]))
        .collect();
    let goals: Vec<_> = r
        .workbench
        .goals
        .iter()
        .map(|g| (&g.id, &g.status, &g.summary))
        .collect();
    let plan: Vec<_> = workflow
        .turns
        .iter()
        .find(|t| Some(&t.id) == workflow.current_turn_id.as_ref())
        .map(|t| t.plan.iter().map(|s| (&s.step, &s.status)).collect())
        .unwrap_or_default();
    let value = json!([
        r.current_stage,
        r.stages,
        r.current_version,
        r.current_round,
        updates,
        documents,
        artifacts,
        goals,
        plan
    ]);
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}

impl CodexTerminalService {
    pub(super) fn auto_turn_started(&self, native: &str, turn: &str) {
        let Some((_, conversation)) = self.thread_conversations.lock().get(native).cloned() else {
            return;
        };
        if self.cancelled.lock().contains(&conversation)
            || self.stopping.lock().contains(&conversation)
            || self.shutting_down.load(std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        let mut runs = self.auto_progress.lock();
        let run = runs.entry(conversation).or_default();
        if run.turn != turn {
            run.turn = turn.into();
            run.handled = false;
            run.completion_seen = false;
            run.budget_interrupt = false;
        }
    }

    pub(super) fn auto_completion_seen(
        &self,
        native: &str,
        turn: &str,
        mark: Option<bool>,
    ) -> bool {
        let conversation = {
            self.thread_conversations
                .lock()
                .get(native)
                .map(|(_, c)| c.clone())
        };
        let Some(conversation) = conversation else {
            return false;
        };
        let mut runs = self.auto_progress.lock();
        let Some(run) = runs.get_mut(&conversation).filter(|r| r.turn == turn) else {
            return false;
        };
        if let Some(value) = mark {
            run.completion_seen = value;
        }
        run.completion_seen
    }

    /// Called under the lifecycle lock, only for native events observed in this process.
    pub(super) fn auto_complete_locked(
        &self,
        task: &str,
        conversation: &str,
        turn: &str,
        completed: bool,
    ) -> AppResult<()> {
        if self.cancelled.lock().contains(conversation)
            || self.stopping.lock().contains(conversation)
            || self.shutting_down.load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(());
        }
        let budget_interrupt = self
            .auto_progress
            .lock()
            .get(conversation)
            .is_some_and(|r| r.turn == turn && r.budget_interrupt);
        let recover_failed = !completed
            && self.workflow(task, conversation).ok().is_some_and(|w| {
                w.turns.iter().any(|t| {
                    t.id == turn
                        && (t.status == CodexWorkflowTurnStatus::Failed
                            || budget_interrupt && t.status == CodexWorkflowTurnStatus::Interrupted)
                })
            })
            && self
                .tasks
                .get_readonly(task)
                .ok()
                .and_then(|t| self.tasks.production_attention(&t).ok())
                .and_then(|v| v.record)
                .is_some_and(|r| {
                    crate::modules::tasks::autonomous_implementation(&r)
                        || crate::modules::tasks::preflight_recovery_active(&r)
                });
        let completed = completed || recover_failed;
        {
            let mut runs = self.auto_progress.lock();
            let Some(run) = runs
                .get_mut(conversation)
                .filter(|r| r.turn == turn && !r.handled)
            else {
                return Ok(());
            };
            run.handled = !completed;
        }
        if !completed {
            return Ok(());
        }
        let task_record = self.tasks.get(task)?;
        if task_record.status != crate::modules::tasks::TaskStatus::Active {
            return Ok(());
        }
        let metadata = self.conversation(task, conversation)?;
        if metadata.archived {
            return Ok(());
        }
        let Some(native) = metadata.native_session_id else {
            return Ok(());
        };
        let directory =
            conversation_directory(Path::new(&task_record.workspace_path), conversation);
        let previous = continuation_queue::read(&directory)?;
        if previous.as_ref().is_some_and(|q| {
            !matches!(
                q.status.as_str(),
                "sent" | "running" | "settled" | "superseded"
            )
        }) {
            return Ok(());
        }
        let workflow = self.workflow(task, conversation)?;
        if pending_tools(&workflow)
            || workflow.terminal_interaction
            || workflow.current_turn_id.as_deref() != Some(turn)
            || workflow
                .turns
                .iter()
                .find(|t| t.id == turn)
                .is_none_or(|t| {
                    t.status != CodexWorkflowTurnStatus::Completed
                        && !(recover_failed
                            && (t.status == CodexWorkflowTurnStatus::Failed
                                || budget_interrupt
                                    && t.status == CodexWorkflowTurnStatus::Interrupted))
                })
        {
            return Ok(());
        }
        let runtime_failed = workflow
            .turns
            .iter()
            .find(|t| t.id == turn)
            .is_some_and(|t| {
                t.activities.iter().any(|a| {
                    a.source == "desktopCli"
                        && a.kind == CodexActivityKind::GameInstance
                        && a.status == CodexActivityStatus::Failed
                })
            });
        self.tasks.production_prepare_preflight_recovery(
            task,
            &format!("turn:{native}:{turn}"),
            runtime_failed,
        )?;
        let view = self.tasks.production_attention(&task_record)?;
        if !eligible(&view) {
            return Ok(());
        }
        let autonomous = view
            .record
            .as_ref()
            .is_some_and(crate::modules::tasks::autonomous_implementation);
        let checkpoint = autonomous
            && budget_interrupt
            && self
                .tasks
                .production_execution_checkpoint(task, &format!("checkpoint:{native}:{turn}"))?;
        if autonomous
            && !checkpoint
            && self
                .tasks
                .production_schedule(task, &format!("turn:{native}:{turn}"), true)?
                .is_none()
        {
            if let Some(run) = self.auto_progress.lock().get_mut(conversation) {
                run.handled = true;
            }
            if let Some(mut request) = continuation_queue::read(&directory)? {
                request.status = "settled".into();
                continuation_queue::save(&directory, &mut request)?;
            }
            return Ok(());
        }
        let fingerprint = progress(&view, &workflow);
        let contract = view
            .record
            .as_ref()
            .is_some_and(|r| r.policy["implementationExecution"]["inPlaceRecoveryVersion"] == 1);
        if contract
            && !autonomous
            && !self
                .tasks
                .production_stage_checkpoint(task, turn, &fingerprint)?
        {
            if let Some(run) = self.auto_progress.lock().get_mut(conversation) {
                run.handled = true;
            }
            if let Some(mut request) = continuation_queue::read(&directory)? {
                request.status = "settled".into();
                continuation_queue::save(&directory, &mut request)?;
            }
            return Ok(());
        }
        let stalled = {
            let mut runs = self.auto_progress.lock();
            let Some(run) = runs.get_mut(conversation) else {
                return Ok(());
            };
            if run.turn != turn || run.handled {
                return Ok(());
            }
            run.handled = true;
            let stalled = !contract
                && !autonomous
                && !view
                    .record
                    .as_ref()
                    .is_some_and(crate::modules::tasks::preflight_recovery_active)
                && run.last_progress.as_ref() == Some(&fingerprint);
            run.last_progress = Some(fingerprint);
            stalled
        };
        let mut request = ContinueRequest {
            id: Uuid::new_v4().to_string(),
            task_id: task.into(),
            conversation_id: conversation.into(),
            native_id: native,
            goal_id: None,
            attempt_id: None,
            repair: None,
            group_id: None,
            answer_revision: 0,
            after_revision: view.record.as_ref().map(|r| r.revision),
            automatic_after: Some(turn.into()),
            status: if stalled { "needsResume" } else { "queued" }.into(),
            owner: self.process_epoch.clone(),
            revision: previous.as_ref().map_or(0, |q| q.revision),
            updated_at: String::new(),
            after_turn: None,
            turn_id: None,
            error: stalled.then(|| {
                "连续执行未记录新的目标或成果进展，自动推进已停止，请核对当前问题。".into()
            }),
        };
        self.queue_tasks
            .lock()
            .insert(conversation.into(), task.into());
        continuation_queue::save(&directory, &mut request)?;
        if !stalled {
            let _ = self.dispatch_or_record(&directory, &mut request)?;
        }
        Ok(())
    }
}

pub(super) fn pending_tools(workflow: &CodexWorkflowSnapshot) -> bool {
    workflow
        .turns
        .iter()
        .find(|t| Some(&t.id) == workflow.current_turn_id.as_ref())
        .is_some_and(|t| {
            t.activities.iter().any(|a| {
                a.source != "desktopCliReport"
                    && a.kind != CodexActivityKind::Analysis
                    && matches!(
                        a.status,
                        CodexActivityStatus::Started | CodexActivityStatus::Progress
                    )
            })
        })
}
