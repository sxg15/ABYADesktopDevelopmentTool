use super::*;
use std::fs;

const SKILL: &str = ".codex/skills/abya-game-development-task";
fn setup() -> (Fixture, PathBuf) {
    let f = Fixture::new();
    let package = f.root.join("release");
    for (root, content) in [(&f.workspace, "original"), (&package, "new")] {
        fs::create_dir_all(root.join(SKILL)).unwrap();
        fs::write(root.join(SKILL).join("SKILL.md"), content).unwrap();
    }
    f.service
        .production_commit(&f.id, f.record().revision, |r| {
            r.policy = policy();
            r.workflow_version = "2.12.0".into();
            r.current_stage = "review".into();
            r.stages.remove("delivery");
            r.skill_pins = catalog::scan(&f.workspace)?;
            Ok(())
        })
        .unwrap();
    (f, package)
}
#[test]
fn archived_project_adapts_once_and_backup_retains_original_record_and_skill() {
    let (f, package) = setup();
    f.service.set_status(&f.id, TaskStatus::Archived).unwrap();
    let before = f.record();
    let recovery = f.root.join("recovery");
    let run = || {
        f.service
            .migrate_desktop_projects(9, &package, &recovery)
            .unwrap()
    };
    assert_eq!(run()[0].status, "adapted");
    assert_eq!(
        fs::read_to_string(f.workspace.join(SKILL).join("SKILL.md")).unwrap(),
        "new"
    );
    let after = f.record();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.current_stage, "review");
    let report =
        fs::read_to_string(f.workspace.join(ROOT).join("reports/review-report.html")).unwrap();
    assert!(report.contains("未记录"));
    assert_eq!(after.current_version, before.current_version);
    assert_eq!(
        f.service.get_readonly(&f.id).unwrap().status,
        TaskStatus::Archived
    );
    let backup = fs::read_dir(&recovery)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let db = rusqlite::Connection::open(&backup).unwrap();
    assert_eq!(load(&db, &f.id).unwrap().unwrap().revision, before.revision);
    let dir = fs::read_dir(f.workspace.join(ROOT).join("release-backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::read_to_string(dir.join("old").join(SKILL).join("SKILL.md")).unwrap(),
        "original"
    );
    run();
    assert_eq!(f.record().revision, after.revision);
    assert_eq!(fs::read_dir(&recovery).unwrap().count(), 1);
    assert_eq!(
        f.service
            .migrate_desktop_projects(8, &package, &recovery)
            .unwrap()[0]
            .status,
        "review"
    );
}
#[test]
fn conflicting_edits_and_incompatible_stages_require_review_without_mutation() {
    let (f, package) = setup();
    let file = f.workspace.join(SKILL).join("SKILL.md");
    fs::write(&file, "mine").unwrap();
    let run = || {
        f.service
            .migrate_desktop_projects(2, &package, &f.root.join("backups"))
            .unwrap()
    };
    let before = f.record().revision;
    assert_eq!(run()[0].status, "review");
    assert_eq!(f.record().revision, before);
    assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
    fs::write(&file, "original").unwrap();
    f.service
        .production_commit(&f.id, before, |r| {
            r.policy["stages"] = json!([]);
            Ok(())
        })
        .unwrap();
    assert_eq!(run()[0].status, "review");
    assert_eq!(fs::read_to_string(file).unwrap(), "original");
    assert!(!f.root.join("backups").exists());
}

