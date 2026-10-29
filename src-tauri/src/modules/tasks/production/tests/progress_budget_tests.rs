use super::*;
fn record() -> ProductionRecord {
    let f = Fixture::new();
    let mut r = f.record();
    r.current_stage = "implementation".into();
    r.stages
        .insert("implementation".into(), "in-progress".into());
    r.policy["implementationExecution"] =
        json!({"mode":"autonomous","completionLoopVersion":1,"progressBudgetVersion":1});
    r.workbench.active_goal_id = Some("goal".into());
    r.workbench.execution.units.push(autonomy::WorkUnit {
        id: "work".into(),
        kind: "implementation".into(),
        status: "running".into(),
        cycle: r.cycle,
        goal_id: "goal".into(),
        scope_hash: autonomy::scope(&r),
        ..Default::default()
    });
    r.workbench.execution.active_unit_id = Some("work".into());
    progress_budget::initialize(&mut r, "work");
    r
}
#[test]
fn saved_content_can_extend_original_work_without_resetting_elapsed_time() {
    let mut r = record();
    r.workbench.execution.units[0].elapsed_ms = 60 * 60_000;
    r.version_details["contentManifest"] =
        json!({"files":[{"path":"Levels/game.PBLevel","sha256":"actual-new-content"}]});
    progress_budget::refresh(&mut r);
    let u = &r.workbench.execution.units[0];
    assert_eq!(u.elapsed_ms, 60 * 60_000);
    assert_eq!(u.budget_grants[0]["kind"], "progress");
    assert!(!autonomy_budget::exhausted(&r, u));
}
#[test]
fn prose_metadata_and_repeated_diagnostics_do_not_create_progress_credits() {
    let mut r = record();
    r.workbench.execution.units[0].elapsed_ms = 60 * 60_000;
    r.version_details["contentManifest"] =
        json!({"files":[{"path":"Index.PBConf","sha256":"new-time"}]});
    progress_budget::refresh(&mut r);
    assert_eq!(
        r.workbench.execution.units[0].budget_grants[0]["kind"],
        "settlement"
    );
    r.workbench.execution.units[0].elapsed_ms = 75 * 60_000;
    r.workbench.execution.units[0].reason = "new wording".into();
    progress_budget::refresh(&mut r);
    assert_eq!(r.workbench.execution.units[0].budget_grants.len(), 1);
    assert!(autonomy_budget::exhausted(
        &r,
        &r.workbench.execution.units[0]
    ));
}
#[test]
fn one_exhausted_issue_does_not_poison_an_independent_repair() {
    let mut r = record();
    let mut a = r.workbench.execution.units[0].clone();
    a.id = "focus".into();
    a.kind = "handoff-repair".into();
    a.issue_ids = vec!["focus-gap".into()];
    a.elapsed_ms = 90 * 60_000;
    let mut b = a.clone();
    b.id = "menu".into();
    b.issue_ids = vec!["menu-conflict".into()];
    b.elapsed_ms = 0;
    r.workbench.execution.units.extend([a, b]);
    assert!(!autonomy_budget::exhausted(
        &r,
        &r.workbench.execution.units[2]
    ));
    let mut duplicate = r.workbench.execution.units[1].clone();
    duplicate.id = "focus-new-id".into();
    duplicate.elapsed_ms = 0;
    r.workbench.execution.units.push(duplicate);
    assert!(autonomy_budget::exhausted(
        &r,
        r.workbench.execution.units.last().unwrap()
    ));
}
#[test]
fn real_fish_snapshot_gets_one_bounded_settlement_without_losing_work_or_time() {
    let mut r = record();
    let f: Value = serde_json::from_str(include_str!("fixtures/fish-budget-stalled.json")).unwrap();
    r.workbench.execution.units = serde_json::from_value(f["units"].clone()).unwrap();
    r.workbench.active_goal_id = Some(f["activeGoalId"].as_str().unwrap().into());
    let scope = autonomy::scope(&r);
    for u in &mut r.workbench.execution.units {
        u.scope_hash = scope.clone();
        u.cycle = r.cycle;
    }
    let before = r.workbench.execution.units.clone();
    progress_budget::migrate(&mut r);
    for id in [
        "fish-c1-05",
        "fish-c1-10",
        "fish-c1-11",
        "fish-c1-12",
        "fish-c1-14",
        "fish-c1-16",
    ] {
        let u = r
            .workbench
            .execution
            .units
            .iter()
            .find(|u| u.id == id)
            .unwrap();
        assert_eq!(u.status, "pending");
        assert!(!autonomy_budget::exhausted(&r, u));
        assert_eq!(
            u.elapsed_ms,
            before.iter().find(|u| u.id == id).unwrap().elapsed_ms
        );
        assert_eq!(u.budget_grants.len(), 1);
    }
    assert_eq!(r.workbench.execution.units.len(), before.len());
    progress_budget::migrate(&mut r);
    assert_eq!(
        r.workbench
            .execution
            .units
            .iter()
            .filter(|u| u.settlement_used)
            .count(),
        6
    );
    assert_eq!(r.workbench.execution.state, "paused");
}

