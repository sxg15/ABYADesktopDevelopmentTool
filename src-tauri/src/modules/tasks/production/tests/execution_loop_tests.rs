use super::*;

fn setup() -> Fixture {
    let f = Fixture::new();
    f.plan();
    f.service.production_commit(&f.id,f.record().revision,|r|{
        r.policy["implementationExecution"]=json!({"mode":"autonomous","completionLoopVersion":1,"inPlaceRecoveryVersion":1,"reviewHandoffVersion":1,"workMinutes":60});
        r.policy["iterationMode"]=json!("human-feedback");
        r.policy["selfTestChecks"]=json!(["lua","input","core-loop","save-reload"]);
        Ok(())
    }).unwrap();
    f.evidence("proof", "json", "state");
    f
}
fn unit(f: &Fixture, id: &str) {
    f.apply("save-work-unit",json!({"id":id,"goalId":f.record().workbench.active_goal_id,"title":id,"routes":["first implementation","different verification method"],"dependsOn":[],"issueIds":[]})).unwrap();
}
fn next(f: &Fixture, event: &str, advance: bool) -> Option<autonomy::WorkUnit> {
    f.service
        .production_schedule(&f.id, event, advance)
        .unwrap()
}
fn report(f: &Fixture, id: &str, status: &str, code: &str) {
    f.apply("report-work-unit",json!({"id":id,"status":status,"deferReasonCode":code,
        "reason":"synthetic observed operation result","resultPaths":["artifacts/evidence/proof.json"],"nextCondition":"Read original receipt",
        "observation":{"kind":"verification-failed","origin":"unknown","expected":"Observed input","observed":"No matching observation"}})).unwrap();
}

#[test]
fn ordinary_failure_recovers_same_unit_without_repair_task_or_budget_reset() {
    let f = setup();
    unit(&f, "input");
    next(&f, "start", false);
    let approvals = serde_json::to_value(f.record().approvals).unwrap();
    report(&f, "input", "deferred", "repairable");
    let u = next(&f, "failure", true).unwrap();
    assert_eq!(u.id, "input");
    assert_eq!(u.recovery_rounds, 1);
    assert_eq!(u.route, 1);
    assert_eq!(f.record().workbench.execution.units.len(), 1);
    assert_eq!(
        serde_json::to_value(f.record().approvals).unwrap(),
        approvals
    );
    report(&f, "input", "succeeded", "");
    assert_eq!(f.record().workbench.execution.units[0].status, "succeeded");
}

#[test]
fn checkpoint_does_not_invent_functional_failure() {
    let f = setup();
    unit(&f, "input");
    next(&f, "start", false);
    let u = next(&f, "turn-end", true).unwrap();
    assert_eq!(u.id, "input");
    assert_eq!(u.recovery_rounds, 0);
    assert_eq!(u.stop_reason, "turn-checkpoint");
    assert!(
        !f.record()
            .workbench
            .execution
            .events
            .iter()
            .any(|e| e["kind"] == "failed")
    );
}

#[test]
fn missing_reports_or_provider_failures_cannot_spin_until_time_budget() {
    let f = setup();
    unit(&f, "input");
    next(&f, "start", false);
    assert_eq!(next(&f, "no-report-one", true).unwrap().id, "input");
    assert_eq!(next(&f, "no-report-two", true).unwrap().id, "input");
    assert_eq!(
        next(&f, "no-report-three", true).unwrap().kind,
        "finalization"
    );
    assert_eq!(
        f.record().workbench.execution.technical_queue["input"]["reason"],
        "missing-result-report"
    );
}

#[test]
fn unproven_external_label_does_not_stop_recovery_but_unknown_writes_do() {
    let f = setup();
    unit(&f, "input");
    unit(&f, "independent");
    next(&f, "start", false);
    report(&f, "input", "deferred", "external-condition");
    assert_eq!(next(&f, "retry", true).unwrap().id, "input");
    report(&f, "input", "awaiting-condition", "unknown-result");
    assert_eq!(next(&f, "unknown", true).unwrap().id, "independent");
    let r = f.record();
    assert_eq!(r.workbench.execution.units[0].status, "awaiting-condition");
    assert_eq!(
        r.workbench.execution.technical_queue["input"]["reason"],
        "reconcile-write"
    );
}

