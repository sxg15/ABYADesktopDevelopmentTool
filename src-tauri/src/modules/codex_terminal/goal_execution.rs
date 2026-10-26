use super::process_identity::{self, Liveness};
use super::*;
use crate::modules::tasks::{ProductionView, WorkbenchAttempt};

impl CodexTerminalService {
    pub(super) fn attempt_template(
        &self,
        id: &str,
        goal: &str,
        conversation: &str,
        native: &str,
    ) -> WorkbenchAttempt {
        let backend_pid = self
            .runtime_handle(conversation)
            .as_ref()
            .and_then(AppServerHandle::pid);
        WorkbenchAttempt {
            id: id.into(),
            goal_id: goal.into(),
            conversation_id: conversation.into(),
            native_id: native.into(),
            process_epoch: self.process_epoch.clone(),
            owner_pid: std::process::id(),
            owner_created: process_identity::stamp(std::process::id()),
            backend_pid,
            backend_created: backend_pid.and_then(process_identity::stamp),
            generation: self.gate(conversation).epoch(),
            status: "queued".into(),
            ..Default::default()
        }
    }
    pub fn reconcile_task_execution(&self, task: &str) -> AppResult<ProductionView> {
        let t = self.tasks.get(task)?;
        let view = self.tasks.production_attention(&t)?;
        if t.status != crate::modules::tasks::TaskStatus::Active {
            return self.tasks.production_get(task);
        }
        for attempt in view
            .record
            .iter()
            .flat_map(|r| r.workbench.attempts.iter())
            .filter(|a| {
                matches!(
                    a.status.as_str(),
                    "queued" | "sending" | "running" | "reconciling" | "needsReview"
                )
            })
        {
            let local = self.runtime_handle(&attempt.conversation_id);
            if let Some(server) = local.filter(|s| s.is_alive()) {
                if let Ok(snapshot) = server.request(
                    "thread/read",
                    json!({"threadId":attempt.native_id,"includeTurns":true}),
                ) {
                    self.reconcile_snapshot(
                        task,
                        &attempt.conversation_id,
                        &attempt.native_id,
                        &snapshot,
                    )?;
                }
                continue;
            }
            if attempt.process_epoch == self.process_epoch && attempt.status == "queued" {
                continue;
            }
            let owner = process_identity::known(attempt.owner_pid, attempt.owner_created);
            let backend = attempt.backend_pid.map_or(Liveness::Dead, |pid| {
                process_identity::known(pid, attempt.backend_created)
            });
            let (status, reason) = if backend == Liveness::Alive
                || owner == Liveness::Alive && attempt.backend_pid.is_none()
            {
                (
                    "reconciling",
                    "原执行进程仍存在，需接回核对，不会启动第二份。",
                )
            } else if backend == Liveness::Unknown
                || owner == Liveness::Unknown && attempt.backend_pid.is_none()
            {
                (
                    "needsReview",
                    "无法可靠核对旧执行身份，请核对成果后再恢复。",
                )
            } else if attempt.status == "sending" && attempt.turn_id.is_none() {
                (
                    "needsReview",
                    "派发结果未确认，可能已有操作完成，请先核对，不能直接重放。",
                )
            } else {
                let finished = self
                    .workflow(task, &attempt.conversation_id)
                    .ok()
                    .is_some_and(|w| {
                        w.turns.iter().any(|t| {
                            Some(&t.id) == attempt.turn_id.as_ref()
                                && t.status == CodexWorkflowTurnStatus::Completed
                        })
                    });
                if finished {
                    ("finished", "原轮次已结束；业务目标仍按原记录保留。")
                } else {
                    (
                        "interrupted",
                        "已确认原执行进程退出，未完成的子流程被中断。",
                    )
                }
            };
            self.tasks
                .production_attempt_status(task, &attempt.id, status, None, reason)?;
            if attempt.process_epoch != self.process_epoch
                && matches!(status, "interrupted" | "finished" | "needsReview")
            {
                self.tasks.production_execution_observation_gap(task)?;
            }
        }
        self.tasks.production_get(task)
    }
    pub(super) fn reconcile_snapshot(
        &self,
        task: &str,
        conversation: &str,
        native: &str,
        snapshot: &serde_json::Value,
    ) -> AppResult<()> {
        if snapshot["thread"]["id"] != native {
            return Err(AppError::validation("原生执行身份不匹配。"));
        }
        let view = self.tasks.production_get(task)?;
        for a in view
            .record
            .iter()
            .flat_map(|r| r.workbench.attempts.iter())
            .filter(|a| {
                a.conversation_id == conversation
                    && a.native_id == native
                    && matches!(
                        a.status.as_str(),
                        "running" | "sending" | "reconciling" | "needsReview"
                    )
            })
        {
            let turn = a.turn_id.clone().or_else(|| {
                let t = self.tasks.get(task).ok()?;
                let dir = conversation_directory(Path::new(&t.workspace_path), conversation);
                continuation_queue::read(&dir)
                    .ok()
                    .flatten()
                    .filter(|q| q.id == a.id)
                    .and_then(|q| q.turn_id)
                    .or_else(|| {
                        let receipt: serde_json::Value = serde_json::from_slice(
                            &std::fs::read(dir.join(format!("task-continue-{}.json", a.id)))
                                .ok()?,
                        )
                        .ok()?;
                        (receipt["nativeSessionId"] == native)
                            .then(|| receipt["turnId"].as_str().map(str::to_owned))
                            .flatten()
                    })
            });
            if let Some(t) = snapshot["thread"]["turns"]
                .as_array()
                .and_then(|ts| ts.iter().find(|t| t["id"].as_str() == turn.as_deref()))
            {
                let status = match t["status"].as_str() {
                    Some("completed") => "finished",
                    Some("interrupted" | "failed") => "interrupted",
                    Some("inProgress") => "running",
                    _ => continue,
                };
                self.tasks.production_attempt_status(
                    task,
                    &a.id,
                    status,
                    turn.as_deref(),
                    "已核对原生轮次状态。",
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn observe_goal_turn(&self, native: &str, turn: &str, status: &str) {
        let Some((task, conversation)) = self.thread_conversations.lock().get(native).cloned()
        else {
            return;
        };
        let Ok(t) = self.tasks.get(&task) else {
            return;
        };
        let Ok(view) = self.tasks.production_attention(&t) else {
            return;
        };
        let Some(r) = view.record else {
            return;
        };
        let matching = r.workbench.attempts.iter().rev().find(|a| {
            a.conversation_id == conversation
                && a.native_id == native
                && a.process_epoch == self.process_epoch
                && (a.turn_id.as_deref() == Some(turn)
                    || a.turn_id.is_none() && matches!(a.status.as_str(), "queued" | "sending"))
        });
        if let Some(attempt) = matching {
            let _ =
                self.tasks
                    .production_attempt_status(&task, &attempt.id, status, Some(turn), "");
        } else if status == "running"
            && let Some(goal) = r.workbench.active_goal_id
        {
            let mut attempt =
                self.attempt_template(&Uuid::new_v4().to_string(), &goal, &conversation, native);
            attempt.turn_id = Some(turn.into());
            attempt.status = "running".into();
            let _ = self
                .tasks
                .production_begin_attempt(&task, Some(r.revision), attempt);
        }
    }
    pub(super) fn mark_goal_stopped(&self, task: &str, conversation: &str, status: &str) {
        let _ = self.tasks.production_execution_pause(task);
        let Ok(view) = self
            .tasks
            .get_readonly(task)
            .and_then(|t| self.tasks.production_attention(&t))
        else {
            return;
        };
        for a in view
            .record
            .iter()
            .flat_map(|r| r.workbench.attempts.iter())
            .filter(|a| {
                a.conversation_id == conversation
                    && matches!(a.status.as_str(), "running" | "queued" | "sending")
            })
        {
            let _ = self.tasks.production_attempt_status(
                task,
                &a.id,
                if a.status == "sending" && a.turn_id.is_none() {
                    "needsReview"
                } else {
                    status
                },
                None,
                if a.status == "sending" && a.turn_id.is_none() {
                    "执行已停止，但派发结果未确认，请先核对成果。"
                } else {
                    "执行已停止，目标与成果保留。"
                },
            );
        }
    }
}

impl CodexTerminalService {
    pub fn continue_app(
        &self,
        task: &str,
        conversation: &str,
        revision: Option<u64>,
        intent: Option<&str>,
    ) -> AppResult<IntakeContinuation> {
        if revision.is_none()
            && let Some(r) = self.tasks.production_get(task)?.record
            && let Some(goal) = r.workbench.active_goal_id
        {
            return self.resume_goal(
                task,
                &goal,
                conversation,
                r.revision,
                intent.unwrap_or(&Uuid::new_v4().to_string()),
                None,
            );
        }
        self.continue_after_revision(task, conversation, revision)
    }
    pub fn resume_goal(
        &self,
        task: &str,
        goal: &str,
        conversation: &str,
        expected: u64,
        intent: &str,
        note: Option<&str>,
    ) -> AppResult<IntakeContinuation> {
        use super::intake_receipt::IntakeContinueStatus as S;
        Uuid::parse_str(intent).map_err(|_| AppError::validation("恢复请求标识无效。"))?;
        let record = self
            .tasks
            .production_get(task)?
            .record
            .ok_or_else(|| AppError::validation("尚未启用制作流程。"))?;
        let bound = record
            .workbench
            .attempts
            .iter()
            .rev()
            .find(|a| a.goal_id == goal)
            .map(|a| a.conversation_id.as_str())
            .unwrap_or(conversation)
            .to_owned();
        let gate = self.gate(&bound);
        let _guard = gate
            .normal
            .try_lock_for(Duration::from_secs(2))
            .ok_or_else(|| {
                AppError::new(
                    "codex_control_busy",
                    "当前会话正在处理，请稍后重试；其他任务可继续使用。",
                    "not_executed",
                )
            })?;
        let generation = gate.epoch();
        let record = self
            .tasks
            .production_get(task)?
            .record
            .ok_or_else(|| AppError::validation("缺少制作记录。"))?;
        let metadata = self.conversation(task, &bound)?;
        if metadata.archived {
            return Err(AppError::validation("原会话已归档，请先恢复会话。"));
        }
        let native = metadata
            .native_session_id
            .ok_or_else(|| AppError::validation("请先打开原会话连接，再继续此子流程。"))?;
        if let Some(old) = self.tasks.production_attempt(task, intent)? {
            if old.goal_id != goal || old.conversation_id != bound || old.native_id != native {
                return Err(AppError::validation("恢复请求与原目标不匹配。"));
            }
            if old.status != "queued" {
                return Ok(attempt_response(&old));
            }
            let t = self.tasks.get(task)?;
            let dir = conversation_directory(Path::new(&t.workspace_path), &bound);
            if let Some(mut q) = continuation_queue::read(&dir)? {
                if q.id == old.id {
                    return self.dispatch_or_record(&dir, &mut q);
                }
                return Err(AppError::validation(
                    "存在另一份续接回执，请先核对，未覆盖已有请求。",
                ));
            }
            // Queued is persisted before dispatch; sending is persisted before RPC.
            // Only this definitely-unsent crash window may rebuild the same receipt.
            let mut q = resume_request(
                intent,
                task,
                goal,
                &bound,
                &native,
                record.revision,
                &self.process_epoch,
                0,
            );
            gate.check(generation)?;
            continuation_queue::save(&dir, &mut q)?;
            return self.dispatch_or_record(&dir, &mut q);
        }
        if record.revision != expected {
            return Err(AppError::new(
                "conflict",
                "任务已更新，请刷新后再恢复。",
                "",
            ));
        }
        let view = self.reconcile_task_execution(task)?;
        let r = view
            .record
            .as_ref()
            .ok_or_else(|| AppError::validation("缺少制作记录。"))?;
        if let Some(wait) = continuation_queue::waiting(Some(&view)) {
            return Ok(IntakeContinuation {
                status: wait,
                conversation_id: bound,
                native_session_id: native,
                turn_id: None,
                request_id: None,
            });
        }
        if !crate::modules::tasks::preflight_recovery_active(r)
            && !crate::modules::tasks::preflight_recovery_resumable(r)
            && !crate::modules::tasks::preflight_recovery_manual(r)
            && view
                .stage_statuses
                .get(&r.current_stage)
                .is_some_and(|s| s == "blocked")
            || (!crate::modules::tasks::autonomous_implementation(r)
                && r.feedback.iter().any(|f| f["status"] == "awaiting-recheck"))
        {
            return Err(AppError::validation(
                "请先处理当前阻塞或人工复验，再恢复子流程。",
            ));
        }
        if let Some(old) = r
            .workbench
            .attempts
            .iter()
            .rev()
            .find(|a| a.goal_id == goal)
        {
            if old.native_id != native {
                return Err(AppError::validation("原生会话绑定已变化，请先核对历史。"));
            }
            if matches!(old.status.as_str(), "running" | "queued") {
                return Ok(attempt_response(old));
            }
            if matches!(
                old.status.as_str(),
                "needsReview" | "reconciling" | "sending"
            ) {
                let note = note.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
                    AppError::validation("执行结果待确认，请先核对成果并填写核对说明。")
                })?;
                let server = self
                    .runtime_handle(&bound)
                    .filter(|s| s.is_alive())
                    .ok_or_else(|| AppError::validation("请先连接原会话，核对原生执行已结束。"))?;
                server.ensure_idle(&native)?;
                self.tasks.production_attempt_status(
                    task,
                    &old.id,
                    "interrupted",
                    None,
                    &format!("用户核对后恢复：{note}"),
                )?;
            }
        }
        if let Some(server) = self.runtime_handle(&bound).filter(|s| s.is_alive()) {
            // A legacy goal may have no journal while its original turn is still live.
            // Never enqueue a duplicate continuation behind that unbound turn.
            server.ensure_idle(&native)?;
        }
        if crate::modules::tasks::preflight_recovery_manual(r)
            && !crate::modules::tasks::preflight_recovery_active(r)
            && !crate::modules::tasks::preflight_recovery_resumable(r)
            && view
                .stage_statuses
                .get(&r.current_stage)
                .is_some_and(|s| s == "blocked")
            && !self.tasks.production_prepare_preflight_recovery(
                task,
                &format!("user-resume:{intent}"),
                true,
            )?
        {
            return Err(AppError::validation(
                "技术恢复预算已结束，请先核对问题中的恢复条件。",
            ));
        }
        self.tasks.production_resume_preflight(task)?;
        let attempt = self.attempt_template(intent, goal, &bound, &native);
        self.tasks.production_begin_attempt(task, None, attempt)?;
        let t = self.tasks.get(task)?;
        let dir = conversation_directory(Path::new(&t.workspace_path), &bound);
        let previous = continuation_queue::read(&dir)?;
        let mut q = resume_request(
            intent,
            task,
            goal,
            &bound,
            &native,
            r.revision,
            &self.process_epoch,
            previous.as_ref().map_or(0, |q| q.revision),
        );
        {
            let _publication = gate.publication.lock();
            gate.check(generation)?;
            self.cancelled.lock().remove(&bound);
        }
        self.queue_tasks.lock().insert(bound, task.into());
        continuation_queue::save(&dir, &mut q)?;
        let result = self.dispatch_or_record(&dir, &mut q)?;
        if result.status == S::NeedsReview {
            let _ = self.tasks.production_attempt_status(
                task,
                intent,
                "needsReview",
                None,
                "派发结果未确认，请先核对。",
            );
        }
        Ok(result)
    }
}
fn attempt_response(a: &WorkbenchAttempt) -> IntakeContinuation {
    use super::intake_receipt::IntakeContinueStatus as S;
    IntakeContinuation {
        status: match a.status.as_str() {
            "running" => S::Running,
            "queued" => S::NeedsConnection,
            "finished" => S::Completed,
            "paused" | "interrupted" => S::Paused,
            _ => S::NeedsReview,
        },
        conversation_id: a.conversation_id.clone(),
        native_session_id: a.native_id.clone(),
        turn_id: a.turn_id.clone(),
        request_id: Some(a.id.clone()),
    }
}

#[allow(clippy::too_many_arguments)]
fn resume_request(
    id: &str,
    task: &str,
    goal: &str,
    conversation: &str,
    native: &str,
    after_revision: u64,
    owner: &str,
    revision: u64,
) -> continuation_queue::ContinueRequest {
    continuation_queue::ContinueRequest {
        id: id.into(),
        task_id: task.into(),
        conversation_id: conversation.into(),
        native_id: native.into(),
        goal_id: Some(goal.into()),
        attempt_id: Some(id.into()),
        repair: None,
        group_id: None,
        answer_revision: 0,
        after_revision: Some(after_revision),
        automatic_after: None,
        status: "queued".into(),
        owner: owner.into(),
        revision,
        updated_at: String::new(),
        after_turn: None,
        turn_id: None,
        error: None,
    }
}
