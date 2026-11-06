use super::*;

fn fixture() -> (Fixture, Value) {
    let f = Fixture::new();
    f.doc("requirements", "Synthetic preflight contract fixture");
    f.accept("requirements");
    f.apply(
        "complete-stage",
        json!({"stage":"resources","summary":"Fixture resources"}),
    )
    .unwrap();
    let player = f.root.join("fixture-player.exe");
    std::fs::write(&player, b"synthetic build; never executed").unwrap();
    f.service.database.with_connection(|db|{
        db.execute("INSERT OR REPLACE INTO settings(key,value_json,updated_at) VALUES('application',?1,'fixture')",[json!({"gameExecutablePath":player}).to_string()])?;Ok(())
    }).unwrap();
    let p = f
        .service
        .begin_capability_preflight(&f.id, false, false)
        .unwrap();
    (f, p)
}

fn successful_contract(f: &Fixture, p: &mut Value) {
    // 纯状态契约夹具，不宣称真实 Player 测试成功。
    p["state"] = json!("passed");
    p["checks"] = json!(
        [
            "runtime-connect",
            "create-archive",
            "save-archive",
            "cold-load"
        ]
        .map(|id| json!({"id":id,"status":"passed"}))
    );
    f.service
        .cleanup_capability_preflight_files(&f.id, p["runId"].as_str().unwrap())
        .unwrap();
    p["cleanup"] = json!({"state":"completed"});
    f.service.publish_capability_preflight(&f.id, p).unwrap();
}

fn approved_maintenance(stage: &str) -> (Fixture, Value) {
    let (f, mut p) = fixture();
    successful_contract(&f, &mut p);
    let report = f.record().workbench.execution.capability_preflight["reportPath"]
        .as_str()
        .unwrap()
        .to_owned();
    f.doc(
        "plan",
        &format!("Approved fixture implementation. {report}"),
    );
    f.accept("plan");
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.current_stage = stage.into();
            Ok(())
        })
        .unwrap();
    (f, p)
}

#[test]
fn review_preflight_reopens_editing_in_place_without_new_archive_or_approvals() {
    let (f, _) = approved_maintenance("review");
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.workbench.execution.capability_preflight = Value::Null;
            Ok(())
        })
        .unwrap();
    let before = f.record();
    assert!(f.service.validate_current_preflight(&f.id).is_err());
    let mut p = f
        .service
        .begin_capability_preflight(&f.id, false, false)
        .unwrap();
    assert_eq!(p["stage"], "review");
    assert_eq!(p["purpose"], "maintenance");
    assert_eq!(p["reuseExisting"], true);
    p["state"] = json!("existing-verified");
    p["checks"] = json!(
        ["runtime-connect", "save-archive", "cold-load"]
            .map(|id| json!({"id":id,"status":"passed"}))
    );
    f.service
        .cleanup_capability_preflight_files(&f.id, p["runId"].as_str().unwrap())
        .unwrap();
    p["cleanup"] = json!({"state":"completed"});
    f.service.publish_capability_preflight(&f.id, &p).unwrap();
    f.service.validate_current_preflight(&f.id).unwrap();
    let player = PathBuf::from(p["playerPath"].as_str().unwrap());
    f.service.check_task_player(&f.id, &player).unwrap();
    let after = f.record();
    assert_eq!(after.current_stage, "review");
    assert_eq!(
        serde_json::to_value(after.approvals).unwrap(),
        serde_json::to_value(before.approvals).unwrap()
    );
    assert_eq!(
        serde_json::to_value(after.documents).unwrap(),
        serde_json::to_value(before.documents).unwrap()
    );
    assert!(
        after
            .issues
            .iter()
            .all(|i| i["id"] != capability_preflight::ISSUE)
    );
    assert_eq!(
        f.service
            .begin_capability_preflight(&f.id, false, false)
            .unwrap()["runId"],
        p["runId"]
    );
}

#[test]
fn maintenance_checks_keep_authorization_and_do_not_reuse_failed_creation_route() {
    for stage in ["implementation", "review"] {
        let (f, mut old) = approved_maintenance(stage);
        old["state"] = json!("failed");
        old["error"] =
            json!({"code":"preflight_capability_unavailable","message":"create unavailable"});
        f.service.publish_capability_preflight(&f.id, &old).unwrap();
        let p = f
            .service
            .begin_capability_preflight(&f.id, true, false)
            .unwrap();
        assert_ne!(p["runId"], old["runId"]);
        assert_eq!(f.record().current_stage, stage);
        let mut failed = p.clone();
        failed["error"] = json!({"code":"timeout","message":"synthetic failure"});
        failed["state"] = json!("failed");
        f.service
            .publish_capability_preflight(&f.id, &failed)
            .unwrap();
        assert_eq!(
            f.record()
                .issues
                .iter()
                .find(|i| i["id"] == capability_preflight::ISSUE)
                .unwrap()["stage"],
            stage
        );
        assert!(f.service.validate_current_preflight(&f.id).is_err());
    }
    let (f, _) = approved_maintenance("review");
    let r = f.record();
    files::write_file(
        &f.workspace,
        &r.documents["plan"].path,
        b"unapproved change",
    )
    .unwrap();
    assert!(
        f.service
            .begin_capability_preflight(&f.id, true, true)
            .is_err()
    );
}

