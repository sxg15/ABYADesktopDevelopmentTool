use super::*;
fn setup() -> Fixture {
    let f = Fixture::new();
    f.plan();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy["implementationExecution"] =
                json!({"mode":"autonomous","completionLoopVersion":1,"reviewHandoffVersion":1});
            r.policy["iterationMode"] = json!("human-feedback");
            r.policy["selfTestChecks"] = json!(["lua", "input", "core-loop", "save-reload"]);
            Ok(())
        })
        .unwrap();
    f.evidence("proof", "json", "state");
    for id in ["lua", "input", "core-loop", "save-reload"] {
        f.apply("save-self-test",json!({"id":id,"status":"passed","observations":"synthetic contract fixture","evidenceIds":["proof"]})).unwrap();
    }
    f
}
fn source(f: &Fixture) -> Value {
    json!({"id":"engine","stage":"implementation","kind":"defect","status":"open",
        "description":"Synthetic engine defect","repairTarget":"abya-source","diagnosisStatus":"confirmed",
        "diagnosis":{"reproduction":"Repeatable minimal case","comparison":"Same Lua works in corrected build",
            "rootCause":"Engine dispatch implementation","playerBuildHash":f.record().version_details["playerBuildHash"]},
        "evidenceIds":["proof"]})
}
#[test]
fn ordinary_or_suspected_defects_and_unrun_checks_get_auto_repair_not_review() {
    let f = setup();
    f.apply("save-issue",json!({"id":"lua","stage":"implementation","kind":"defect","status":"open","repairTarget":"lua","description":"Fix input handler"})).unwrap();
    f.apply(
        "save-self-test",
        json!({"id":"input","status":"failed","observations":"Needs repair"}),
    )
    .unwrap();
    let mut r = f.record();
    assert!(review_handoff::gate(&r, &f.workspace).is_err());
    review_handoff::ensure_repairs(&mut r);
    assert_eq!(r.workbench.execution.units.len(), 1);
    assert_eq!(r.workbench.execution.units[0].kind, "handoff-repair");
    r.workbench.execution.units[0].status = "deferred".into();
    review_handoff::ensure_repairs(&mut r);
    assert_eq!(
        r.workbench.execution.units.len(),
        1,
        "unchanged evidence cannot buy another repair budget"
    );
    r.issues[0]["repairTarget"] = json!("abya-source");
    r.issues[0]["diagnosisStatus"] = json!("suspected");
    assert!(review_handoff::gate(&r, &f.workspace).is_err());
    r.issues.clear();
    r.self_tests.retain(|t| t["id"] != "input");
    assert!(review_handoff::deficits(&r).contains(&"check:input".into()));
}
#[test]
fn only_current_source_evidence_with_explicit_check_mapping_can_cross() {
    let f = setup();
    f.apply("save-issue", source(&f)).unwrap();
    f.apply("save-self-test",json!({"id":"input","status":"blocked","observations":"Engine owns focus event","blockedByIssueIds":["engine"]})).unwrap();
    let r = f.record();
    assert!(review_handoff::gate(&r, &f.workspace).is_ok());
    let mut bad = r.clone();
    bad.issues[0]["repairTarget"] = json!("abya-build");
    assert!(review_handoff::gate(&bad, &f.workspace).is_err());
    bad = r.clone();
    bad.issues[0]["diagnosis"]["playerBuildHash"] = json!("old-build");
    assert!(review_handoff::gate(&bad, &f.workspace).is_err());
    bad = r.clone();
    bad.self_tests.last_mut().unwrap()["blockedByIssueIds"] = json!([]);
    assert!(review_handoff::gate(&bad, &f.workspace).is_err());
    let evidence = r.evidence.iter().find(|e| e.id == "proof").unwrap();
    files::write_file(&f.workspace, &evidence.path, b"changed evidence").unwrap();
    assert!(review_handoff::gate(&r, &f.workspace).is_err());
}
#[test]
fn successful_dependencies_do_not_excuse_unfinished_lua_work() {
    let f = setup();
    let mut r = f.record();
    let goal = r.workbench.active_goal_id.clone().unwrap();
    let scope = autonomy::scope(&r);
    for (id, status, deps) in [
        ("a", "succeeded", vec![]),
        ("b", "deferred", vec!["a".into()]),
    ] {
        r.workbench.execution.units.push(autonomy::WorkUnit {
            id: id.into(),
            status: status.into(),
            kind: "implementation".into(),
            goal_id: goal.clone(),
            scope_hash: scope.clone(),
            cycle: r.cycle,
            defer_reason_code: "dependency".into(),
            depends_on: deps,
            ..Default::default()
        });
    }
    assert!(review_handoff::deficits(&r).contains(&"work:b".into()));
    r.issues.push(source(&f));
    r.workbench.execution.units[0].status = "deferred".into();
    r.workbench.execution.units[0].issue_ids = vec!["engine".into()];
    assert!(review_handoff::gate(&r, &f.workspace).is_ok());
}

