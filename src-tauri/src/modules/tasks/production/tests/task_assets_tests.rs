use super::*;

#[test]
fn only_app_selection_can_be_bound_and_switch_requires_acknowledgement() {
    let f = Fixture::new();
    let old = f.root.join("old.exe");
    let new = f.root.join("new.exe");
    std::fs::write(&old, b"old build").unwrap();
    std::fs::write(&new, b"new build").unwrap();
    let select = |path: &Path| {
        f.service.database.with_connection(|db| {
        db.execute("INSERT OR REPLACE INTO settings(key,value_json,updated_at) VALUES('application',?1,'test')", [json!({"gameExecutablePath":path}).to_string()])?;
        Ok(())
    }).unwrap()
    };
    assert!(f.service.production_check_player_pin(&f.id, &old).is_err());
    select(&old);
    f.service.production_check_player_pin(&f.id, &old).unwrap();
    assert_eq!(
        f.service
            .production_check_player_pin(&f.id, &new)
            .unwrap_err()
            .code,
        "app_player_mismatch"
    );
    select(&new);
    assert_eq!(
        f.service
            .production_check_player_pin(&f.id, &new)
            .unwrap_err()
            .code,
        "player_pin_mismatch"
    );
    let status = f.service.task_player_status(&f.id).unwrap();
    assert_eq!(status["matches"], false);
    assert!(f.service.adopt_task_player(&f.id, "stale").is_err());
    f.service
        .adopt_task_player(&f.id, status["selectedHash"].as_str().unwrap())
        .unwrap();
    f.service.production_check_player_pin(&f.id, &new).unwrap();
    assert_eq!(f.record().current_version, None);
    assert!(f.service.production_check_player_pin(&f.id, &old).is_err());
    std::fs::write(&new, b"changed in place").unwrap();
    assert_eq!(
        f.service
            .production_check_player_pin(&f.id, &new)
            .unwrap_err()
            .code,
        "player_pin_mismatch"
    );
}

#[test]
fn private_sources_do_not_require_a_candidate_and_reject_foreign_owner() {
    let f = Fixture::new();
    let data = f.service.owned_authoring_data_root(&f.id).unwrap();
    let source = data.join("Archives/Demo");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("Main.PBArc"), br#"{"Guid":"private"}"#).unwrap();
    let result = f.service.task_archive_sources(&f.id).unwrap();
    assert_eq!(result["editableRoots"][0], json!(data.join("Archives")));
    assert!(result["candidate"].is_null());
    assert_eq!(
        f.service
            .production_owned_archive_root(&f.id, &source.to_string_lossy())
            .unwrap(),
        Some(data.clone())
    );
    std::fs::write(data.join("owner.json"), br#"{"taskId":"foreign"}"#).unwrap();
    assert!(f.service.task_archive_sources(&f.id).is_err());
}

#[test]
fn migrates_owned_saves_without_changing_historical_paths_or_digests() {
    let f = Fixture::new();
    let old = f
        .workspace
        .parent()
        .unwrap()
        .join(".abya-authoring")
        .join(&f.id);
    let archive = old.join("Archives/旧档");
    let snapshot = f
        .workspace
        .join("artifacts/version-snapshots")
        .join(uuid::Uuid::new_v4().to_string())
        .join("旧档");
    for dir in [&archive, &snapshot] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("Main.PBArc"), br#"{"Guid":"keep"}"#).unwrap();
    }
    std::fs::write(
        old.join("owner.json"),
        serde_json::to_vec(&json!({"taskId":f.id})).unwrap(),
    )
    .unwrap();
    let expected = versions::digest(std::slice::from_ref(&archive)).unwrap();
    for _ in 0..2 {
        f.service.migrate_save_layout(&f.id).unwrap();
        let base = f
            .service
            .project_save_folder(&f.id)
            .unwrap()
            .canonicalize()
            .unwrap();
        assert!(
            archive
                .canonicalize()
                .unwrap()
                .starts_with(base.join("编辑数据"))
        );
        assert!(
            snapshot
                .canonicalize()
                .unwrap()
                .starts_with(base.join("版本"))
        );
        assert_eq!(
            expected,
            versions::digest(std::slice::from_ref(&snapshot)).unwrap()
        );
        let aliases = std::fs::read_dir(&base)
            .unwrap()
            .flatten()
            .filter(|e| e.path().canonicalize().ok() == archive.canonicalize().ok())
            .count();
        assert_eq!(aliases, 1);
    }
}