#[test]
fn self_reported_permission_or_phantom_dependency_does_not_create_a_stop_gate() {
    let f = setup();
    unit(&f, "input");
    next(&f, "start", false);
    f.apply("report-work-unit",json!({"id":"input","status":"deferred","deferReasonCode":"external-condition","reason":"Unconfirmed permission hypothesis","resultPaths":["artifacts/evidence/proof.json"],"nextCondition":"Inspect the real tool result","observation":{"kind":"verification-unavailable","origin":"permission","expected":"Tool result","observed":"No permission request is known"}})).unwrap();
    assert_eq!(next(&f, "permission-hypothesis", true).unwrap().id, "input");
    report(&f, "input", "deferred", "dependency");
    assert_eq!(next(&f, "phantom-dependency", true).unwrap().id, "input");
    assert!(f.record().workbench.execution.technical_queue.is_empty());
}

#[test]
fn repeated_failure_is_bounded_and_retains_internal_technical_record() {
    let f = setup();
    unit(&f, "input");
    next(&f, "start", false);
    for n in 0..3 {
        report(&f, "input", "failed", "");
        assert_eq!(next(&f, &format!("retry-{n}"), true).unwrap().id, "input");
    }
    report(&f, "input", "failed", "");
    assert_eq!(
        f.record().workbench.execution.technical_queue["input"]["reason"],
        "no-progress"
    );
    let u = next(&f, "settle", true).unwrap();
    assert_eq!(u.kind, "finalization");
    assert!(
        !f.record()
            .workbench
            .execution
            .units
            .iter()
            .any(|u| matches!(u.kind.as_str(), "repair-plan" | "handoff-repair"))
    );
}

#[test]
fn optional_check_failure_does_not_expand_gate_and_required_quote_must_exist() {
    let f = setup();
    for id in ["lua", "input", "core-loop", "save-reload"] {
        f.apply("save-self-test",json!({"id":id,"status":"passed","observations":"synthetic proof","evidenceIds":["proof"]})).unwrap();
    }
    f.apply(
        "register-acceptance-check",
        json!({"id":"polish","basis":"optional","required":false,"expected":"Optional polish"}),
    )
    .unwrap();
    f.apply(
        "save-self-test",
        json!({"id":"polish","status":"failed","observations":"Optional visual preference"}),
    )
    .unwrap();
    assert!(review_handoff::deficits(&f.record()).is_empty());
    assert!(f.apply("register-acceptance-check",json!({"id":"invented","basis":"approved-requirement","required":true,"expected":"New scope","documentKind":"requirements","quote":"Not in approved text"})).is_err());
    f.apply("register-acceptance-check",json!({"id":"full-cycle","basis":"approved-requirement","required":true,"expected":"Complete cycle","documentKind":"requirements","quote":"A complete test cycle."})).unwrap();
    assert!(review_handoff::deficits(&f.record()).contains(&"check:full-cycle".into()));
    assert!(iteration::ready(&f.record(), &f.workspace).is_err());
}

#[test]
fn package_preview_preserves_current_failed_candidate_and_approvals() {
    let f = setup();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.player_mode = "single".into();
            Ok(())
        })
        .unwrap();
    for id in ["lua", "input", "core-loop", "save-reload"] {
        f.apply("save-self-test",json!({"id":id,"status":"passed","observations":"synthetic proof","evidenceIds":["proof"]})).unwrap();
    }
    f.service
        .production_build_package(&f.id, f.record().revision)
        .unwrap();
    let before = f.record();
    f.apply(
        "save-self-test",
        json!({"id":"input","status":"failed","observations":"new current-version counterexample"}),
    )
    .unwrap();
    assert!(f.service.preview_spec(&f.id).is_err());
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.current_version = Some("unfinished-v2".into());
            Ok(())
        })
        .unwrap();
    let spec = f.service.preview_spec(&f.id).unwrap();
    assert_eq!(spec.version, "v1");
    let copy = f.service.preview_archive(&f.id, "v1").unwrap();
    assert_ne!(copy, spec.archive_path);
    assert!(Path::new(&copy).join("Main.PBArc").is_file());
    assert_eq!(f.record().current_version.as_deref(), Some("unfinished-v2"));
    assert_eq!(
        serde_json::to_value(f.record().approvals).unwrap(),
        serde_json::to_value(before.approvals).unwrap()
    );
    std::fs::write(
        f.workspace.join("artifacts/evidence/proof.json"),
        "changed evidence",
    )
    .unwrap();
    assert!(f.service.preview_spec(&f.id).is_err());
}