#[test]
fn blocked_pending_dependencies_do_not_hide_the_automatic_handoff_repair() {
    let f = setup();
    let mut r = f.record();
    let goal = r.workbench.active_goal_id.clone().unwrap();
    let scope = autonomy::scope(&r);
    for (id, status, deps) in [
        ("a", "deferred", vec![]),
        ("b", "pending", vec!["a".into()]),
    ] {
        r.workbench.execution.units.push(autonomy::WorkUnit {
            id: id.into(),
            status: status.into(),
            kind: "implementation".into(),
            goal_id: goal.clone(),
            scope_hash: scope.clone(),
            cycle: r.cycle,
            depends_on: deps,
            ..Default::default()
        });
    }
    review_handoff::ensure_repairs(&mut r);
    assert!(
        r.workbench
            .execution
            .units
            .iter()
            .any(|u| u.kind == "handoff-repair" && u.status == "pending")
    );
}
#[test]
fn real_package_does_not_allow_ordinary_defect_to_converge_into_review() {
    let f = setup();
    f.apply("save-issue",json!({"id":"lua","stage":"implementation","kind":"defect","status":"open","description":"Still broken","repairTarget":"lua"})).unwrap();
    f.service
        .production_build_package(&f.id, f.record().revision)
        .unwrap();
    let mut r = f.record();
    assert!(verification_package::valid(&r, &f.workspace));
    let scope = autonomy::scope(&r);
    let goal = r.workbench.active_goal_id.clone().unwrap();
    r.workbench.execution.units.push(autonomy::WorkUnit {
        id: "core".into(),
        kind: "implementation".into(),
        status: "succeeded".into(),
        goal_id: goal.clone(),
        cycle: r.cycle,
        scope_hash: scope.clone(),
        ..Default::default()
    });
    let sig = completion::signature(&r);
    r.workbench.execution.units.push(autonomy::WorkUnit {
        id: "final".into(),
        kind: "finalization".into(),
        status: "running".into(),
        goal_id: goal,
        cycle: r.cycle,
        scope_hash: scope,
        input_signature: sig,
        ..Default::default()
    });
    r.workbench.execution.active_unit_id = Some("final".into());
    autonomy::conclude(
        &mut r,
        &f.workspace,
        &json!({"summary":"Core is playable but ordinary defect remains"}),
    )
    .unwrap();
    assert_eq!(r.current_stage, "implementation");
    assert_eq!(r.workbench.execution.state, "repairRequired");
    assert!(!r.workbench.execution.verification_package.is_null());
}
#[test]
fn upgrade_returns_legacy_gaps_to_original_goal_without_losing_approvals() {
    let f = setup();
    let mut r = f.record();
    let approvals = serde_json::to_value(&r.approvals).unwrap();
    let goal = r.workbench.active_goal_id.clone();
    r.current_stage = "review".into();
    r.stages
        .insert("implementation".into(), "execution-converged".into());
    r.self_tests.retain(|t| t["id"] != "input");
    review_handoff::migrate(&mut r);
    assert_eq!(r.current_stage, "implementation");
    assert_eq!(r.workbench.active_goal_id, goal);
    assert_eq!(r.workbench.execution.state, "paused");
    assert_eq!(serde_json::to_value(&r.approvals).unwrap(), approvals);
    assert!(
        r.workbench
            .execution
            .units
            .iter()
            .any(|u| u.kind == "handoff-repair")
    );
}
#[test]
fn selected_source_export_contains_diagnosis_and_exact_issue_only() {
    use std::io::Read;
    let f = setup();
    f.apply("save-issue", source(&f)).unwrap();
    f.apply("save-issue",json!({"id":"other","stage":"implementation","kind":"defect","status":"open","description":"unrelated","repairTarget":"lua"})).unwrap();
    let before = f.record();
    let path = f
        .service
        .export_developer_package(&f.id, before.revision, &["engine".into()])
        .unwrap();
    let mut z = zip::ZipArchive::new(std::fs::File::open(f.workspace.join(path)).unwrap()).unwrap();
    let mut body = String::new();
    z.by_name("diagnosis.json")
        .unwrap()
        .read_to_string(&mut body)
        .unwrap();
    let d: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(d["issues"].as_array().unwrap().len(), 1);
    assert_eq!(d["issues"][0]["id"], "engine");
    assert_eq!(
        d["playerBuildHash"],
        before.version_details["playerBuildHash"]
    );
    assert_eq!(f.record().revision, before.revision);
}
