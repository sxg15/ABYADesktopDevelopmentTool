use super::*;
fn goal(f: &Fixture) -> String {
    f.service
        .tasks
        .production_get(&f.task)
        .unwrap()
        .record
        .unwrap()
        .workbench
        .active_goal_id
        .unwrap()
}
#[test]
fn held_task_gate_does_not_block_other_task_reads_or_force_stop() {
    let f = Fixture::new();
    let other = f
        .service
        .tasks
        .create(TaskInput {
            title: "Independent".into(),
            description: String::new(),
        })
        .unwrap();
    let c = f.service.create_conversation(&other.id, None).unwrap();
    let gate = f.service.gate(&f.conversation);
    let guard = gate.normal.lock();
    f.service.task_control(&other.id, &c.id).unwrap();
    f.service.tasks.production_get(&other.id).unwrap();
    f.service.list_conversations(&other.id).unwrap();
    f.service
        .force_stop_generation(&f.task, &f.conversation, 0)
        .unwrap();
    assert!(f.service.runtime_handle(&f.conversation).is_none());
    assert!(f.service.session(&f.conversation).is_err());
    assert!(
        f.service
            .force_stop_generation(&f.task, &f.conversation, 0)
            .is_err()
    );
    drop(guard);
}
#[test]
fn resume_preserves_goal_and_history_and_is_idempotent() {
    let f = Fixture::new();
    let goal = goal(&f);
    let intent = Uuid::new_v4().to_string();
    let before = f
        .service
        .tasks
        .production_get(&f.task)
        .unwrap()
        .record
        .unwrap();
    let first = f
        .service
        .resume_goal(&f.task, &goal, &f.conversation, f.revision(), &intent, None)
        .unwrap();
    assert_eq!(first.status, S::Started);
    let repeated = f
        .service
        .resume_goal(&f.task, &goal, &f.conversation, 0, &intent, None)
        .unwrap();
    assert_eq!(repeated.status, S::Running);
    assert_eq!(f.calls.lock().len(), 1);
    f.service
        .mark_goal_stopped(&f.task, &f.conversation, "interrupted");
    f.complete();
    let next = Uuid::new_v4().to_string();
    f.service
        .resume_goal(&f.task, &goal, &f.conversation, f.revision(), &next, None)
        .unwrap();
    let after = f
        .service
        .tasks
        .production_get(&f.task)
        .unwrap()
        .record
        .unwrap();
    assert_eq!(before.workbench.goals.len(), after.workbench.goals.len());
    assert_eq!(
        after.workbench.active_goal_id.as_deref(),
        Some(goal.as_str())
    );
    assert_eq!(after.workbench.attempts.len(), 2);
    assert_eq!(after.workbench.attempts[0].status, "interrupted");
    assert_eq!(
        after.workbench.attempts[1].predecessor_id.as_deref(),
        Some(intent.as_str())
    );
    f.service
        .tasks
        .production_attempt_status(&f.task, &intent, "running", None, "late")
        .unwrap();
    assert_eq!(
        f.service
            .tasks
            .production_attempt(&f.task, &intent)
            .unwrap()
            .unwrap()
            .status,
        "interrupted"
    );
}
#[test]
fn unsent_crash_window_recovers_same_intent_and_human_gates_block_dispatch() {
    let f = Fixture::new();
    let goal = goal(&f);
    let id = Uuid::new_v4().to_string();
    let a = f
        .service
        .attempt_template(&id, &goal, &f.conversation, &f.native);
    f.service
        .tasks
        .production_begin_attempt(&f.task, None, a)
        .unwrap();
    f.question("gate");
    let result = f
        .service
        .resume_goal(&f.task, &goal, &f.conversation, 0, &id, None)
        .unwrap();
    assert_eq!(result.status, S::WaitingForAnswers);
    assert!(f.calls.lock().is_empty());
    assert_eq!(
        f.service
            .tasks
            .production_get(&f.task)
            .unwrap()
            .record
            .unwrap()
            .workbench
            .attempts
            .len(),
        1
    );
}
#[test]
fn unknown_dispatch_requires_review_after_force_stop() {
    let f = Fixture::new();
    let id = Uuid::new_v4().to_string();
    let mut a = f
        .service
        .attempt_template(&id, &goal(&f), &f.conversation, &f.native);
    a.status = "sending".into();
    f.service
        .tasks
        .production_begin_attempt(&f.task, None, a)
        .unwrap();
    f.service.force_stop(&f.task, &f.conversation).unwrap();
    assert_eq!(
        f.service
            .tasks
            .production_attempt(&f.task, &id)
            .unwrap()
            .unwrap()
            .status,
        "needsReview"
    );
}
#[test]
fn force_during_pending_dispatch_does_not_accept_late_success() {
    let f = Fixture::new();
    let goal = goal(&f);
    let id = Uuid::new_v4().to_string();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    *f.start_gate.lock() = Some((entered_tx, release_rx));
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            f.service
                .resume_goal(&f.task, &goal, &f.conversation, f.revision(), &id, None)
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        f.service
            .force_stop_generation(&f.task, &f.conversation, 0)
            .unwrap();
        release_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap().unwrap().status, S::Paused);
    });
    assert!(f.service.runtime_handle(&f.conversation).is_none());
    assert_eq!(
        f.service
            .tasks
            .production_attempt(&f.task, &id)
            .unwrap()
            .unwrap()
            .status,
        "needsReview"
    );
}
#[cfg(windows)]
#[test]
fn restarted_execution_reconciles_process_identity_without_completing_goal() {
    use super::super::process_identity;
    for (identity, expected) in [
        (Some(1), "interrupted"),
        (None, "needsReview"),
        (process_identity::stamp(std::process::id()), "reconciling"),
    ] {
        let f = Fixture::new();
        let goal = goal(&f);
        let id = Uuid::new_v4().to_string();
        let mut a = f
            .service
            .attempt_template(&id, &goal, &f.conversation, &f.native);
        a.process_epoch = "previous-app".into();
        a.owner_created = identity;
        a.backend_pid = None;
        a.backend_created = None;
        a.status = "running".into();
        a.turn_id = Some("old-turn".into());
        f.service
            .tasks
            .production_begin_attempt(&f.task, None, a)
            .unwrap();
        f.service.runtime_servers.lock().remove(&f.conversation);
        let view = f.service.reconcile_task_execution(&f.task).unwrap();
        let r = view.record.unwrap();
        assert_eq!(r.workbench.attempts[0].status, expected);
        assert!(
            !["completed", "accepted"].contains(
                &r.workbench
                    .goals
                    .iter()
                    .find(|g| g.id == goal)
                    .unwrap()
                    .status
                    .as_str()
            )
        );
        assert!(f.calls.lock().is_empty());
    }
}
#[test]
fn stale_force_confirmation_cannot_cancel_new_generation_tools() {
    let f = Fixture::new();
    f.service.gate(&f.conversation).invalidate();
    let called = std::sync::atomic::AtomicBool::new(false);
    assert!(
        f.service
            .force_stop_with_cancel(&f.task, &f.conversation, 0, &|| {
                called.store(true, std::sync::atomic::Ordering::SeqCst);
            })
            .is_err()
    );
    assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
    assert!(f.service.runtime_handle(&f.conversation).is_some());
}
#[test]
fn rejected_background_attachment_preserves_existing_generation() {
    let f = Fixture::new();
    f.service.sessions.lock().remove(&f.conversation);
    f.busy("still-running");
    let generation = f.service.gate(&f.conversation).epoch();
    let result = f
        .service
        .open(&f.task, &f.conversation, 80, 24, Channel::new(|_| Ok(())));
    assert!(result.is_err());
    assert_eq!(f.service.gate(&f.conversation).epoch(), generation);
    assert!(
        f.service
            .runtime_handle(&f.conversation)
            .unwrap()
            .is_alive()
    );
}