#[test]
fn source_churn_is_bounded_and_does_not_approve_the_game() {
    let mut r = record();
    for n in 0..8 {
        r.workbench.execution.units[0].elapsed_ms = 60 * 60_000 + n * 20 * 60_000;
        r.version_details["contentManifest"] =
            json!({"files":[{"path":"game.PBLevel","sha256":format!("source-{n}")}]});
        progress_budget::refresh(&mut r);
    }
    let u = &r.workbench.execution.units[0];
    assert_eq!(
        u.budget_grants
            .iter()
            .filter(|g| g["kind"] == "progress")
            .count(),
        2
    );
    assert_eq!(
        u.budget_grants
            .iter()
            .filter(|g| g["kind"] == "settlement")
            .count(),
        1
    );
    assert!(autonomy_budget::exhausted(&r, u));
    assert_eq!(r.current_stage, "implementation");
}
#[test]
fn successful_owned_fixture_validation_is_progress_not_game_acceptance() {
    let mut r = record();
    r.workbench.execution.units[0].elapsed_ms = 60 * 60_000;
    let scope = autonomy::scope(&r);
    r.workbench
        .execution
        .validation_snapshots
        .insert("probe".into(), json!({"archiveHash":"fixture-content"}));
    r.workbench.execution.validation_runs.insert("run".into(),json!({"workUnitId":"work","cycle":r.cycle,"scopeHash":scope,"version":"probe","purpose":"fixture","checks":[{"id":"menu-input","status":"passed"}]}));
    progress_budget::refresh(&mut r);
    assert_eq!(
        r.workbench.execution.units[0].budget_grants[0]["kind"],
        "progress"
    );
    assert!(r.self_tests.is_empty());
    assert!(r.current_version.is_none());
}

#[test]
fn parked_original_work_can_resume_on_new_content_but_unknown_writes_cannot() {
    let mut r = record();
    let u = &mut r.workbench.execution.units[0];
    u.elapsed_ms = 80 * 60_000;
    u.status = "deferred".into();
    u.defer_reason_code = "budget-exhausted".into();
    u.settlement_used = true;
    r.version_details["contentManifest"] =
        json!({"files":[{"path":"game.PBLevel","sha256":"new-content"}]});
    // Model a previously earned settlement so the next evidence-backed slice has room.
    r.workbench.execution.units[0]
        .budget_grants
        .push(json!({"kind":"settlement","extraMs":15*60_000}));
    assert!(progress_budget::reassess(&mut r));
    assert_eq!(r.workbench.execution.units[0].status, "pending");
    let u = &mut r.workbench.execution.units[0];
    u.status = "awaiting-condition".into();
    u.defer_reason_code = "unknown-result".into();
    r.version_details["contentManifest"]["files"][0]["sha256"] = json!("later-content");
    assert!(!progress_budget::reassess(&mut r));
    assert_eq!(r.workbench.execution.units[0].status, "awaiting-condition");
}
