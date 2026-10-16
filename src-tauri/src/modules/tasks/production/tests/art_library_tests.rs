use super::art_design_tests::{enabled_fixture, prepare};
use super::*;

fn draft(f: &Fixture, id: &str, artifact: &str, parent: Option<(&str, u32)>) {
    let mut data = json!({"id":id,"title":"Object inventory HUD","artifactIds":[artifact],"tags":["desktop","fixed-camera","inventory"],
        "context":"Desktop game using object counts","plannerRequirements":"No numeric stock label; stable player identity",
        "visualRules":"Consistent silhouettes and identity colors","decisions":"Objects make stock visible while aiming",
        "production":"Separate sprite and input layers","applicability":"Small bounded inventories",
        "limitations":"Dense inventories require redesign","sources":"Synthetic local test artwork"});
    if let Some((parent, version)) = parent {
        data["parentCaseId"] = json!(parent);
        data["parentVersion"] = json!(version);
    }
    f.apply("save-art-case-draft", data).unwrap();
}
fn action(f: &Fixture, op: &str, data: Value) -> AppResult<Value> {
    f.service.art_library_action(ProductionMutation {
        task_id: f.id.clone(),
        expected_revision: f.record().revision,
        operation: op.into(),
        data,
    })
}
fn collect(f: &Fixture, id: &str) -> Value {
    action(
        f,
        "collect",
        json!({"draftId":id,"satisfaction":"满意库存图形化的具体表现，不代表任务整体通过"}),
    )
    .unwrap()
}

