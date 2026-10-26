use super::*;

fn design_fixture() -> Fixture {
    let f = Fixture::new();
    f.doc("requirements", "Fixture scope");
    f.accept("requirements");
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.question_mode = "ask".into();
            r.policy["stageContractsVersion"] = json!(1);
            r.current_stage = "resources".into();
            Ok(())
        })
        .unwrap();
    f
}
#[test]
fn audit_scope_error_stays_open_without_gating_design_or_forging_approval() {
    let f = design_fixture();
    let approvals = f.record().approvals.len();
    f.apply("save-issue",json!({"id":"resources-buttons-architecture-scope","stage":"resources","kind":"blocker","status":"open","repairTarget":"other","category":"contract-gap","owningStage":"implementation","description":"Unknown component scope","resumeWhen":"Read actual schema","blockedOperations":["submit-document","complete-stage"]})).unwrap();
    let r = f.record();
    assert_eq!(
        r.issues[0]["blockingDisposition"],
        "tracked-not-gating-design"
    );
    assert!(validation::clear_issues(&r, "resources").is_ok());
    assert_eq!(r.issues[0]["status"], "open");
    assert_eq!(r.approvals.len(), approvals);
    assert!(
        f.apply(
            "update-stage",
            json!({"stage":"resources","status":"blocked","summary":"schema missing"})
        )
        .is_err()
    );
    assert!(f.service.production_authoring_guard(&f.id, "").is_err());
    assert!(!preflight_recovery::preflight_recovery_manual(&r));
}
#[test]
fn design_gate_still_requires_real_questions_and_intact_approved_inputs() {
    let f = design_fixture();
    f.apply("save-issue",json!({"id":"fake-gate","stage":"resources","kind":"blocker","status":"open","description":"invented","resumeWhen":"answer","gateRef":"question:missing"})).unwrap();
    assert!(validation::document_gate(&f.record(), &f.workspace, "resources").is_ok());
    f.apply("publish-questions",json!({"id":"choice","stage":"resources","title":"Design decision","provider":"codex","conversationId":uuid::Uuid::new_v4().to_string(),"questions":[{"id":"a","text":"Choose input"}]})).unwrap();
    assert!(validation::document_gate(&f.record(), &f.workspace, "resources").is_err());
    files::write_file(&f.workspace, &format!("{ROOT}/requirements.md"), b"changed").unwrap();
    assert!(validation::approved(&f.record(), &f.workspace, "requirements").is_err());
}
#[test]
fn prototype_grant_is_scoped_expiring_and_revocable_without_changing_approvals() {
    let f = design_fixture();
    let folder = f.workspace.join("artifacts/prototypes/fixture");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.to_string_lossy();
    let before = f.record().approvals;
    f.service
        .production_grant_prototype(
            &f.id,
            f.record().revision,
            "synthetic-instance",
            &path,
            "Button hit test",
        )
        .unwrap();
    assert!(
        f.service
            .production_authoring_guard(&f.id, &path)
            .unwrap()
            .is_some()
    );
    assert!(
        f.service
            .production_authoring_guard(&f.id, &f.workspace.to_string_lossy())
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(f.record().approvals).unwrap()
    );
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.workbench.prototype_grants[0]["expiresAt"] = json!("2000-01-01T00:00:00Z");
            Ok(())
        })
        .unwrap();
    assert!(f.service.production_authoring_guard(&f.id, &path).is_err());
    f.service
        .production_grant_prototype(
            &f.id,
            f.record().revision,
            "synthetic-instance",
            &path,
            "Next test",
        )
        .unwrap();
    let id = f.record().workbench.prototype_grants[1]["id"].clone();
    f.service
        .production_workbench(ProductionMutation {
            task_id: f.id.clone(),
            expected_revision: f.record().revision,
            operation: "revoke-prototype".into(),
            data: json!({"id":id}),
        })
        .unwrap();
    assert!(f.service.production_authoring_guard(&f.id, &path).is_err());
}
#[test]
fn migration_preserves_issue_history_and_valid_human_gate() {
    let f = design_fixture();
    let mut r = f.record();
    r.stages.insert("resources".into(), "blocked".into());
    r.workbench.execution.preflight.state = "exhausted".into();
    r.issues.push(json!({"id":"old","stage":"resources","status":"open","kind":"blocker","description":"old runtime error"}));
    let approvals = serde_json::to_value(&r.approvals).unwrap();
    stage_contracts::migrate(&mut r);
    assert_eq!(r.stages["resources"], "in-progress");
    assert_eq!(r.workbench.execution.preflight.state, "not-required");
    assert_eq!(
        r.workbench.execution.events.last().unwrap()["previous"]["state"],
        "exhausted"
    );
    assert_eq!(r.issues[0]["status"], "open");
    assert_eq!(serde_json::to_value(&r.approvals).unwrap(), approvals);
}