#[test]
fn preflight_remains_restricted_to_active_authorized_stages() {
    for stage in ["requirements", "resources", "delivery", "closeout"] {
        let (f, _) = approved_maintenance(stage);
        assert!(
            f.service
                .begin_capability_preflight(&f.id, true, true)
                .is_err()
        );
    }
    let (f, _) = approved_maintenance("review");
    f.service
        .database
        .with_connection(|db| {
            db.execute(
                "UPDATE development_tasks SET status='archived' WHERE id=?1",
                [&f.id],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(
        f.service
            .begin_capability_preflight(&f.id, true, true)
            .is_err()
    );
}

#[test]
fn changed_player_is_reported_before_preflight_and_moved_build_is_rechecked() {
    let (f, p) = approved_maintenance("review");
    let initial = f.service.task_player_status(&f.id).unwrap();
    f.service
        .adopt_task_player(&f.id, initial["selectedHash"].as_str().unwrap())
        .unwrap();
    let moved = f.root.join("relocated/fixture-player.exe");
    std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
    std::fs::copy(p["playerPath"].as_str().unwrap(), &moved).unwrap();
    f.service
        .database
        .with_connection(|db| {
            db.execute(
                "UPDATE settings SET value_json=?1 WHERE key='application'",
                [json!({"gameExecutablePath":moved}).to_string()],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        f.service.check_task_player(&f.id, &moved).unwrap_err().code,
        "player_pin_mismatch"
    );
    let next = f.service.task_player_status(&f.id).unwrap();
    f.service
        .adopt_task_player(&f.id, next["selectedHash"].as_str().unwrap())
        .unwrap();
    let fresh = f
        .service
        .begin_capability_preflight(&f.id, false, false)
        .unwrap();
    assert_ne!(fresh["runId"], p["runId"]);
    assert_eq!(fresh["reuseExisting"], true);
}

#[test]
fn failures_are_visible_immediately_and_are_not_all_platform_defects() {
    let (f, mut p) = fixture();
    assert!(f.service.validate_current_preflight(&f.id).is_err());
    for (code, target) in [
        ("preflight_timeout", "other"),
        ("preflight_capability_unavailable", "other"),
        ("preflight_capability_missing", "abya-build"),
    ] {
        p["state"] = json!("failed");
        p["error"] = json!({"code":code,"message":"Synthetic failure"});
        f.service.publish_capability_preflight(&f.id, &p).unwrap();
        let r = f.record();
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0]["repairTarget"], target);
        assert_eq!(r.issues[0]["stage"], "plan");
    }
    assert!(
        f.apply(
            "save-issue",
            json!({"id":"preflight:basic","status":"resolved"})
        )
        .is_err()
    );
    f.service
        .cleanup_capability_preflight_files(&f.id, p["runId"].as_str().unwrap())
        .unwrap();
    assert!(
        files::safe_path(&f.workspace, p["reportPath"].as_str().unwrap())
            .unwrap()
            .is_file()
    );
}

#[test]
fn cleanup_rejects_foreign_owner_and_escape_and_preserves_other_files() {
    let (f, p) = fixture();
    let data = PathBuf::from(p["dataRoot"].as_str().unwrap());
    let run = p["runId"].as_str().unwrap();
    let original = f.root.join("user-original.txt");
    std::fs::write(&original, b"keep").unwrap();
    std::fs::write(data.join("probe.txt"), b"temporary").unwrap();
    let marker = std::fs::read(data.join("owner.json")).unwrap();
    std::fs::write(data.join("owner.json"), br#"{"taskId":"foreign"}"#).unwrap();
    assert!(
        f.service
            .cleanup_capability_preflight_files(&f.id, run)
            .is_err()
    );
    assert!(data.join("probe.txt").exists());
    assert!(
        f.service
            .cleanup_capability_preflight_files(&f.id, "../outside")
            .is_err()
    );
    std::fs::write(data.join("owner.json"), marker).unwrap();
    f.service
        .cleanup_capability_preflight_files(&f.id, run)
        .unwrap();
    assert!(!data.exists());
    assert_eq!(std::fs::read(original).unwrap(), b"keep");
}

#[test]
fn plan_requires_finished_checks_cleanup_current_build_and_report_reference() {
    let (f, mut p) = fixture();
    successful_contract(&f, &mut p);
    f.service.validate_current_preflight(&f.id).unwrap();
    files::write_file(&f.workspace, "artifacts/plan.md", b"Plan without preflight").unwrap();
    assert!(
        f.apply(
            "submit-document",
            json!({"kind":"plan","path":"artifacts/plan.md"})
        )
        .is_err()
    );
    let r = f.record();
    let report = r.workbench.execution.capability_preflight["reportPath"]
        .as_str()
        .unwrap();
    files::write_file(
        &f.workspace,
        "artifacts/plan.md",
        format!("Plan: {report}").as_bytes(),
    )
    .unwrap();
    f.apply(
        "submit-document",
        json!({"kind":"plan","path":"artifacts/plan.md"}),
    )
    .unwrap();
    std::fs::write(f.root.join("fixture-player.exe"), b"different build").unwrap();
    assert!(f.service.validate_current_preflight(&f.id).is_err());
}

#[test]
fn pending_cleanup_and_changed_evidence_cannot_be_reported_as_passed() {
    let (f, mut p) = fixture();
    successful_contract(&f, &mut p);
    p["cleanup"] = json!({"state":"pending","message":"locked"});
    f.service.publish_capability_preflight(&f.id, &p).unwrap();
    assert!(f.service.validate_current_preflight(&f.id).is_err());
    assert_eq!(f.service.preflight_recovery_records().unwrap().len(), 1);
    p["cleanup"] = json!({"state":"completed"});
    f.service.publish_capability_preflight(&f.id, &p).unwrap();
    let report = files::safe_path(&f.workspace, p["reportPath"].as_str().unwrap()).unwrap();
    std::fs::write(report, b"tampered").unwrap();
    assert!(f.service.validate_current_preflight(&f.id).is_err());
}

#[test]
fn verified_workaround_keeps_original_issue_and_requires_plan_disclosure() {
    let (f, mut p) = fixture();
    successful_contract(&f, &mut p);
    p["state"] = json!("workaround-verified");
    p["error"] =
        json!({"code":"preflight_capability_missing","message":"Native create contract missing"});
    f.service.publish_capability_preflight(&f.id, &p).unwrap();
    let r = f.record();
    assert_eq!(r.issues[0]["status"], "open");
    assert_eq!(r.issues[0]["repairTarget"], "abya-build");
    assert!(capability_preflight::workaround(&r, &r.issues[0]));
    assert!(capability_preflight::plan_notes(&r, p["reportPath"].as_str().unwrap()).is_err());
    assert!(
        capability_preflight::plan_notes(
            &r,
            &format!(
                "复用本任务已有存档；新建能力未恢复。{}",
                p["reportPath"].as_str().unwrap()
            )
        )
        .is_ok()
    );
}

#[test]
fn interrupted_preflight_is_cleaned_without_replaying_game_operations() {
    let (f, p) = fixture();
    assert!(f.service.ensure_deletable(&f.id).is_err());
    let data = PathBuf::from(p["dataRoot"].as_str().unwrap());
    std::fs::write(data.join("partial-save.tmp"), b"partial").unwrap();
    let paths = AppPaths {
        data_dir: f.root.clone(),
        database_path: f.root.join("app.db"),
        bootstrap_logs_dir: f.root.join("logs"),
        reports_dir: f.root.join("reports"),
        archive_transfers_dir: f.root.join("transfers"),
        default_archive_root: f.root.join("archives"),
        default_workspace_root: f.root.join("workspaces"),
    };
    let instances = crate::modules::instances::InstanceService::new(
        f.service.database.clone(),
        paths,
        crate::modules::game_connections::GameConnectionService::new(),
    );
    let bridge = crate::modules::runtime_bridge::RuntimeBridgeService::new(
        instances.clone(),
        f.service.clone(),
    );
    bridge.reconcile_capability_preflights().unwrap();
    assert!(!data.exists());
    assert!(instances.list(Some(&f.id)).unwrap().is_empty());
    let record = f.record();
    let finished = &record.workbench.execution.capability_preflight;
    assert_eq!(finished["state"], "interrupted");
    assert_eq!(finished["cleanup"]["state"], "completed");
    assert!(f.service.ensure_deletable(&f.id).is_ok());
    assert!(
        files::safe_path(&f.workspace, finished["reportPath"].as_str().unwrap())
            .unwrap()
            .exists()
    );
    bridge.reconcile_capability_preflights().unwrap();
}

#[cfg(windows)]
#[test]
fn locked_test_file_preserves_owner_for_automatic_retry() {
    use std::os::windows::fs::OpenOptionsExt;
    let (f, p) = fixture();
    let data = PathBuf::from(p["dataRoot"].as_str().unwrap());
    let run = p["runId"].as_str().unwrap();
    std::fs::write(data.join("locked.tmp"), b"temporary").unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(data.join("locked.tmp"))
        .unwrap();
    assert!(
        f.service
            .cleanup_capability_preflight_files(&f.id, run)
            .is_err()
    );
    assert!(data.join("owner.json").exists());
    drop(lock);
    f.service
        .cleanup_capability_preflight_files(&f.id, run)
        .unwrap();
    assert!(!data.exists());
}
