use super::*;
use base64::Engine as _;

const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=";
fn upload(f: &Fixture, revision: u64, data: Value) -> AppResult<ProductionView> {
    f.service.production_feedback(ProductionMutation {
        task_id: f.id.clone(),
        expected_revision: revision,
        operation: "attach-art-reference".into(),
        data,
    })
}

#[test]
fn planner_references_work_before_any_deliverable_and_survive_reload_without_approving() {
    let f = Fixture::new();
    let before = f.record();
    let r = upload(
        &f,
        before.revision,
        json!({"name":"风格参考.png","base64":PNG,"summary":"只参考配色"}),
    )
    .unwrap()
    .record
    .unwrap();
    let a = &r.artifacts[0];
    assert_eq!(a["plannerReference"], true);
    assert_eq!(a["summary"], "只参考配色");
    assert!(
        a["path"]
            .as_str()
            .unwrap()
            .starts_with("artifacts/art-references/")
    );
    assert_eq!(r.stages, before.stages);
    assert_eq!(r.current_stage, before.current_stage);
    assert!(r.approvals.is_empty() && r.documents.is_empty() && r.feedback.is_empty());
    assert_eq!(f.record().artifacts, r.artifacts);
    assert!(
        f.service
            .production_media(&f.id, a["id"].as_str().unwrap())
            .unwrap()
            .ends_with(PNG)
    );
    let exported = std::fs::read_to_string(f.workspace.join(ROOT).join("workflow.json")).unwrap();
    assert!(exported.contains("plannerReference"));
    assert!(interaction::feedback(&r, &f.workspace, a["id"].as_str()).is_err());
    let mut modern = r.clone();
    modern.policy["iterationMode"] = json!("human-feedback");
    assert!(visuals::bindings_for(&modern, &f.workspace, true).is_err());
    assert!(
        visuals::bindings_for(&modern, &f.workspace, false)
            .unwrap()
            .is_empty()
    );
    assert!(report::render(&r, "plan", &[]).contains("策划参考（非验收成果）"));
    assert!(upload(&f, before.revision, json!({"name":"copy.png","base64":PNG})).is_err());
    assert_eq!(f.record().artifacts.len(), 1);
    assert!(
        f.apply(
            "attach-art-reference",
            json!({"name":"forged.png","base64":PNG})
        )
        .is_err()
    );
    f.service
        .production_feedback(ProductionMutation {
            task_id: f.id.clone(),
            expected_revision: r.revision,
            operation: "remove-art-reference".into(),
            data: json!({"id":a["id"]}),
        })
        .unwrap();
    let removed = f.record();
    assert_eq!(removed.artifacts[0]["status"], "withdrawn");
    assert!(f.workspace.join(a["path"].as_str().unwrap()).exists());
    assert!(report::render(&removed, "plan", &[]).contains("已移除，不再使用"));
    f.service.set_status(&f.id, TaskStatus::Archived).unwrap();
    assert!(
        upload(
            &f,
            removed.revision,
            json!({"name":"archived.png","base64":PNG})
        )
        .is_err()
    );
}

#[test]
fn reference_upload_validates_bytes_limits_and_ownership_before_writing() {
    let f = Fixture::new();
    let rev = f.record().revision;
    for data in [
        json!({"name":"bad.png","base64":"%%%"}),
        json!({"name":"fake.png","base64":"PGh0bWw+"}),
        json!({"name":"empty.png","base64":""}),
        json!({"name":"a".repeat(161),"base64":PNG}),
        json!({"name":"a.png","base64":PNG,"summary":"a".repeat(2001)}),
        json!({"name":"big.png","base64":base64::engine::general_purpose::STANDARD.encode(vec![1;16*1024*1024+1])}),
    ] {
        assert!(upload(&f, rev, data).is_err());
    }
    assert!(!f.workspace.join("artifacts/art-references").exists());
    assert_eq!(f.record().revision, rev);
    assert!(
        f.service
            .production_feedback(ProductionMutation {
                task_id: f.id.clone(),
                expected_revision: rev,
                operation: "remove-art-reference".into(),
                data: json!({"id":"foreign"})
            })
            .is_err()
    );
}
