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
#[ignore = "Reads an unpacked handoff selected via ABYA_ART_HANDOFF and writes only an isolated fixture"]
fn replay_legacy_answer_and_stage_receipt() {
    let src = PathBuf::from(std::env::var_os("ABYA_ART_HANDOFF").unwrap());
    let f = Fixture::new();
    let read = |name: &str| -> Value {
        serde_json::from_slice(&std::fs::read(src.join(name)).unwrap()).unwrap()
    };
    crate::modules::tasks::copy_directory(
        &src.join("05-task-artifacts/artifacts"),
        &f.workspace.join("artifacts"),
    )
    .unwrap();
    let mut r: ProductionRecord =
        serde_json::from_value(read("02-recovery-audit/database-record-before-r352.json")).unwrap();
    for doc in r.documents.values() {
        if !files::safe_path(&f.workspace, &doc.path).unwrap().exists() {
            let dest = files::safe_path(&f.workspace, &doc.path).unwrap();
            files::write_file(&f.workspace, &doc.path, doc.content.as_bytes()).unwrap();
            assert_eq!(files::hash_file(&dest).unwrap().0, doc.sha256);
        }
    }
    let mut q = r
        .art_design
        .requirements
        .iter()
        .find(|q| q["id"] == "ART-18")
        .unwrap()
        .clone();
    let original = q.clone();
    q["answerRequirementId"] = json!("ART-19");
    q["answerRevision"] = json!(1);
    assert!(mutation::apply(&mut r, &f.workspace, "save-art-requirement", &q).is_err());
    art_design::apply(&mut r, &f.workspace, "save-art-requirement", &q, true).unwrap();
    let old = r
        .art_design
        .requirements
        .iter()
        .find(|q| q["id"] == "ART-18")
        .unwrap();
    assert!(art_clarification::resolved(&r, old));
    for key in ["quote", "kind", "sourceId", "sourceHash"] {
        assert_eq!(old[key], original[key]);
    }
    for entry in {
        let mut entries = std::fs::read_dir(src.join("02-recovery-audit/receipts"))
            .unwrap()
            .map(|x| x.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(|c: char| c.is_ascii_digit())
            })
            .collect::<Vec<_>>();
        entries.sort();
        entries
    } {
        let v: Value = serde_json::from_slice(&std::fs::read(entry).unwrap()).unwrap();
        let input = &v["input"];
        // This handoff contains the v3 coverage assets but omits eight older
        // preview groups needed by the independent full-package visual gate.
        if input["operation"] == "submit-document" {
            art_design_validation::gate(&r, &f.workspace, "resources", Some("new-art")).unwrap();
            continue;
        }
        mutation::apply(
            &mut r,
            &f.workspace,
            input["operation"].as_str().unwrap(),
            &input["data"],
        )
        .unwrap();
    }
    let mut r: ProductionRecord =
        serde_json::from_value(read("02-recovery-audit/before-local-bookkeeping-r379.json"))
            .unwrap();
    let close = read("02-recovery-audit/issue-close-public-api-result.json");
    let data = &close["input"]["data"];
    let evidence = r
        .evidence
        .iter()
        .find(|e| data["evidenceIds"][0] == e.id)
        .unwrap();
    let bytes =
        std::fs::read(src.join("02-recovery-audit/resubmission-verification.json")).unwrap();
    files::write_file(&f.workspace, &evidence.path, &bytes).unwrap();
    mutation::apply(&mut r, &f.workspace, "save-issue", data).unwrap();
    assert!(
        r.issues
            .iter()
            .any(|i| i["id"] == data["id"] && i["status"] == "resolved")
    );
}
