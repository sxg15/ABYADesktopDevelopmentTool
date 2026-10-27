use super::*;
fn setup() -> Fixture {
    let f = Fixture::new();
    f.plan();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy["implementationExecution"] =
                json!({"mode":"autonomous","completionLoopVersion":1,"workMinutes":60});
            r.policy["iterationMode"] = json!("human-feedback");
            r.policy["selfTestChecks"] = json!(["lua", "input", "core-loop", "save-reload"]);
            Ok(())
        })
        .unwrap();
    f
}
fn unit(f: &Fixture, id: &str, deps: Value) {
    let goal = f.record().workbench.active_goal_id.unwrap();
    f.apply("save-work-unit",json!({"id":id,"goalId":goal,"title":id,"dependsOn":deps,"issueIds":[],"routes":["actual first route"]})).unwrap();
}
fn next(f: &Fixture, id: &str, advance: bool) -> Option<autonomy::WorkUnit> {
    f.service.production_schedule(&f.id, id, advance).unwrap()
}
fn report(f: &Fixture, id: &str, status: &str) {
    files::write_file(
        &f.workspace,
        "artifacts/probe.json",
        b"synthetic probe evidence",
    )
    .unwrap();
    f.apply("report-work-unit",json!({"id":id,"status":status,"reason":"synthetic operation result","deferReasonCode":"repairable","resultPaths":["artifacts/probe.json"]})).unwrap();
}
#[test]
fn failure_automatically_dispatches_repair_then_original_unit_without_losing_budget() {
    let f = setup();
    unit(&f, "input", json!([]));
    unit(&f, "deploy", json!(["input"]));
    assert_eq!(next(&f, "start", false).unwrap().id, "input");
    report(&f, "input", "deferred");
    let repair = next(&f, "failed-input", true).unwrap();
    assert_eq!(repair.kind, "repair-plan");
    assert_eq!(repair.repair_for, "input");
    assert!(f.apply("report-work-unit",json!({"id":repair.id,"status":"succeeded","reason":"Only a summary","resultPaths":["artifacts/probe.json"]})).is_err());
    let before = f.record().workbench.execution.units[0].elapsed_ms;
    f.apply("revise-work-unit",json!({"id":"input","expectedDefinitionRevision":0,"reason":"Use verified event bridge","routes":["different tested event owner route"],"dependsOn":[],"resultPaths":["artifacts/probe.json"]})).unwrap();
    report(&f, &repair.id, "succeeded");
    let rerun = next(&f, "repair-complete", true).unwrap();
    assert_eq!(rerun.id, "input");
    assert_eq!(rerun.definition_revision, 1);
    assert!(rerun.elapsed_ms >= before);
    report(&f, "input", "succeeded");
    assert_eq!(next(&f, "input-fixed", true).unwrap().id, "deploy");
}
#[test]
fn dependency_result_and_signal_wake_share_one_condition_without_summary_churn() {
    let f = setup();
    unit(&f, "dependency", json!([]));
    unit(&f, "dependent", json!(["dependency"]));
    let r = f.record();
    let before = autonomy::condition(&r);
    let mut after = r.clone();
    after.workbench.execution.units[0].result_hash = "changed-real-result".into();
    assert_ne!(before, autonomy::condition(&after));
    after = r.clone();
    after.workbench.execution.units[0].reason = "wording only".into();
    assert_eq!(before, autonomy::condition(&after));
    after.self_tests.push(json!({"id":"playable-input","version":"v1","cycle":1,"status":"passed","evidenceIds":["new-proof"]}));
    assert_ne!(before, autonomy::condition(&after));
    files::write_file(
        &f.workspace,
        "artifacts/signal.json",
        b"new observed capability",
    )
    .unwrap();
    f.apply(
        "record-work-signal",
        json!({"unitId":"dependent","reason":"Readback changed","paths":["artifacts/signal.json"]}),
    )
    .unwrap();
    assert_ne!(before, autonomy::condition(&f.record()));
}
#[test]
fn acceptance_copy_has_owned_discoverable_data_root() {
    let f = setup();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.version_details["frozenSnapshot"] = json!(true);
            Ok(())
        })
        .unwrap();
    let copy = f
        .service
        .production_acceptance_archive(&f.id, "v1")
        .unwrap();
    let root = f
        .service
        .production_acceptance_data_root(&f.id, &copy)
        .unwrap()
        .unwrap();
    assert!(
        Path::new(&copy)
            .canonicalize()
            .unwrap()
            .starts_with(root.join("Archives"))
    );
    assert!(
        f.service
            .production_acceptance_data_root(&f.id, f.workspace.to_str().unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        versions::digest(&[versions::archive_root(Path::new(&copy)).unwrap()]).unwrap(),
        f.record().version_details["archiveHash"]
    );
}
#[test]
#[ignore = "Real pinned Player with a copied archive; set ABYA_COMPLETION_SOURCE_RECEIPT and ABYA_COMPLETION_OUTPUT"]
fn real_legacy_player_acceptance_loads_frozen_copy_in_owned_data_root() {
    use crate::modules::{
        game_connections::GameConnectionService, instances::InstanceService, logs::LogService,
        runtime_bridge::RuntimeBridgeService,
    };
    let receipt: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("ABYA_COMPLETION_SOURCE_RECEIPT").unwrap()).unwrap(),
    )
    .unwrap();
    let source = &receipt["data"]["record"]["versionDetails"];
    let out = PathBuf::from(std::env::var("ABYA_COMPLETION_OUTPUT").unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let f = setup();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.player_mode = "multiplayer".into();
            Ok(())
        })
        .unwrap();
    let copied = version_snapshots::copy_verified(
        Path::new(source["archiveSource"].as_str().unwrap()),
        &f.workspace.join("source-copy"),
    )
    .unwrap();
    f.service
        .production_capture_version_for_purpose(
            &f.id,
            f.record().revision,
            "actual-v5-copy",
            VersionSources {
                archive_path: copied,
                player_path: PathBuf::from(source["playerSource"].as_str().unwrap()),
                archive_guid: source["archiveGuid"].as_str().unwrap().into(),
                level_guid: source["levelGuid"].as_str().unwrap().into(),
                instance_id: "synthetic-registration-fixture".into(),
            },
            "candidate",
        )
        .unwrap();
    let paths = AppPaths {
        data_dir: f.root.clone(),
        database_path: f.root.join("app.db"),
        bootstrap_logs_dir: f.root.join("logs"),
        reports_dir: f.root.join("reports"),
        archive_transfers_dir: f.root.join("transfers"),
        default_archive_root: f.root.join("archives"),
        default_workspace_root: f.root.join("workspaces"),
    };
    std::fs::create_dir_all(&paths.bootstrap_logs_dir).unwrap();
    std::fs::create_dir_all(&paths.reports_dir).unwrap();
    let connections = GameConnectionService::new();
    let instances = InstanceService::new(
        f.service.database.clone(),
        paths.clone(),
        connections.clone(),
    );
    let logs = LogService::new(f.service.database.clone(), connections.clone());
    connections.set_handler(std::sync::Arc::new(crate::GameConnectionCoordinator {
        instances: instances.clone(),
        logs,
    }));
    let mut settings = crate::foundation::SettingsService::new(
        f.service.database.clone(),
        paths.default_workspace_root.clone(),
    )
    .get()
    .unwrap();
    let socket = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    settings.game_gateway_port = socket.local_addr().unwrap().port();
    drop(socket);
    settings.lan_broadcast_enabled = false;
    connections.start(&settings).unwrap();
    let bridge = RuntimeBridgeService::new(instances.clone(), f.service.clone());
    let result = bridge.start_acceptance(&f.id);
    std::fs::write(
        out.join("acceptance.json"),
        serde_json::to_vec_pretty(&result.as_ref().map_err(|e| e.to_string())).unwrap(),
    )
    .unwrap();
    let owned = instances.list(Some(&f.id)).unwrap();
    for item in &owned {
        let report = instances.read_launch_report(&item.id).unwrap();
        std::fs::write(
            out.join(format!("{}.json", item.id)),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        let _ = instances.stop(&item.id);
    }
    let session = result.unwrap();
    assert_eq!(session["status"], "ready");
    assert_eq!(session["instanceIds"].as_array().unwrap().len(), 2);
    assert!(
        session["launchArchiveCopy"]
            .as_str()
            .unwrap()
            .replace('\\', "/")
            .contains("/.abya-play-runs/")
    );
    println!(
        "Real pinned Player accepted owned frozen copies for Host and Client; synthetic task approvals only."
    );
}
#[test]
fn no_candidate_cannot_be_reported_as_successful_lua_completion() {
    let f = setup();
    unit(&f, "core", json!([]));
    next(&f, "start", false);
    report(&f, "core", "succeeded");
    let finalizer = next(&f, "core-done", true).unwrap();
    assert_eq!(finalizer.kind, "finalization");
    assert!(f.apply("report-work-unit",json!({"id":finalizer.id,"status":"succeeded","reason":"No actual runtime proof","resultPaths":["artifacts/probe.json"]})).is_err());
    f.apply(
        "conclude-implementation",
        json!({"summary":"Core package could not be verified; retain repair action"}),
    )
    .unwrap();
    assert_eq!(f.record().current_stage, "implementation");
    assert_eq!(f.record().workbench.execution.state, "repairRequired");
    assert!(next(&f, "unchanged", false).is_none());
}
#[test]
fn verified_package_is_bound_to_candidate_and_contains_actual_archive() {
    let f = setup();
    f.evidence("core-proof", "json", "state");
    for id in ["lua", "input", "core-loop", "save-reload"] {
        f.apply("save-self-test",json!({"id":id,"status":"passed","observations":"synthetic contract fixture only","evidenceIds":["core-proof"]})).unwrap();
    }
    f.apply(
        "save-self-test",
        json!({"id":"input","status":"failed","observations":"OS focus remains unverified"}),
    )
    .unwrap();
    assert!(
        f.service
            .production_build_package(&f.id, f.record().revision)
            .is_err()
    );
    f.apply("save-self-test",json!({"id":"playable-input","status":"passed","observations":"synthetic primary input proof; not full focus acceptance","evidenceIds":["core-proof"]})).unwrap();
    let view = f
        .service
        .production_build_package(&f.id, f.record().revision)
        .unwrap();
    let r = view.record.unwrap();
    assert!(verification_package::valid(&r, &f.workspace));
    assert!(
        iteration::ready(&r, &f.workspace).is_err(),
        "a playable preview cannot waive full input acceptance"
    );
    let path = files::safe_path(
        &f.workspace,
        r.workbench.execution.verification_package["path"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    assert!(archive.by_name("archive/Main.PBArc").is_ok());
    assert!(archive.by_name("verification.json").is_ok());
    drop(archive);
    std::fs::write(&path, b"corrupt package").unwrap();
    assert!(!verification_package::valid(&r, &f.workspace));
}
#[test]
fn fixture_failed_checks_are_recorded_without_mutating_candidate_self_tests() {
    let f = setup();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            let mut v = r.version_details.clone();
            v["scopeHash"] = json!(autonomy::scope(r));
            v["cycle"] = json!(r.cycle);
            v["purpose"] = json!("fixture");
            r.workbench
                .execution
                .validation_snapshots
                .insert("fixture".into(), v);
            Ok(())
        })
        .unwrap();
    let before = f.record();
    f.apply("record-validation-run",json!({"id":"fixture-failure","version":"fixture","purpose":"fixture","checks":[{"id":"input","status":"failed","observations":"real input did not change state"}],"artChecks":[]})).unwrap();
    let after = f.record();
    assert_eq!(after.current_version, before.current_version);
    assert_eq!(after.self_tests, before.self_tests);
    assert_eq!(
        after.workbench.execution.validation_runs["fixture-failure"]["purpose"],
        "fixture"
    );
}
#[test]
fn reconciliation_requires_raw_readback_and_a_different_remaining_route() {
    let f = setup();
    unit(&f, "uncertain", json!([]));
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            let u = &mut r.workbench.execution.units[0];
            u.status = "awaiting-condition".into();
            u.defer_reason_code = "unknown-result".into();
            u.elapsed_ms = 9000;
            Ok(())
        })
        .unwrap();
    files::write_file(
        &f.workspace,
        "artifacts/readback.json",
        br#"{"requestId":"original-readback","success":true,"data":{"saved":true}}"#,
    )
    .unwrap();
    let data = json!({"id":"uncertain","expectedDefinitionRevision":0,"reason":"Original effect read back; execute only remaining validation","resultPaths":["artifacts/readback.json"],"routes":["readback and remaining validation; no replay"],"dependsOn":[],"reconciliation":{"operationIds":["original-write"],"outcome":"applied","readbackPaths":["artifacts/readback.json"]}});
    let mut bad = data.clone();
    bad["reconciliation"]["readbackPaths"] = json!([]);
    assert!(f.apply("reconcile-work-unit", bad).is_err());
    f.apply("reconcile-work-unit", data).unwrap();
    let r = f.record();
    let u = &r.workbench.execution.units[0];
    assert_eq!(u.id, "uncertain");
    assert_eq!(u.status, "pending");
    assert_eq!(u.elapsed_ms, 9000);
    assert_eq!(u.definition_revision, 1);
    assert!(u.result_paths.is_empty());
    assert_eq!(
        u.definition_history.last().unwrap()["reconciliation"]["outcome"],
        "applied"
    );
}
#[test]
fn captured_stalled_graphs_dispatch_current_goal_repairs_and_expose_all_work() {
    for raw in [
        include_str!("fixtures/memory-stalled.json"),
        include_str!("fixtures/needle-stalled.json"),
    ] {
        let f = setup();
        let source: Value = serde_json::from_str(raw).unwrap();
        f.service
            .production_commit(&f.id, f.record().revision, |r| {
                let goal = source["activeGoalId"].as_str().unwrap().to_owned();
                let mut template = r.workbench.goals.last().unwrap().clone();
                template.id = goal.clone();
                template.source_type = "request".into();
                template.stage = "implementation".into();
                template.status = "in-progress".into();
                template.source_id=goal.strip_prefix("request:").unwrap().into();
                r.workbench.requests.push(json!({"id":template.source_id,"stage":"implementation","status":"in-progress","text":"Synthetic recovery fixture","createdAt":now()}));
                r.workbench.goals.push(template);
                r.workbench.active_goal_id = Some(goal);
                r.workbench.execution.units = serde_json::from_value(source["units"].clone())?;
                let scope = autonomy::scope(r);
                for u in &mut r.workbench.execution.units {
                    u.scope_hash = scope.clone();
                    u.cycle = r.cycle;
                }
                r.workbench.execution.scope_hash = scope;
                r.workbench.execution.state = "settledWithGaps".into();
                r.workbench.execution.parked_condition = autonomy::condition(r);
                completion::migrate(r);
                Ok(())
            })
            .unwrap();
        assert!(
            f.service
                .production_execution_wakeable(&f.id)
                .unwrap()
                .is_none(),
            "migration must remain paused"
        );
        let summary = f.service.production_current(&f.id).unwrap();
        assert_eq!(
            summary["workUnits"].as_array().unwrap().len(),
            source["units"].as_array().unwrap().len()
        );
        assert_eq!(summary["workUnits"][0]["goalId"], source["activeGoalId"]);
        let work = next(&f, "explicit-resume-fixture", false).unwrap();
        assert_eq!(work.kind, "repair-plan");
        assert_eq!(work.goal_id, source["activeGoalId"]);
        assert!(!work.repair_for.is_empty());
        assert_eq!(
            f.record().workbench.active_goal_id.as_deref(),
            source["activeGoalId"].as_str()
        );
    }
}
#[test]
fn budget_migration_preserves_shared_history_and_unknown_units_are_not_replayed() {
    let f = setup();
    unit(&f, "unknown", json!([]));
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.workbench
                .execution
                .recovery_ms
                .insert("prior-fault".into(), 1429899);
            let u = &mut r.workbench.execution.units[0];
            u.status = "awaiting-condition".into();
            u.defer_reason_code = "unknown-result".into();
            u.elapsed_ms = 55000;
            autonomy_budget::migrate_budget(r);
            completion::migrate(r);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        f.record().workbench.execution.recovery_ms["prior-fault"],
        1429899
    );
    let next = next(&f, "review-unknown", false).unwrap();
    assert_eq!(next.kind, "repair-plan");
    assert_eq!(
        f.record().workbench.execution.units[0].status,
        "awaiting-condition"
    );
}
