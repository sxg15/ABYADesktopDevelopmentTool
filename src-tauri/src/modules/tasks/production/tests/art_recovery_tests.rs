use super::*;

/// 复现旧周期反馈已关闭、新周期美术已批准的真实流程。
fn approved_fixture() -> Fixture {
    let f = Fixture::new();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy = prior_policy();
            r.workflow_version = "2.1.0".into();
            Ok(())
        })
        .unwrap();
    f.doc("requirements", "Original rules");
    f.accept("requirements");
    visual(&f, "old");
    submit_art(&f);
    let original = f.record().artifacts[0]["id"].clone();
    human(
        &f,
        "add",
        json!({"description":"Remove inventory numbers","artifactId":original}),
    )
    .unwrap();
    ready(&f, &original);
    let id = f.record().feedback[0]["id"].clone();
    human(&f, "resolve", json!({"id":id})).unwrap();
    submit_art(&f);
    f.accept("resources");
    f.doc("requirements", "Updated rules");
    f.accept("requirements");
    visual(&f, "current");
    submit_art(&f);
    f.accept("resources");
    f
}

fn visual(f: &Fixture, version: &str) {
    let path = format!("artifacts/{version}.png");
    files::write_file(&f.workspace, &path, version.as_bytes()).unwrap();
    f.apply(
        "register-visual",
        json!({"stage":"resources","path":path,"title":"Art","sourceType":"mockup",
        "roles":["asset-board","layout","states"],"groupId":"direction","visualVersion":version}),
    )
    .unwrap();
}

fn submit_art(f: &Fixture) {
    let path = format!("{ROOT}/art.md");
    files::write_file(&f.workspace, &path, b"Create new art").unwrap();
    f.apply("submit-document", json!({"kind":"resources","path":path,"artScope":"new-art","artScopeReason":"Create new art"})).unwrap();
}

fn ready(f: &Fixture, candidate: &Value) {
    let id = f.record().feedback[0]["id"].clone();
    f.apply(
        "update-feedback",
        json!({"id":id,"status":"awaiting-recheck","fix":"Removed numbers",
        "recheck":"Compare the current candidate","candidateArtifactId":candidate}),
    )
    .unwrap();
}

fn reopen_ready(f: &Fixture) -> Value {
    let id = f.record().feedback[0]["id"].clone();
    human(
        f,
        "reopen",
        json!({"id":id,"reason":"Verify the current proposal"}),
    )
    .unwrap();
    let candidate = f.record().artifacts.last().unwrap()["id"].clone();
    ready(f, &candidate);
    id
}

#[test]
fn accepted_art_recovers_after_cross_cycle_feedback_without_another_approval() {
    let f = approved_fixture();
    let approved = f.record();
    let id = reopen_ready(&f);
    let pending = f.service.production_get(&f.id).unwrap();
    assert_eq!(pending.approval_statuses["resources"], "");
    assert_eq!(pending.actions["recheck"][id.as_str().unwrap()], "");
    assert_ne!(pending.actions["resume"]["resources"], "");
    human(&f, "resolve", json!({"id":id})).unwrap();
    let restored = f.record();
    assert_eq!(restored.current_stage, "plan");
    assert_eq!(restored.stages["resources"], "passed");
    assert_eq!(restored.stages["plan"], "in-progress");
    assert_eq!(restored.stages["implementation"], "not-started");
    assert_eq!(
        serde_json::to_value(&restored.approvals).unwrap(),
        serde_json::to_value(&approved.approvals).unwrap()
    );
    assert_eq!(
        restored.documents["resources"].revision,
        approved.documents["resources"].revision
    );
    assert!(validation::approved(&restored, &f.workspace, "resources").is_ok());
}

#[test]
fn stranded_closed_feedback_has_revision_checked_app_only_recovery() {
    let f = approved_fixture();
    let id = reopen_ready(&f);
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.feedback[0]["status"] = json!("resolved");
            Ok(())
        })
        .unwrap();
    let before = serde_json::to_value(f.record()).unwrap();
    let view = f.service.production_get(&f.id).unwrap();
    assert_eq!(view.actions["resume"]["resources"], "");
    assert_eq!(serde_json::to_value(f.record()).unwrap(), before);
    assert!(f.apply("resume-art", json!({})).is_err());
    assert!(
        f.service
            .production_feedback(ProductionMutation {
                task_id: f.id.clone(),
                expected_revision: f.record().revision - 1,
                operation: "resume-art".into(),
                data: json!({})
            })
            .is_err()
    );
    human(&f, "resume-art", json!({})).unwrap();
    assert_eq!(f.record().current_stage, "plan");
    assert_eq!(f.record().feedback[0]["id"], id);
    assert!(human(&f, "resume-art", json!({})).is_err());
}

#[test]
fn changed_document_allows_feedback_closure_but_never_restores_old_approval() {
    let f = approved_fixture();
    let id = reopen_ready(&f);
    files::write_file(&f.workspace, &format!("{ROOT}/art.md"), b"Changed design").unwrap();
    human(&f, "resolve", json!({"id":id})).unwrap();
    let view = f.service.production_get(&f.id).unwrap();
    assert_eq!(view.record.unwrap().current_stage, "resources");
    assert_ne!(view.approval_statuses["resources"], "");
    assert_ne!(view.actions["resume"]["resources"], "");
    assert!(human(&f, "resume-art", json!({})).is_err());
}

#[test]
fn missing_or_changed_candidate_cannot_be_resolved_or_restore_the_stage() {
    let f = approved_fixture();
    let id = reopen_ready(&f);
    files::write_file(&f.workspace, "artifacts/current.png", b"tampered").unwrap();
    assert!(human(&f, "resolve", json!({"id":id})).is_err());
    assert_eq!(f.record().feedback[0]["status"], "awaiting-recheck");
    assert_eq!(f.record().current_stage, "resources");
}

#[test]
fn other_open_feedback_and_art_blockers_prevent_recovery() {
    let f = approved_fixture();
    let candidate = f.record().artifacts.last().unwrap()["id"].clone();
    human(
        &f,
        "add",
        json!({"description":"Another visual problem","artifactId":candidate}),
    )
    .unwrap();
    let id = reopen_ready(&f);
    human(&f, "resolve", json!({"id":id})).unwrap();
    assert_eq!(f.record().current_stage, "resources");
    let f = approved_fixture();
    let id = reopen_ready(&f);
    f.apply("save-issue", json!({"id":"art-blocker","stage":"resources","kind":"blocker","status":"open",
        "description":"Missing art input","resumeWhen":"Fix the missing input","affectedStages":["resources"]})).unwrap();
    human(&f, "resolve", json!({"id":id})).unwrap();
    assert_eq!(f.record().current_stage, "resources");
    assert!(human(&f, "resume-art", json!({})).is_err());
}

#[test]
fn art_reopen_requires_a_reason_and_new_requirements_cannot_reuse_old_art() {
    let f = approved_fixture();
    let id = f.record().feedback[0]["id"].clone();
    assert!(human(&f, "reopen", json!({"id":id})).is_err());
    assert_eq!(f.record().current_stage, "plan");
    f.doc("requirements", "Another cycle");
    f.accept("requirements");
    assert!(validation::accepted_document(&f.record(), &f.workspace, "resources").is_err());
    assert!(human(&f, "resume-art", json!({})).is_err());
}