#[test]
fn source_diagnosis_requires_excluding_call_state_data_and_test_errors() {
    let f = setup();
    let data = json!({"diagnosisStatus":"confirmed","repairTarget":"abya-source","diagnosis":{}});
    assert!(acceptance_contract::validate_platform(&f.record(), &data).is_err());
    let mut old = f.record();
    old.policy["implementationExecution"]["inPlaceRecoveryVersion"] = Value::Null;
    assert!(acceptance_contract::validate_platform(&old, &data).is_ok());
}

#[test]
fn disproven_platform_alarm_can_close_with_evidence_without_a_code_change() {
    let f = setup();
    f.apply("save-issue",json!({"id":"false-alarm","stage":"implementation","kind":"defect","status":"open","repairTarget":"abya-source","diagnosisStatus":"suspected","description":"Synthetic suspected platform issue"})).unwrap();
    f.apply("save-issue",json!({"id":"false-alarm","stage":"implementation","kind":"defect","status":"resolved","repairTarget":"abya-source","diagnosisStatus":"excluded","description":"Synthetic suspected platform issue","diagnosis":{"exclusionReason":"Same build works with the correct tool arguments"},"fix":"Corrected invocation; no gameplay code change","recheck":"Observed correct current-version result","evidenceIds":["proof"]})).unwrap();
    assert_eq!(f.record().issues[0]["diagnosisStatus"], "excluded");
    assert_eq!(f.record().issues[0]["status"], "resolved");
    assert!(f.apply("save-self-test",json!({"id":"input","status":"failed","outcome":"functional-failure","observations":"No actual evidence supplied"})).is_err());
}

#[test]
fn explicit_migration_absorbs_legacy_plans_without_resuming_or_resetting_time() {
    let f = setup();
    unit(&f, "input");
    let mut r = f.record();
    r.workbench.execution.units[0].status = "deferred".into();
    r.workbench.execution.units[0].defer_reason_code = "repairable".into();
    r.workbench.execution.units[0].elapsed_ms = 12345;
    let mut legacy = r.workbench.execution.units[0].clone();
    legacy.id = "old-repair".into();
    legacy.kind = "repair-plan".into();
    legacy.repair_for = "input".into();
    r.workbench.execution.units.push(legacy);
    execution_loop::migrate(&mut r);
    assert_eq!(r.workbench.execution.state, "paused");
    assert_eq!(r.workbench.execution.units[0].elapsed_ms, 12345);
    assert_eq!(r.workbench.execution.units[1].status, "superseded");
    assert!(!completion::relevant(&r).contains("old-repair"));
}

#[test]
fn goal_budget_is_shared_but_preserves_a_bounded_finalization_window() {
    let f = setup();
    unit(&f, "first");
    unit(&f, "second");
    let mut r = f.record();
    r.workbench.execution.units[0].elapsed_ms = 180 * 60_000;
    assert!(autonomy_budget::exhausted(
        &r,
        &r.workbench.execution.units[1]
    ));
    let mut finalizer = r.workbench.execution.units[1].clone();
    finalizer.kind = "finalization".into();
    assert!(!execution_loop::budget_exhausted(&r, &finalizer, 0));
    finalizer.elapsed_ms = 15 * 60_000;
    assert!(execution_loop::budget_exhausted(&r, &finalizer, 0));
}

#[test]
fn other_stages_have_bounded_automatic_continuations_without_fake_success() {
    let f = setup();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.current_stage = "resources".into();
            r.stages.insert("resources".into(), "in-progress".into());
            Ok(())
        })
        .unwrap();
    assert!(
        f.service
            .production_stage_checkpoint(&f.id, "one", "material-a")
            .unwrap()
    );
    assert!(
        f.service
            .production_stage_checkpoint(&f.id, "two", "material-a")
            .unwrap()
    );
    assert!(
        !f.service
            .production_stage_checkpoint(&f.id, "three", "material-a")
            .unwrap()
    );
    assert_eq!(f.record().workbench.execution.state, "technicalReview");
    assert_eq!(f.record().stages["resources"], "in-progress");
    assert!(
        f.service
            .production_stage_checkpoint(&f.id, "four", "real-new-artifact")
            .unwrap()
    );
    assert_eq!(f.record().workbench.execution.state, "ready");
}

