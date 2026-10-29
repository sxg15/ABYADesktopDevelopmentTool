use super::*;
fn autonomous() -> Fixture {
    let f = Fixture::new();
    f.plan();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy["implementationExecution"] = json!({"mode":"autonomous"});
            Ok(())
        })
        .unwrap();
    f
}
#[test]
fn finalization_with_valid_baseline_enters_review_even_with_recorded_optional_gaps() {
    let f = autonomous();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy["iterationMode"] = json!("human-feedback");
            Ok(())
        })
        .unwrap();
    f.apply("save-issue",json!({"id":"unavailable-input","stage":"implementation","kind":"checkpoint","status":"open","description":"unverified platform focus"})).unwrap();
    unit(&f, "input", json!([]), json!(["check"]));
    next(&f, "first", false);
    report(&f, "input", "deferred");
    let finalizer = next(&f, "needs-finalization", true).unwrap();
    assert_eq!(finalizer.kind, "finalization");
    f.evidence("baseline-contract", "json", "state");
    for id in ["lua", "core-loop", "save-reload"] {
        f.apply("save-self-test",json!({"id":id,"status":"passed","observations":"synthetic contract evidence only","evidenceIds":["baseline-contract"]})).unwrap();
    }
    let approvals = f.record().approvals.len();
    report(&f, &finalizer.id, "succeeded");
    assert!(next(&f, "finalized", true).is_none());
    let r = f.record();
    assert_eq!(r.current_stage, "review");
    assert_eq!(r.stages["implementation"], "execution-converged");
    assert_eq!(r.stages["review"], "awaiting-feedback");
    assert_eq!(r.issues[0]["status"], "open");
    assert_eq!(r.approvals.len(), approvals);
    assert_ne!(r.stages["delivery"], "passed");
}
#[test]
fn preflight_recovery_requires_runtime_evidence_and_preserves_human_gates_and_budget() {
    let f = Fixture::new();
    f.doc("requirements", "Approved game scope");
    f.accept("requirements");
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy["preflightRecovery"] = json!({"enabled":true});
            r.current_stage = "resources".into();
            r.stages.insert("resources".into(), "blocked".into());
            Ok(())
        })
        .unwrap();
    f.apply("save-issue",json!({"id":"startup","stage":"resources","kind":"blocker","status":"open","description":"editor not ready","resumeWhen":"verified ready"})).unwrap();
    assert!(
        !f.service
            .production_prepare_preflight_recovery(&f.id, "unverified", false)
            .unwrap()
    );
    assert!(
        f.service
            .production_prepare_preflight_recovery(&f.id, "first", true)
            .unwrap()
    );
    assert!(
        f.service
            .production_prepare_preflight_recovery(&f.id, "first", true)
            .unwrap()
    );
    assert_eq!(f.record().workbench.execution.preflight.attempts, 1);
    assert!(validation::clear_issues(&f.record(), "resources").is_err());
    f.service.production_execution_pause(&f.id).unwrap();
    assert!(!super::super::preflight_recovery_active(&f.record()));
    assert!(super::super::preflight_recovery_resumable(&f.record()));
    f.service.production_resume_preflight(&f.id).unwrap();
    assert!(
        f.service
            .production_prepare_preflight_recovery(&f.id, "second", false)
            .unwrap()
    );
    assert!(
        !f.service
            .production_prepare_preflight_recovery(&f.id, "third", true)
            .unwrap()
    );
    assert_eq!(f.record().workbench.execution.preflight.state, "exhausted");
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.stages
                .insert("resources".into(), "awaiting-confirmation".into());
            Ok(())
        })
        .unwrap();
    assert!(
        !f.service
            .production_prepare_preflight_recovery(&f.id, "approval", true)
            .unwrap()
    );
}
#[test]
fn productive_budget_checkpoints_keep_work_route_and_do_not_spend_fault_budget() {
    let f = autonomous();
    f.apply("save-issue",json!({"id":"logic","stage":"implementation","kind":"defect","status":"open","description":"ordinary correction"})).unwrap();
    let goal = f.record().workbench.active_goal_id.unwrap();
    f.apply("save-work-unit",json!({"id":"code","goalId":goal,"kind":"deployment","title":"save remaining code","dependsOn":[],"issueIds":["logic"],"routes":["normal","alternate"]})).unwrap();
    next(&f, "start", false);
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.workbench.execution.units[0].attempt_started_at =
                (Utc::now() - chrono::Duration::minutes(16)).to_rfc3339();
            Ok(())
        })
        .unwrap();
    assert!(
        f.service
            .production_execution_checkpoint(&f.id, "checkpoint:one")
            .unwrap()
    );
    let resumed = next(&f, "dispatch:two", false).unwrap();
    assert_eq!(resumed.id, "code");
    assert_eq!(resumed.route, 0);
    assert_eq!(resumed.attempts, 1);
    assert!(f.record().workbench.execution.recovery_ms.is_empty());
    assert!(!f.service.production_execution_budget_due(&f.id).unwrap());
}
#[test]
fn budget_migration_requeues_only_misclassified_work_and_preserves_elapsed_history() {
    let f = autonomous();
    unit(&f, "visual", json!([]), json!(["verify"]));
    unit(&f, "dependent", json!(["visual"]), json!(["finish"]));
    unit(&f, "capability", json!([]), json!(["focus"]));
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            let units = &mut r.workbench.execution.units;
            units[0].status = "deferred".into();
            units[0].elapsed_ms = 123456;
            units[0].reason = "本项或关联故障的累计恢复预算已用完，保留成果并继续独立工作。".into();
            units[1].status = "deferred".into();
            units[1].reason = "前置工作未完成；本轮暂缓部署或验证，已有成果保留。".into();
            units[2].status = "deferred".into();
            units[2].reason = "No supported focus capability".into();
            super::super::autonomy_budget::migrate_budget(r);
            Ok(())
        })
        .unwrap();
    let r = f.record();
    assert_eq!(r.workbench.execution.state, "paused");
    assert_eq!(r.workbench.execution.units[0].status, "pending");
    assert_eq!(r.workbench.execution.units[0].elapsed_ms, 123456);
    assert_eq!(r.workbench.execution.units[0].budget_epoch_ms, 123456);
    assert_eq!(r.workbench.execution.units[1].status, "pending");
    assert_eq!(r.workbench.execution.units[2].status, "deferred");
}
#[test]
fn fixture_snapshots_never_replace_candidate_and_activation_checks_saved_bytes() {
    let f = autonomous();
    let sources = || VersionSources {
        archive_path: f.workspace.join("fixture-archive"),
        player_path: f.workspace.join("fixture-player.exe"),
        archive_guid: "archive".into(),
        level_guid: "level".into(),
        instance_id: "fixture".into(),
    };
    let saved = f
        .service
        .production_capture_version(&f.id, f.record().revision, "candidate", sources())
        .unwrap()
        .record
        .unwrap();
    std::fs::write(
        f.workspace.join("fixture-archive/Main.PBArc"),
        "fixed sequence fixture",
    )
    .unwrap();
    let result = f
        .service
        .production_capture_version_for_purpose(
            &f.id,
            saved.revision,
            "fixed-sequence",
            sources(),
            "fixture",
        )
        .unwrap()
        .record
        .unwrap();
    assert_eq!(result.current_version, saved.current_version);
    assert_eq!(result.version_details, saved.version_details);
    assert_eq!(
        result.workbench.execution.validation_snapshots["fixed-sequence"]["purpose"],
        "fixture"
    );
    // Simulate an older client that had already selected a temporary version.
    f.service
        .production_capture_version(&f.id, result.revision, "legacy-temporary", sources())
        .unwrap();
    let restored = f
        .service
        .production_activate_version(&f.id, f.record().revision, saved.revision, "candidate")
        .unwrap()
        .record
        .unwrap();
    assert_eq!(restored.version_details, saved.version_details);
    assert_eq!(restored.self_tests.len(), saved.self_tests.len());
    let path = PathBuf::from(saved.version_details["archiveSource"].as_str().unwrap());
    std::fs::write(path.join("Main.PBArc"), "changed frozen file").unwrap();
    assert!(
        f.service
            .production_activate_version(&f.id, restored.revision, saved.revision, "candidate")
            .is_err()
    );
}
#[test]
fn elapsed_budget_rotates_route_and_manual_pause_stops_accounting() {
    let f = autonomous();
    unit(&f, "slow", json!([]), json!(["native", "alternative"]));
    next(&f, "start", false);
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.workbench.execution.units[0].attempt_started_at =
                (Utc::now() - chrono::Duration::minutes(16)).to_rfc3339();
            Ok(())
        })
        .unwrap();
    assert!(f.service.production_execution_budget_due(&f.id).unwrap());
    let u = next(&f, "budget", true).unwrap();
    assert_eq!(u.route, 1);
    assert!(u.elapsed_ms >= 16 * 60_000);
    assert!(!f.service.production_execution_budget_due(&f.id).unwrap());
    f.service.production_execution_pause(&f.id).unwrap();
    assert!(
        f.record().workbench.execution.units[0]
            .attempt_started_at
            .is_empty()
    );
    assert!(!f.service.production_execution_budget_due(&f.id).unwrap());
}
#[test]
fn reporting_an_output_does_not_disable_the_native_turn_budget() {
    let f = autonomous();
    unit(&f, "work", json!([]), json!(["code"]));
    next(&f, "start", false);
    report(&f, "work", "succeeded");
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            for e in &mut r.workbench.execution.events {
                if e["kind"] == "scheduled" {
                    e["at"] = json!((Utc::now() - chrono::Duration::minutes(16)).to_rfc3339());
                }
            }
            Ok(())
        })
        .unwrap();
    assert!(f.service.production_execution_budget_due(&f.id).unwrap());
    f.service.production_execution_pause(&f.id).unwrap();
    assert!(!f.service.production_execution_budget_due(&f.id).unwrap());
}
#[test]
fn raw_legacy_blocked_state_cannot_disable_an_enabled_autonomous_policy() {
    let f = autonomous();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.stages.insert("implementation".into(), "blocked".into());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        f.service.production_get(&f.id).unwrap().stage_statuses["implementation"],
        "in-progress"
    );
    assert!(next(&f, "recover", false).is_some());
}
#[test]
fn explicit_23_migration_preserves_identity_approvals_results_and_no_auto_start() {
    let f = autonomous();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy = policy();
            r.policy["workflowVersion"] = json!("2.3.0");
            r.policy
                .as_object_mut()
                .unwrap()
                .remove("implementationExecution");
            r.workflow_version = "2.3.0".into();
            r.stages.insert("implementation".into(), "blocked".into());
            Ok(())
        })
        .unwrap();
    let before = f.record();
    let r = f
        .service
        .production_upgrade(&f.id, before.revision)
        .unwrap()
        .record
        .unwrap();
    assert_eq!(r.workflow_version, "2.10.0");
    assert_eq!(r.workbench.active_goal_id, before.workbench.active_goal_id);
    assert_eq!(r.current_version, before.current_version);
    assert_eq!(
        serde_json::to_value(r.approvals).unwrap(),
        serde_json::to_value(before.approvals).unwrap()
    );
    assert_eq!(r.self_tests.len(), before.self_tests.len());
    assert!(r.workbench.execution.units.is_empty());
    assert_eq!(r.stages["implementation"], "in-progress");
}
#[test]
fn confirmed_diagnosis_requires_reproduction_comparison_and_bound_evidence() {
    let f = autonomous();
    assert!(f.apply("save-issue",json!({"id":"claim","kind":"blocker","stage":"implementation","status":"open","description":"unknown failure","resumeWhen":"healthy","repairTarget":"abya-build","diagnosisStatus":"confirmed"})).is_err());
    f.apply("save-issue",json!({"id":"claim","kind":"blocker","stage":"implementation","status":"open","description":"unknown failure","resumeWhen":"healthy","repairTarget":"abya-build","diagnosisStatus":"suspected"})).unwrap();
}
#[test]
fn recovery_budget_is_shared_across_unit_ids_but_not_independent_work() {
    let f = autonomous();
    f.apply("save-issue",json!({"id":"startup","stage":"implementation","kind":"blocker","status":"open","description":"init failure","resumeWhen":"healthy instance"})).unwrap();
    let goal = f.record().workbench.active_goal_id.unwrap();
    for id in ["repair-a", "repair-b"] {
        f.apply("save-work-unit",json!({"id":id,"goalId":goal,"title":id,"kind":"recovery","dependsOn":[],"issueIds":["startup"],"routes":["recover"]})).unwrap();
    }
    unit(&f, "code", json!([]), json!(["write code"]));
    next(&f, "start", false);
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.workbench.execution.units[0].attempt_started_at =
                (Utc::now() - chrono::Duration::minutes(31)).to_rfc3339();
            Ok(())
        })
        .unwrap();
    assert_eq!(next(&f, "budget", true).unwrap().id, "code");
    let r = f.record();
    assert_eq!(r.workbench.execution.units[0].status, "deferred");
    assert_eq!(r.workbench.execution.units[1].status, "deferred");
}
fn unit(f: &Fixture, id: &str, deps: Value, routes: Value) {
    let goal = f.record().workbench.active_goal_id.unwrap();
    f.apply(
        "save-work-unit",
        json!({"id":id,"goalId":goal,"title":id,"dependsOn":deps,"issueIds":[],"routes":routes}),
    )
    .unwrap();
}
fn next(f: &Fixture, event: &str, advance: bool) -> Option<super::super::autonomy::WorkUnit> {
    f.service
        .production_schedule(&f.id, event, advance)
        .unwrap()
}
fn finish_gaps(f: &Fixture, event: &str) {
    let finalizer =
        next(f, event, true).expect("finalization runs even with deferred dependencies");
    assert_eq!(finalizer.kind, "finalization");
    report(f, &finalizer.id, "succeeded");
    assert!(next(f, &format!("{event}-finalized"), true).is_none());
}
fn report(f: &Fixture, id: &str, status: &str) {
    files::write_file(
        &f.workspace,
        "artifacts/unit-result.json",
        b"controlled result",
    )
    .unwrap();
    f.apply("report-work-unit",json!({"id":id,"status":status,"reason":"controlled outcome","resultPaths":["artifacts/unit-result.json"]})).unwrap();
}
#[test]
fn pb_issue_does_not_stop_independent_work_or_erase_delivery_gate() {
    let f = autonomous();
    let approvals = f.record().approvals.len();
    f.apply("save-issue",json!({"id":"platform","stage":"implementation","kind":"blocker","status":"open","description":"missing platform capability","repairTarget":"abya-source","resumeWhen":"new build"})).unwrap();
    assert_eq!(
        f.service.production_get(&f.id).unwrap().stage_statuses["implementation"],
        "in-progress"
    );
    assert!(validation::clear_issues(&f.record(), "implementation").is_err());
    unit(&f, "affected", json!([]), json!(["native", "fallback"]));
    unit(&f, "dependent", json!(["affected"]), json!(["bind output"]));
    unit(&f, "independent", json!([]), json!(["scoring"]));
    assert_eq!(next(&f, "one", false).unwrap().id, "affected");
    f.apply("record-issue-handling",json!({"issueId":"platform","mode":"deferred","reason":"both options unavailable","impact":"visual effect omitted from candidate","affectedUnitIds":["affected"]})).unwrap();
    assert_eq!(next(&f, "two", true).unwrap().id, "independent");
    report(&f, "independent", "succeeded");
    finish_gaps(&f, "three");
    let r = f.record();
    assert_eq!(r.workbench.execution.state, "settledWithGaps");
    assert_eq!(
        r.workbench.execution.summary["units"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(r.issues[0]["status"], "open");
    assert_eq!(r.approvals.len(), approvals);
    assert_eq!(
        r.workbench
            .execution
            .units
            .iter()
            .find(|u| u.id == "dependent")
            .unwrap()
            .status,
        "deferred"
    );
}
#[test]
fn no_progress_changes_route_then_parks_without_fake_completion() {
    let f = autonomous();
    unit(
        &f,
        "stuck",
        json!([]),
        json!(["native", "equivalent fallback"]),
    );
    assert_eq!(next(&f, "start", false).unwrap().route, 0);
    assert_eq!(next(&f, "end1", true).unwrap().attempts, 2);
    assert_eq!(next(&f, "end2", true).unwrap().route, 1);
    assert_eq!(next(&f, "end3", true).unwrap().attempts, 2);
    finish_gaps(&f, "end4");
    assert_eq!(f.record().workbench.execution.units[0].status, "deferred");
    assert!(next(&f, "end4", true).is_none());
    assert_eq!(f.record().stages["implementation"], "in-progress");
    assert!(
        f.apply("complete-stage", json!({"stage":"implementation"}))
            .is_err()
    );
}
#[test]
fn protected_gates_legacy_and_dependency_definition_remain_intact() {
    let f = Fixture::new();
    f.plan();
    assert!(next(&f, "legacy", false).is_none());
    assert!(f.apply("save-work-unit", json!({})).is_err());
    let f = autonomous();
    unit(&f, "first", json!([]), json!(["one"]));
    let goal = f.record().workbench.active_goal_id.unwrap();
    assert!(f.apply("save-work-unit",json!({"id":"cycle","goalId":goal,"title":"cycle","dependsOn":["cycle"],"issueIds":[],"routes":["x"]})).is_err());
    assert!(f.apply("publish-questions", json!({})).is_err());
    f.apply(
        "update-stage",
        json!({"stage":"implementation","summary":"local failure","status":"blocked"}),
    )
    .unwrap();
    assert_eq!(f.record().stages["implementation"], "in-progress");
    assert!(
        f.apply(
            "update-stage",
            json!({"stage":"implementation","summary":"stop","status":"stopped"})
        )
        .is_err()
    );
    std::fs::write(
        f.workspace.join(ROOT).join("plan.md"),
        "modified after approval",
    )
    .unwrap();
    assert!(
        f.service
            .production_schedule(&f.id, "changed-plan", false)
            .is_err()
    );
}
#[test]
fn receipts_preserve_partial_commits_and_reject_changed_inputs_and_stale_continuation() {
    let f = Fixture::new();
    let batch = uuid::Uuid::new_v4().to_string();
    let before = f.record().revision;
    let items = vec![
        json!({"operation":"update-stage","data":{"stage":"requirements","summary":"saved"}}),
        json!({"operation":"approve","data":{}}),
    ];
    let result = f
        .service
        .production_batch(&f.id, before, &batch, &items)
        .unwrap();
    assert_eq!(result["complete"], false);
    assert_eq!(f.record().revision, before + 1);
    let repeated = f
        .service
        .production_batch(&f.id, before, &batch, &items)
        .unwrap();
    assert_eq!(repeated["results"][0]["resultRevision"], before + 1);
    assert_eq!(f.record().revision, before + 1);
    let changed = vec![
        json!({"operation":"update-stage","data":{"stage":"requirements","summary":"DIFFERENT"}}),
    ];
    assert_eq!(
        f.service
            .production_batch(&f.id, before, &batch, &changed)
            .unwrap()["results"][0]["code"],
        "conflict"
    );
    f.apply(
        "update-stage",
        json!({"stage":"requirements","summary":"new human input"}),
    )
    .unwrap();
    let mut continuation = items;
    continuation[1] = json!({"operation":"update-stage","data":{"stage":"requirements","summary":"would overwrite newer input"}});
    let result = f
        .service
        .production_batch(&f.id, before, &batch, &continuation)
        .unwrap();
    assert_eq!(result["results"][1]["code"], "conflict");
    assert_eq!(
        f.record().stage_updates["requirements"]["summary"],
        "new human input"
    );
}
#[test]
fn current_context_is_bounded_and_has_no_full_documents_or_receipt_history() {
    let f = autonomous();
    let before = f.record().revision;
    let current = f.service.production_current(&f.id).unwrap();
    assert_eq!(current["autonomous"], true);
    assert!(current.get("documents").is_none());
    assert!(current.get("receipts").is_none());
    assert_eq!(f.record().revision, before);
}
#[test]
fn wake_requires_changed_conditions_and_respects_pause_and_convergence() {
    let f = autonomous();
    unit(&f, "missing", json!([]), json!(["one"]));
    next(&f, "start", false);
    report(&f, "missing", "deferred");
    finish_gaps(&f, "end");
    assert!(
        f.service
            .production_execution_wakeable(&f.id)
            .unwrap()
            .is_none()
    );
    f.apply(
        "update-stage",
        json!({"stage":"implementation","summary":"different wording only"}),
    )
    .unwrap();
    assert!(
        f.service
            .production_execution_wakeable(&f.id)
            .unwrap()
            .is_none()
    );
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.version_details["playerBuildHash"] = json!("new build");
            Ok(())
        })
        .unwrap();
    assert!(
        f.service
            .production_execution_wakeable(&f.id)
            .unwrap()
            .is_some()
    );
    f.service.production_execution_pause(&f.id).unwrap();
    assert_eq!(f.record().workbench.execution.state, "settledWithGaps");
    // Execution pause belongs to the native cancellation gate. Closing an idle APP
    // must not erase an already consolidated report or require another Continue.
}
#[test]
fn frozen_version_and_acceptance_copy_are_isolated_from_editable_archive() {
    let f = autonomous();
    let source = f.workspace.join("fixture-archive");
    let view = f
        .service
        .production_capture_version(
            &f.id,
            f.record().revision,
            "frozen-v2",
            VersionSources {
                archive_path: source.clone(),
                player_path: f.workspace.join("fixture-player.exe"),
                archive_guid: "archive".into(),
                level_guid: "level".into(),
                instance_id: "fixture".into(),
            },
        )
        .unwrap();
    let r = view.record.unwrap();
    assert_eq!(r.version_details["frozenSnapshot"], true);
    let frozen = PathBuf::from(r.version_details["archiveSource"].as_str().unwrap());
    assert_ne!(
        source.canonicalize().unwrap(),
        frozen.canonicalize().unwrap()
    );
    std::fs::write(source.join("Main.PBArc"), "editor keeps working").unwrap();
    versions::verify(&r.version_details).unwrap();
    let run = PathBuf::from(
        f.service
            .production_acceptance_archive(&f.id, "frozen-v2")
            .unwrap(),
    );
    std::fs::write(run.join("Main.PBArc"), "playtest save").unwrap();
    versions::verify(&r.version_details).unwrap();
    std::fs::write(frozen.join("Main.PBArc"), "tampered snapshot").unwrap();
    assert!(versions::verify(&r.version_details).is_err());
}
#[test]
fn unknown_archive_changes_never_authorize_automatic_inheritance() {
    let old = json!({"files":[{"path":"Main.PBArc","sha256":"old","kind":"unknown"}]});
    let new = json!({"files":[{"path":"Main.PBArc","sha256":"new","kind":"unknown"}]});
    let delta = version_snapshots::difference(&old, &new, false);
    assert_eq!(delta["unknownImpact"], true);
    assert_eq!(delta["mustRetest"].as_array().unwrap().len(), 8);
    assert_eq!(delta["mode"], "shadow");
}
#[test]
fn unrelated_issue_does_not_reset_exhausted_route_budget() {
    let f = autonomous();
    unit(&f, "fixed-scope", json!([]), json!(["single route"]));
    next(&f, "start", false);
    report(&f, "fixed-scope", "deferred");
    finish_gaps(&f, "end");
    f.apply("save-issue",json!({"id":"unrelated","stage":"implementation","kind":"defect","status":"open","description":"independent problem"})).unwrap();
    assert!(next(&f, "unrelated-event", false).is_none());
    assert_eq!(f.record().workbench.execution.units[0].status, "deferred");
}
#[test]
fn newly_approved_scope_does_not_replay_old_work_units() {
    let f = autonomous();
    unit(&f, "old", json!([]), json!(["old route"]));
    next(&f, "old-start", false);
    report(&f, "old", "deferred");
    finish_gaps(&f, "old-end");
    let text = "Updated approved implementation plan";
    // This fixture uses the real human decision endpoint to accept the replacement plan.
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.current_stage = "plan".into();
            r.stages.insert("plan".into(), "in-progress".into());
            Ok(())
        })
        .unwrap();
    f.doc("plan", text);
    f.accept("plan");
    let selected = next(&f, "new-scope", false).unwrap();
    assert_ne!(selected.id, "old");
    assert!(selected.id.ends_with("bootstrap"));
    assert_ne!(
        selected.scope_hash,
        f.record().workbench.execution.units[0].scope_hash
    );
}
#[test]
fn one_validation_run_derives_checks_atomically_without_duplicate_results() {
    let f = autonomous();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy["iterationMode"] = json!("human-feedback");
            Ok(())
        })
        .unwrap();
    f.evidence("proof", "json", "state");
    let check = json!({"id":"lua","status":"passed","observations":"controlled test evidence","evidenceIds":["proof"]});
    let bad = json!({"id":"bad-run","version":"v1","checks":[check.clone(),{"id":"other","status":"invalid","observations":"bad"}],"artChecks":[]});
    let count = f.record().self_tests.len();
    assert!(f.apply("record-validation-run", bad).is_err());
    assert_eq!(f.record().self_tests.len(), count);
    let run = json!({"id":"valid-run","version":"v1","checks":[check],"artChecks":[]});
    f.apply("record-validation-run", run.clone()).unwrap();
    f.apply("record-validation-run", run).unwrap();
    assert_eq!(f.record().self_tests.len(), count + 1);
    assert_eq!(f.record().self_tests.last().unwrap()["runId"], "valid-run");
}
#[test]
fn convergence_never_invents_a_playable_or_delivery_pass() {
    let f = autonomous();
    assert!(
        f.apply("conclude-implementation", json!({"summary":"nothing done"}))
            .is_err()
    );
    unit(&f, "limited", json!([]), json!(["bounded approach"]));
    next(&f, "start", false);
    f.apply(
        "conclude-implementation",
        json!({"summary":"Only partial files exist; missing playable evidence"}),
    )
    .unwrap();
    let r = f.record();
    assert_eq!(r.current_stage, "implementation");
    assert_eq!(r.stages["implementation"], "in-progress");
    assert_eq!(r.workbench.execution.state, "settledWithGaps");
    assert!(next(&f, "repeat", false).is_none());
    assert!(
        f.service
            .production_execution_wakeable(&f.id)
            .unwrap()
            .is_none()
    );
    assert!(validation::document_gate(&r, &f.workspace, "delivery").is_err());
}
#[test]
fn verified_equivalent_workaround_keeps_platform_issue_open_without_blocking_implementation() {
    let f = autonomous();
    let issue = json!({"id":"pb-covered","kind":"blocker","status":"open","stage":"implementation","description":"native effect unavailable","repairTarget":"abya-source","resumeWhen":"upstream build"});
    f.apply("save-issue", issue.clone()).unwrap();
    f.evidence("equivalent-proof", "json", "state");
    assert!(validation::clear_issues(&f.record(), "implementation").is_err());
    let mut handling = json!({"issueId":"pb-covered","mode":"workaround","scope":"candidate","reason":"alternate implementation","impact":"same requirements","affectedUnitIds":[],"evidenceIds":["equivalent-proof"]});
    f.apply("record-issue-handling", handling.clone()).unwrap();
    assert!(validation::clear_issues(&f.record(), "implementation").is_err());
    handling["scope"] = json!("equivalent");
    f.apply("record-issue-handling", handling).unwrap();
    let r = f.record();
    assert_eq!(r.issues[0]["status"], "open");
    validation::clear_issues(&r, "implementation").unwrap();
    autonomy::verify_workarounds(&r, &f.workspace, "implementation").unwrap();
    let mut changed = issue;
    changed["description"] = json!("different platform failure");
    f.apply("save-issue", changed).unwrap();
    assert!(validation::clear_issues(&f.record(), "implementation").is_err());
    assert!(
        f.apply(
            "save-issue",
            json!({"id":"forge","executionHandling":{"mode":"workaround"}})
        )
        .is_err()
    );
    let evidence = &r
        .evidence
        .iter()
        .find(|e| e.id == "equivalent-proof")
        .unwrap();
    std::fs::write(f.workspace.join(&evidence.path), "changed").unwrap();
    assert!(autonomy::verify_workarounds(&r, &f.workspace, "implementation").is_err());
}
#[test]
fn a_new_user_goal_after_convergence_gets_its_own_queue_without_replaying_old_work() {
    let f = autonomous();
    unit(&f, "old-deferred", json!([]), json!(["old route"]));
    next(&f, "old-start", false);
    report(&f, "old-deferred", "deferred");
    finish_gaps(&f, "old-end");
    let id = uuid::Uuid::new_v4().to_string();
    let view = f
        .service
        .production_workbench(ProductionMutation {
            task_id: f.id.clone(),
            expected_revision: f.record().revision,
            operation: "request".into(),
            data: json!({"id":id,"text":"Add an independent diagnostic panel"}),
        })
        .unwrap();
    let goal = view
        .record
        .unwrap()
        .workbench
        .goals
        .into_iter()
        .find(|g| g.source_id == id && g.source_type == "request")
        .unwrap();
    f.service
        .production_prepare_goal(&f.id, Some(&goal.id))
        .unwrap();
    let count = f.record().workbench.execution.units.len();
    f.service
        .production_prepare_goal(&f.id, Some(&goal.id))
        .unwrap();
    assert_eq!(f.record().workbench.execution.units.len(), count);
    let selected = next(&f, "new-request", false).unwrap();
    assert_eq!(selected.goal_id, goal.id);
    assert_eq!(f.record().workbench.execution.units[0].status, "deferred");
    assert_eq!(
        f.service.production_current(&f.id).unwrap()["requests"][0]["id"],
        id
    );
}