#[test]
#[ignore = "Requires a user-selected exported project. All writes use a temporary workspace and database."]
fn replay_exported_project_from_a_read_only_source() {
    let source = PathBuf::from(
        std::env::var_os("ABYA_MIGRATION_FIXTURE").expect("Set exported workspace path"),
    );
    let original = fs::read(source.join(ROOT).join("workflow.json")).unwrap();
    let record: ProductionRecord = serde_json::from_slice(&original).unwrap();
    let f = Fixture::new();
    let ws = f.root.join("isolated-project");
    fs::create_dir_all(&ws).unwrap();
    for pin in &record.skill_pins {
        let from = files::safe_path(&source, &pin.path).unwrap();
        let to = files::safe_path(&ws, &pin.path).unwrap();
        crate::modules::tasks::copy_directory(&from, &to).unwrap();
    }
    let package = f.root.join("next-package");
    crate::modules::tasks::copy_directory(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../Publish/.codex"),
        &package.join(".codex"),
    )
    .unwrap();
    // Exercise a release that really changes a managed file even when the live project is already current.
    fs::write(
        package.join(SKILL).join("migration-fixture.txt"),
        "release migration probe",
    )
    .unwrap();
    f.service.database.with_connection(|c|{
        c.execute("DELETE FROM development_tasks WHERE id=?1",[&f.id])?;
        c.execute("INSERT INTO development_tasks(id,title,status,created_at,updated_at,workspace_path) VALUES(?1,'migration fixture','active',?2,?2,?3)",rusqlite::params![record.task_id,now(),ws.to_string_lossy()])?;
        c.execute("INSERT INTO task_production(task_id,revision,body) VALUES(?1,?2,?3)",rusqlite::params![record.task_id,record.revision as i64,std::str::from_utf8(&original).unwrap()])?;
        Ok(())
    }).unwrap();
    let run = || {
        f.service
            .migrate_desktop_projects(1, &package, &f.root.join("recovery"))
            .unwrap()
    };
    let report = run();
    assert_eq!(report[0].status, "adapted", "{}", report[0].reason);
    let after = f
        .service
        .database
        .with_connection(|c| load(c, &record.task_id))
        .unwrap()
        .unwrap();
    let mut expected = serde_json::to_value(&record).unwrap();
    let mut actual = serde_json::to_value(&after).unwrap();
    for name in ["revision", "updatedAt", "skillPins"] {
        actual.as_object_mut().unwrap().remove(name);
        expected.as_object_mut().unwrap().remove(name);
    }
    assert_eq!(
        actual, expected,
        "Project state changed outside the release adaptation"
    );
    run();
    assert_eq!(
        f.service
            .database
            .with_connection(|c| load(c, &record.task_id))
            .unwrap()
            .unwrap()
            .revision,
        after.revision
    );
    println!(
        "Replayed project revision {} -> {}, stage {}, version {:?}",
        record.revision, after.revision, after.current_stage, after.current_version
    );
}

#[test]
fn interrupted_upgrade_restores_original_then_applies_current_release() {
    let (f, package) = setup();
    let recovery = f.root.join("recovery");
    fs::create_dir_all(&recovery).unwrap();
    let revision = f.record().revision;
    let batch = uuid::Uuid::new_v4();
    let old = f
        .workspace
        .join(ROOT)
        .join("release-backups")
        .join(batch.to_string())
        .join("old")
        .join(SKILL);
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    let hash = catalog::tree_hash(&f.workspace, SKILL).unwrap();
    fs::rename(f.workspace.join(SKILL), &old).unwrap();
    let pending = recovery.join(format!("pending-{}.json", f.id));
    fs::write(
        &pending,
        serde_json::to_vec(&json!({"task_id":f.id,"batch":batch,
        "workspace":f.workspace.canonicalize().unwrap(),
        "swaps":[{"path":SKILL,"old":hash,"new":catalog::tree_hash(&package,SKILL).unwrap()}]}))
        .unwrap(),
    )
    .unwrap();
    let report = f
        .service
        .migrate_desktop_projects(9, &package, &recovery)
        .unwrap();
    assert_eq!(report[0].status, "adapted", "{}", report[0].reason);
    assert!(!pending.exists());
    assert_eq!(f.record().revision, revision + 1);
    assert_eq!(
        fs::read_to_string(f.workspace.join(SKILL).join("SKILL.md")).unwrap(),
        "new"
    );
}

#[test]
fn compatible_version_bump_preserves_existing_preflight() {
    let (f, package) = setup();
    f.service
        .production_commit(&f.id, f.record().revision, |record| {
            record.workflow_version = "2.11.9".into();
            record.policy["workflowVersion"] = json!("2.11.9");
            record.workbench.execution.preflight.state = "verified".into();
            Ok(())
        })
        .unwrap();
    let report = f
        .service
        .migrate_desktop_projects(1, &package, &f.root.join("backup"))
        .unwrap();
    assert_eq!(report[0].status, "adapted", "{}", report[0].reason);
    let record = f.record();
    assert_eq!(record.workflow_version, "2.12.0");
    assert_eq!(record.workbench.execution.preflight.state, "verified");
    assert_eq!(record.current_stage, "review");
}