#[test]
fn optional_failed_validation_does_not_turn_completed_work_into_repair() {
    let f = setup();
    unit(&f, "input");
    next(&f, "start", false);
    f.apply("register-acceptance-check",json!({"id":"optional-glow","basis":"optional","required":false,"expected":"Optional glow"})).unwrap();
    f.apply("record-validation-run",json!({"id":"real-fixture-run","version":"v1","purpose":"candidate","checks":[{"id":"optional-glow","status":"failed","observations":"Optional preference only"}],"artChecks":[]})).unwrap();
    report(&f, "input", "succeeded", "");
    assert_eq!(f.record().workbench.execution.units[0].status, "succeeded");
    assert!(
        f.record()
            .self_tests
            .iter()
            .any(|t| t["id"] == "optional-glow" && t["status"] == "failed")
    );
}

#[test]
fn direct_recheck_clears_prior_failed_run_without_an_extra_repair_cycle() {
    let f = setup();
    unit(&f, "input");
    next(&f, "start", false);
    f.apply("record-validation-run",json!({"id":"failed-input-run","version":"v1","purpose":"candidate","checks":[{"id":"input","status":"failed","observations":"Synthetic failed input"}],"artChecks":[]})).unwrap();
    f.apply("save-self-test",json!({"id":"input","status":"passed","observations":"Synthetic successful recheck","evidenceIds":["proof"]})).unwrap();
    report(&f, "input", "succeeded", "");
    assert_eq!(f.record().workbench.execution.units[0].status, "succeeded");
    assert!(
        f.record().workbench.execution.units[0]
            .validation_failures
            .is_empty()
    );
}

#[test]
fn scoped_evidence_rejects_dependency_build_and_unknown_changes() {
    let f = setup();
    let mut r = f.record();
    r.version_details["validationImpact"] = json!({"unknownImpact":false});
    r.version_details["contentManifest"] = json!({"files":[{"path":"rule.lua","sha256":"rule-a"}]});
    r.workbench.execution.acceptance_contract.insert(
        "score".into(),
        json!({"scopeHash":autonomy::scope(&r),"dependencies":["rule.lua"]}),
    );
    scoped_evidence::bind(&mut r, &json!({"id":"score-proof","checkId":"score"})).unwrap();
    let request = json!({"sourceId":"score-proof"});
    assert!(scoped_evidence::verify_reuse(&r, &request).is_ok());
    let mut changed = r.clone();
    changed.version_details["contentManifest"]["files"][0]["sha256"] = json!("rule-b");
    assert!(scoped_evidence::verify_reuse(&changed, &request).is_err());
    changed = r.clone();
    changed.version_details["validationImpact"] = json!({"unknownImpact":true});
    assert!(scoped_evidence::verify_reuse(&changed, &request).is_err());
    changed = r.clone();
    changed.version_details["playerBuildHash"] = json!("wrong-build");
    assert!(scoped_evidence::verify_reuse(&changed, &request).is_err());
}

#[test]
fn player_pin_and_authoring_data_are_task_owned() {
    let f = setup();
    let data = f
        .service
        .production_authoring_data_root(&f.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        data,
        f.service
            .production_authoring_data_root(&f.id)
            .unwrap()
            .unwrap()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(data.join("owner.json")).unwrap()).unwrap()
            ["taskId"],
        f.id
    );
    let r = f.record();
    let player = Path::new(r.version_details["playerSource"].as_str().unwrap());
    f.service
        .production_check_player_pin(&f.id, player)
        .unwrap();
    let foreign = f.root.join("foreign-player.exe");
    std::fs::copy(player, &foreign).unwrap();
    assert!(
        f.service
            .production_check_player_pin(&f.id, &foreign)
            .is_err()
    );
    std::fs::write(data.join("owner.json"), br#"{"taskId":"foreign"}"#).unwrap();
    assert!(f.service.production_authoring_data_root(&f.id).is_err());
}

#[test]
fn baseline_round_trip_supports_full_catalog_but_returns_bounded_context() {
    let f = setup();
    let r = f.record();
    let player = Path::new(r.version_details["playerSource"].as_str().unwrap());
    let hash = r.version_details["playerBuildHash"].as_str().unwrap();
    let value = json!({"schema":"test-only","playerBuildHash":hash,"observedCapabilities":[{"id":"input.focus","description":"x".repeat(1100000)}],"reports":[]});
    let saved = f.service.save_runtime_baseline(&f.id, hash, value).unwrap();
    assert!(saved.to_string().len() < 2000);
    let read = f
        .service
        .runtime_baseline_for_player(&f.id, player, "focus")
        .unwrap();
    assert_eq!(read["capabilityCount"], 1);
    assert_eq!(read["matchingCapabilities"][0]["id"], "input.focus");
    assert!(read.to_string().len() < 2500);
}
