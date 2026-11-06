use super::*;

#[test]
fn unlimited_execution_survives_all_old_deadlines_and_preserves_accounting() {
    let f = Fixture::new();
    f.plan();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy["executionTimePolicy"] = json!("unlimited");
            r.policy["implementationExecution"] = json!({"mode":"autonomous",
            "inPlaceRecoveryVersion":1,"progressBudgetVersion":1});
            r.workbench.execution.units.push(autonomy::WorkUnit {
                id: "long-work".into(),
                status: "running".into(),
                elapsed_ms: 30 * 24 * 60 * 60_000,
                attempt_started_at: "2020-01-01T00:00:00Z".into(),
                ..Default::default()
            });
            r.workbench.execution.active_unit_id = Some("long-work".into());
            r.workbench.execution.state = "running".into();
            Ok(())
        })
        .unwrap();
    let mut r = f.record();
    for kind in ["implementation", "recovery", "finalization"] {
        r.workbench.execution.units[0].kind = kind.into();
        let u = &r.workbench.execution.units[0];
        assert!(!autonomy_budget::exhausted(&r, u));
        assert!(!execution_loop::budget_exhausted(&r, u, 99_000_000));
        assert!(!progress_budget::exhausted(&r, u, 99_000_000));
    }
    assert!(!f.service.production_execution_budget_due(&f.id).unwrap());
    assert_eq!(
        r.workbench.execution.units[0].elapsed_ms,
        30 * 24 * 60 * 60_000
    );
}

#[test]
fn migration_reopens_only_expired_work_without_resuming_or_losing_history() {
    let f = Fixture::new();
    let mut r = f.record();
    r.policy["executionTimePolicy"] = json!("unlimited");
    let scope = autonomy::scope(&r);
    for (id, status, reason) in [
        ("expired", "deferred", "budget-exhausted"),
        ("unknown", "awaiting-condition", "unknown-result"),
        ("external", "deferred", "external-condition"),
        ("done", "succeeded", ""),
    ] {
        r.workbench.execution.units.push(autonomy::WorkUnit {
            id: id.into(),
            cycle: r.cycle,
            scope_hash: scope.clone(),
            status: status.into(),
            defer_reason_code: reason.into(),
            elapsed_ms: 999_000,
            result_hash: "original".into(),
            ..Default::default()
        });
    }
    let approvals = serde_json::to_value(&r.approvals).unwrap();
    execution_time::migrate(&mut r);
    assert_eq!(r.workbench.execution.state, "paused");
    assert_eq!(r.workbench.execution.units[0].status, "pending");
    assert_eq!(r.workbench.execution.units[1].status, "awaiting-condition");
    assert_eq!(r.workbench.execution.units[2].status, "deferred");
    assert_eq!(r.workbench.execution.units[3].status, "succeeded");
    assert!(
        r.workbench
            .execution
            .units
            .iter()
            .all(|u| u.elapsed_ms == 999_000 && u.result_hash == "original")
    );
    assert_eq!(serde_json::to_value(&r.approvals).unwrap(), approvals);
}

#[test]
fn test_plan_requires_current_requirement_quotes_and_rejects_fabricated_results() {
    let f = Fixture::new();
    f.plan();
    let mut r = f.record();
    r.policy["runtimeTestPlanVersion"] = json!(1);
    assert!(test_plan::validate(&r, &f.workspace).is_err());
    let requirement = &r.documents["requirements"];
    let mut plan = json!({"schema":"abya.test-plan/v1","requirementsHash":requirement.sha256,
        "cases":[{"id":"input","requirementQuote":requirement.content,
        "roles":["Host"],"precondition":"loaded candidate","actions":"actual input",
        "expected":"observable state transition","evidence":"state and screenshot"}]});
    let save = |plan: &Value| {
        files::write_file(
            &f.workspace,
            "artifacts/runtime-tests/plan.json",
            &serde_json::to_vec(plan).unwrap(),
        )
        .unwrap()
    };
    save(&plan);
    test_plan::validate(&r, &f.workspace).unwrap();
    plan["cases"][0]["passed"] = json!(true);
    save(&plan);
    assert!(test_plan::validate(&r, &f.workspace).is_err());
    plan["cases"][0].as_object_mut().unwrap().remove("passed");
    plan["requirementsHash"] = json!("stale");
    save(&plan);
    assert!(test_plan::validate(&r, &f.workspace).is_err());
}

#[test]
fn legacy_resume_migrates_only_time_policy_once_and_preserves_other_contracts() {
    let f = Fixture::new();
    f.plan();
    let before = f.record();
    f.service
        .production_remove_execution_deadlines(&f.id)
        .unwrap();
    let after = f.record();
    assert_eq!(after.policy["executionTimePolicy"], "unlimited");
    assert_eq!(after.workflow_version, before.workflow_version);
    assert_eq!(after.current_stage, before.current_stage);
    assert_eq!(
        after.policy["runtimeTestPlanVersion"],
        before.policy["runtimeTestPlanVersion"]
    );
    assert_eq!(
        serde_json::to_value(&after.approvals).unwrap(),
        serde_json::to_value(&before.approvals).unwrap()
    );
    f.service
        .production_remove_execution_deadlines(&f.id)
        .unwrap();
    assert_eq!(f.record().revision, after.revision);
}