#[test]
fn collecting_requires_explicit_satisfaction_preserves_task_and_is_idempotent() {
    let f = enabled_fixture();
    let image = prepare(&f);
    draft(&f, "candidate", &image, None);
    let before = serde_json::to_value(f.record()).unwrap();
    assert!(
        action(
            &f,
            "collect",
            json!({"draftId":"candidate","satisfaction":""})
        )
        .is_err()
    );
    assert!(
        f.apply(
            "collect",
            json!({"draftId":"candidate","satisfaction":"fake"})
        )
        .is_err()
    );
    let entry = collect(&f, "candidate");
    assert_eq!(entry["designApproved"], true);
    assert_eq!(entry["runtimeVerified"], false);
    assert_eq!(serde_json::to_value(f.record()).unwrap(), before);
    assert_eq!(collect(&f, "candidate")["id"], entry["id"]);
    assert_eq!(
        f.service
            .art_library_list("inventory", false)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let id = entry["id"].as_str().unwrap();
    let original = f.service.art_library_media(id, 1, 0).unwrap();
    f.service.delete(&f.id).unwrap();
    assert_eq!(f.service.art_library_media(id, 1, 0).unwrap(), original);
    assert_eq!(
        f.service.art_library_get(id, 1).unwrap()["satisfaction"],
        entry["satisfaction"]
    );
}

#[test]
fn revisions_withdrawal_and_frozen_task_references_do_not_mutate_old_cases() {
    let f = enabled_fixture();
    let image = prepare(&f);
    draft(&f, "one", &image, None);
    let one = collect(&f, "one");
    let id = one["id"].as_str().unwrap();
    let before_inspection = art_design_sources::design_hash(&f.record());
    f.apply("inspect-art-case", json!({"caseId":id,"version":1}))
        .unwrap();
    assert!(f.record().art_design.case_uses.is_empty());
    assert_eq!(
        art_design_sources::design_hash(&f.record()),
        before_inspection
    );
    assert!(
        f.record()
            .artifacts
            .iter()
            .any(|a| a["libraryReference"] == true)
    );
    let use_data = json!({"caseId":id,"version":1,"borrow":"Inventory readability","adapt":"Touch spacing","avoid":"Desktop layout"});
    f.apply("use-art-case", use_data.clone()).unwrap();
    let snapshot = f.record().art_design.case_uses[0].clone();
    let copied = snapshot["paths"][0].as_str().unwrap();
    assert!(f.workspace.join(copied).is_file());
    let views = visuals::bindings_for(&f.record(), &f.workspace, false).unwrap();
    assert!(!views.contains_key(copied));
    draft(&f, "two", &image, Some((id, 1)));
    let two = collect(&f, "two");
    assert_eq!(two["version"], 2);
    assert_eq!(f.service.art_library_get(id, 1).unwrap(), one);
    assert_eq!(
        f.service.art_library_list("", false).unwrap()[0]["version"],
        2
    );
    action(
        &f,
        "withdraw",
        json!({"id":id,"version":2,"reason":"Need a better comparison"}),
    )
    .unwrap();
    assert!(
        f.service
            .art_library_list("", false)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    let mut withdrawn = use_data;
    withdrawn["version"] = json!(2);
    assert!(f.apply("use-art-case", withdrawn).is_err());
    assert_eq!(f.record().art_design.case_uses[0], snapshot);
    assert!(f.service.art_library_media(id, 1, 0).is_ok());
}

#[test]
fn portable_case_roundtrip_marks_external_claims_and_checks_integrity() {
    let f = enabled_fixture();
    let image = prepare(&f);
    draft(&f, "one", &image, None);
    let one = collect(&f, "one");
    let exported = action(&f, "export", json!({"id":one["id"],"version":1})).unwrap();
    let path = f.workspace.join(exported["path"].as_str().unwrap());
    assert!(
        f.service
            .production_artifact_path(&f.id, exported["path"].as_str().unwrap())
            .is_ok()
    );
    assert!(
        f.service
            .production_artifact_path(&f.id, "artifacts/arbitrary.zip")
            .is_err()
    );
    let imported = action(&f, "import", json!({"path":path})).unwrap();
    assert_eq!(imported["provenance"], "imported");
    assert_eq!(imported["designApproved"], false);
    assert_eq!(imported["reportedDesignApproved"], true);
    assert_eq!(imported["runtimeVerified"], false);
    assert_eq!(imported["visualRules"], one["visualRules"]);
    assert_ne!(imported["id"], one["id"]);
    assert_eq!(
        action(&f, "import", json!({"path":path})).unwrap()["id"],
        imported["id"]
    );
    let library = f.root.join("ArtLibrary");
    std::fs::write(
        library.join(imported["media"][0]["path"].as_str().unwrap()),
        "modified",
    )
    .unwrap();
    assert!(
        f.service
            .art_library_media(imported["id"].as_str().unwrap(), 1, 0)
            .is_err()
    );
}

#[test]
fn stale_drafts_and_unlisted_zip_payloads_never_enter_library() {
    use std::io::Write;
    let f = enabled_fixture();
    let image = prepare(&f);
    draft(&f, "old", &image, None);
    human(&f, "add-art-note", json!({"text":"另一个明确要求"})).unwrap();
    assert!(
        action(
            &f,
            "collect",
            json!({"draftId":"old","satisfaction":"喜欢旧图"})
        )
        .is_err()
    );
    assert!(
        f.service
            .art_library_list("", true)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    // A syntactically valid manifest cannot authorize arbitrary extraction.
    let path = f.workspace.join("malformed.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("case.json", options).unwrap();
    zip.write_all(br#"{"schema":"abya.art-case/v1","media":[{"path":"media/../outside.png","mediaType":"image/png","sha256":"fake"}]}"#).unwrap();
    zip.start_file("media/../outside.png", options).unwrap();
    zip.write_all(b"fake").unwrap();
    zip.finish().unwrap();
    assert!(action(&f, "import", json!({"path":path})).is_err());
    assert!(
        f.service
            .art_library_list("", true)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(!f.root.join("outside.png").exists());
}

#[test]
fn case_can_preserve_reference_context_without_promoting_it_to_a_deliverable() {
    let f = enabled_fixture();
    let image = prepare(&f);
    let reference = super::art_design_tests::register_image(&f, "reference", "reference");
    draft(&f, "with-reference", &image, None);
    let mut d = f.record().art_design.case_drafts[0].clone();
    d["referenceIds"] = json!([reference]);
    f.apply("save-art-case-draft", d).unwrap();
    let entry = collect(&f, "with-reference");
    assert_eq!(entry["media"].as_array().unwrap().len(), 2);
    assert_eq!(entry["media"][1]["sourceType"], "reference");
    assert_eq!(entry["runtimeVerified"], false);
}
