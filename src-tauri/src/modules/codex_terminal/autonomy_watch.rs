use super::*;
#[cfg(not(test))]
impl CodexTerminalService {
    pub(super) fn watch_autonomous_dependencies(&self) {
        let service = self.clone();
        std::thread::spawn(move || {
            while !service
                .shutting_down
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                std::thread::sleep(Duration::from_secs(5));
                let bindings = service.queue_tasks.lock().clone();
                for (conversation, task) in bindings {
                    if service
                        .shutting_down
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        return;
                    }
                    if service.cancelled.lock().contains(&conversation)
                        || service.stopping.lock().contains(&conversation)
                        || !service
                            .runtime_handle(&conversation)
                            .is_some_and(|s| s.is_alive())
                    {
                        continue;
                    }
                    // Only interrupt a known owned turn at a tool-free checkpoint.
                    crate::foundation::cli_sessions::renew_owned("codex", &task, &conversation);
                    // In-flight or unknown writes must finish/reconcile before dispatching again.
                    if service
                        .tasks
                        .production_execution_budget_due(&task)
                        .unwrap_or(false)
                    {
                        let worker = service.clone();
                        let task = task.clone();
                        let conversation = conversation.clone();
                        std::thread::spawn(move || {
                            let _ = worker.checkpoint_autonomy_budget(&task, &conversation);
                        });
                        continue;
                    }
                    let Ok(Some(revision)) = service.tasks.production_execution_wakeable(&task)
                    else {
                        continue;
                    };
                    let Ok(t) = service.tasks.get_readonly(&task) else {
                        continue;
                    };
                    let directory =
                        conversation_directory(Path::new(&t.workspace_path), &conversation);
                    let Ok(Some(q)) = continuation_queue::read(&directory) else {
                        continue;
                    };
                    if q.owner != service.process_epoch || q.status != "settled" {
                        continue;
                    }
                    // Dispatch revalidates the native idle state, generations, approvals and pause flags.
                    let _ = service.wake_owned_autonomy(&task, &conversation, revision);
                }
            }
        });
    }
}

impl CodexTerminalService {
    pub(super) fn checkpoint_autonomy_budget(
        &self,
        task: &str,
        conversation: &str,
    ) -> AppResult<()> {
        let gate = self.gate(conversation);
        let Some(_guard) = gate.normal.try_lock() else {
            return Ok(());
        };
        if self.cancelled.lock().contains(conversation)
            || self.stopping.lock().contains(conversation)
            || self.shutting_down.load(std::sync::atomic::Ordering::SeqCst)
            || !self.tasks.production_execution_budget_due(task)?
        {
            return Ok(());
        }
        let workflow = self.workflow(task, conversation)?;
        if workflow.terminal_interaction || super::auto_progress::pending_tools(&workflow) {
            return Ok(());
        }
        let Some(turn) = workflow.turns.iter().find(|t| {
            Some(&t.id) == workflow.current_turn_id.as_ref()
                && matches!(
                    t.status,
                    CodexWorkflowTurnStatus::InProgress | CodexWorkflowTurnStatus::WaitingForPlan
                )
        }) else {
            return Ok(());
        };
        let metadata = self.conversation(task, conversation)?;
        let Some(native) = metadata.native_session_id.filter(|_| !metadata.archived) else {
            return Ok(());
        };
        let Some(server) = self.runtime_handle(conversation).filter(|s| s.is_alive()) else {
            return Ok(());
        };
        {
            let mut runs = self.auto_progress.lock();
            let Some(run) = runs
                .get_mut(conversation)
                .filter(|r| r.turn == turn.id && !r.handled && !r.budget_interrupt)
            else {
                return Ok(());
            };
            run.budget_interrupt = true;
        }
        // Never use the user-pause path: its cancellation flag must remain authoritative.
        // An uncertain interrupt is not resent; only a matching terminal event may continue.
        server.request(
            "turn/interrupt",
            json!({"threadId":native,"turnId":turn.id}),
        )?;
        Ok(())
    }
    pub(super) fn wake_owned_autonomy(
        &self,
        task: &str,
        conversation: &str,
        revision: u64,
    ) -> AppResult<()> {
        let gate = self.gate(conversation);
        let Some(_guard) = gate.normal.try_lock_for(Duration::from_secs(2)) else {
            return Ok(());
        };
        if self.cancelled.lock().contains(conversation)
            || self.stopping.lock().contains(conversation)
            || self.shutting_down.load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(());
        }
        let t = self.tasks.get_readonly(task)?;
        let directory = conversation_directory(Path::new(&t.workspace_path), conversation);
        let Some(old) = continuation_queue::read(&directory)? else {
            return Ok(());
        };
        if old.owner != self.process_epoch
            || old.status != "settled"
            || old.task_id != task
            || old.conversation_id != conversation
        {
            return Ok(());
        }
        if self.tasks.production_execution_wakeable(task)?.is_none() {
            return Ok(());
        }
        let Some(predecessor) = old.turn_id.clone().or(old.automatic_after.clone()) else {
            return Ok(());
        };
        let metadata = self.conversation(task, conversation)?;
        if metadata.archived || metadata.native_session_id.as_deref() != Some(&old.native_id) {
            return Ok(());
        }
        let mut q = continuation_queue::ContinueRequest {
            id: Uuid::new_v4().to_string(),
            task_id: task.into(),
            conversation_id: conversation.into(),
            native_id: old.native_id,
            group_id: None,
            answer_revision: 0,
            after_revision: Some(revision),
            automatic_after: Some(predecessor),
            goal_id: None,
            attempt_id: None,
            repair: None,
            status: "queued".into(),
            owner: self.process_epoch.clone(),
            revision: old.revision,
            updated_at: String::new(),
            after_turn: None,
            turn_id: None,
            error: None,
        };
        continuation_queue::save(&directory, &mut q)?;
        self.dispatch_or_record(&directory, &mut q)?;
        Ok(())
    }
}
