use super::art_design_tests::{direction, enabled_fixture, prepare, submit_art};
use super::*;

#[test]
fn linked_answer_unblocks_art_review_without_changing_kind_or_approving() {
    let f = enabled_fixture();
    let image = prepare(&f);
    let mut old = f.record().art_design.requirements[0].clone();
    old["id"] = json!("ART-18");
    old["kind"] = json!("clarify");
    f.apply("save-art-requirement", old.clone()).unwrap();
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.question_mode = "ask".into();
            Ok(())
        })
        .unwrap();
    f.apply("publish-questions",json!({"id":"timing","stage":"resources","title":"Timing","provider":"codex",
        "conversationId":uuid::Uuid::new_v4(),"questions":[{"id":"delay","text":"How long?","clarifiesRequirementId":"ART-18"}]})).unwrap();
    let answer = |op: &str, data: Value| {
        f.service.production_answer(ProductionMutation {
            task_id: f.id.clone(),
            expected_revision: f.record().revision,
            operation: op.into(),
            data,
        })
    };
    answer(
        "save-draft",
        json!({"id":"timing","answers":{"delay":"500 ms"}}),
    )
    .unwrap();
    assert!(!art_clarification::resolved(&f.record(), &old));
    answer(
        "submit-answers",
        json!({"id":"timing","answers":{"delay":"500 ms"}}),
    )
    .unwrap();
    let source = f
        .record()
        .art_design
        .sources
        .into_iter()
        .find(|s| s["id"] == "answer:timing:delay")
        .unwrap();
    let mut adopted = json!({"id":"ART-19","kind":"must","sourceId":source["id"],"sourceHash":source["hash"],"quote":"500 ms",
        "interpretation":"Use the approved delay","scope":"timing","acceptance":"500 ms","answerRevision":2});
    assert!(f.apply("save-art-requirement", adopted.clone()).is_err());
    adopted["answerRevision"] = json!(1);
    f.apply("save-art-requirement", adopted).unwrap();
    f.apply("review-art-source",json!({"sourceId":source["id"],"sourceHash":source["hash"],"requirementIds":["ART-19"],"reason":"timing"})).unwrap();
    direction(&f);
    for q in ["ART-1", "ART-2", "ART-18", "ART-19"] {
        f.apply("save-art-coverage",json!({"requirementId":q,"artifactIds":[image],"designLocation":"Timing panel","status":"addressed"})).unwrap();
    }
    let key = format!("resources:{}:stage", f.record().cycle);
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.workbench.execution.state = "technicalReview".into();
            r.workbench.execution.technical_queue.insert(
                key.clone(),
                json!({"stage":"resources","reason":"no-artifact-progress","status":"open"}),
            );
            Ok(())
        })
        .unwrap();
    let approvals = f.record().approvals.len();
    submit_art(&f).unwrap();
    let r = f.record();
    assert_eq!(r.approvals.len(), approvals);
    assert_eq!(r.stages["resources"], "awaiting-confirmation");
    assert_eq!(
        r.workbench.execution.technical_queue[&key]["status"],
        "resolved"
    );
    let old = r
        .art_design
        .requirements
        .iter()
        .find(|q| q["id"] == "ART-18")
        .unwrap();
    assert_eq!(old["kind"], "clarify");
    assert!(art_clarification::resolved(&r, old));
    answer("amend", json!({"id":"timing"})).unwrap();
    assert!(!art_clarification::resolved(&f.record(), old));
    answer(
        "submit-answers",
        json!({"id":"timing","answers":{"delay":"500 ms"}}),
    )
    .unwrap();
    assert!(!art_clarification::resolved(&f.record(), old));
}
#[test]
fn design_blocker_requires_current_reviewed_design_evidence() {
    let f = enabled_fixture();
    let mut issue = json!({"id":"review-gate","kind":"blocker","status":"open","stage":"resources","description":"Art submission","resumeWhen":"Verified report"});
    f.apply("save-issue", issue.clone()).unwrap();
    let path = "artifacts/design-receipt.json";
    files::write_file(&f.workspace, path, b"{}").unwrap();
    f.apply("register-evidence",json!({"id":"receipt","path":path,"kind":"json","captureType":"state","reviewed":true,"description":"Verified design state"})).unwrap();
    issue["status"] = json!("resolved");
    issue["fix"] = json!("Answered");
    issue["recheck"] = json!("Reviewed");
    issue["evidenceIds"] = json!(["receipt"]);
    let mut runtime = issue.clone();
    runtime["stage"] = json!("implementation");
    assert!(f.apply("save-issue", runtime).is_err());
    files::write_file(&f.workspace, path, b"changed").unwrap();
    assert!(f.apply("save-issue", issue.clone()).is_err());
    files::write_file(&f.workspace, path, b"{}").unwrap();
    f.apply("save-issue", issue).unwrap();
}

#[test]
fn active_design_scope_allows_document_work_without_a_player_grant() {
    let f = enabled_fixture();
    let before = f.record().revision;
    let current = f.service.production_current(&f.id).unwrap();
    assert_eq!(current["designRuntimeScope"]["playerRequiredForSubmission"], false);
    assert_eq!(current["designRuntimeScope"]["effectivePrototypeGrants"], json!([]));
    assert_eq!(f.record().revision, before);
}
