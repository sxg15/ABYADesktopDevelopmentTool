use super::*;

#[test]
fn current_request_survives_finished_attempt_and_readonly_legacy_projection() {
    for stage in ["implementation", "review"] {
        let f = Fixture::new();
        f.service
            .production_commit(&f.id, f.record().revision, |r| {
                r.current_stage = stage.into();
                r.stages.insert(stage.into(), "in-progress".into());
                Ok(())
            })
            .unwrap();
        action(&f, "request", json!({"id":"old","text":"旧修改"})).unwrap();
        let old = f
            .service
            .production_begin_attempt(
                &f.id,
                None,
                WorkbenchAttempt {
                    id: "old-attempt".into(),
                    goal_id: "request:old".into(),
                    status: "running".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        f.service
            .production_attempt_status(&f.id, &old.id, "interrupted", None, "旧中断")
            .unwrap();
        action(&f, "request", json!({"id":"new","text":"新的插针修改"})).unwrap();
        assert_eq!(
            f.record().workbench.active_goal_id.as_deref(),
            Some("request:new")
        );
        let new = f
            .service
            .production_begin_attempt(
                &f.id,
                None,
                WorkbenchAttempt {
                    id: "new-attempt".into(),
                    goal_id: "request:new".into(),
                    status: "running".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        f.service
            .production_attempt_status(&f.id, &new.id, "finished", None, "等待包确认")
            .unwrap();
        let before = f.record();
        assert_eq!(
            before.workbench.active_goal_id.as_deref(),
            Some("request:new")
        );
        assert_eq!(
            before.workbench.requests.last().unwrap()["status"],
            "pending"
        );
        // Old installations persisted the wrong pointer. Reading must repair only
        // the projected focus, not complete requests or rewrite any stored record.
        f.service
            .database
            .with_connection(|db| {
                let mut legacy = load(db, &f.id)?.unwrap();
                legacy.workbench.active_goal_id = Some("request:old".into());
                db.execute(
                    "UPDATE task_production SET body=?2 WHERE task_id=?1",
                    rusqlite::params![f.id, serde_json::to_string(&legacy)?],
                )?;
                Ok(())
            })
            .unwrap();
        let read = f.record();
        assert_eq!(
            read.workbench.active_goal_id.as_deref(),
            Some("request:new")
        );
        assert_eq!(read.revision, before.revision);
        assert_eq!(
            serde_json::to_value(&read.workbench.attempts).unwrap(),
            serde_json::to_value(&before.workbench.attempts).unwrap()
        );
        assert_eq!(read.workbench.requests, before.workbench.requests);
        assert_eq!(
            f.service
                .database
                .with_connection(|db| Ok(load(db, &f.id)?.unwrap().workbench.active_goal_id))
                .unwrap()
                .as_deref(),
            Some("request:old")
        );
        action(&f, "save-draft", json!({"key":"current","text":"保留草稿"})).unwrap();
        assert_eq!(
            f.record().workbench.active_goal_id.as_deref(),
            Some("request:new")
        );
    }
}

#[test]
fn current_goal_preserves_live_and_uncertain_execution_without_reviving_older_attempts() {
    let f = Fixture::new();
    action(&f, "request", json!({"id":"old","text":"旧修改"})).unwrap();
    action(&f, "request", json!({"id":"new","text":"新修改"})).unwrap();
    let mut r = f.record();
    for state in ["running", "sending", "queued", "reconciling", "needsReview"] {
        r.workbench.attempts = vec![WorkbenchAttempt {
            goal_id: "request:old".into(),
            status: state.into(),
            ..Default::default()
        }];
        assert_eq!(
            workbench::current_goal(&r).as_deref(),
            Some("request:old"),
            "{state}"
        );
    }
    for state in ["finished", "failed", "paused", "interrupted"] {
        r.workbench.attempts = vec![
            WorkbenchAttempt {
                goal_id: "request:old".into(),
                status: "interrupted".into(),
                ..Default::default()
            },
            WorkbenchAttempt {
                goal_id: "request:new".into(),
                status: state.into(),
                ..Default::default()
            },
        ];
        assert_eq!(
            workbench::current_goal(&r).as_deref(),
            Some("request:new"),
            "{state}"
        );
    }
}
#[test]
fn workbench_package_reports_missing_and_changed_evidence_without_claiming_inclusion() {
    use std::io::Read;
    let f = Fixture::new();
    f.service.production_commit(&f.id,f.record().revision,|r| {
        r.issues.push(json!({"id":"source","kind":"blocker","stage":"implementation","status":"open","description":"底层源码问题","repairTarget":"abya-source","evidenceIds":["missing"]}));
        Ok(())
    }).unwrap();
    let before = f.record();
    let name = f
        .service
        .export_developer_package(&f.id, before.revision, &["source".into()])
        .unwrap();
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(f.workspace.join(name)).unwrap()).unwrap();
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    let data: Value = serde_json::from_str(&manifest).unwrap();
    assert!(
        data["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == "missing" && v["status"] == "omitted")
    );
    assert!(archive.by_name("report.md").is_ok());
    assert_eq!(f.record().revision, before.revision);
}
fn action(f: &Fixture, operation: &str, data: Value) -> AppResult<ProductionView> {
    f.service.production_workbench(ProductionMutation {
        task_id: f.id.clone(),
        expected_revision: f.record().revision,
        operation: operation.into(),
        data,
    })
}
#[test]
fn workbench_preserves_history_and_does_not_split_internal_updates() {
    let f = Fixture::new();
    let initial = f.record();
    action(&f, "save-draft", json!({"key":"current","text":"草稿"})).unwrap();
    let next = f.record();
    assert_eq!(initial.workbench.goals.len(), next.workbench.goals.len());
    let historical = f
        .service
        .production_snapshot(&f.id, initial.revision)
        .unwrap();
    assert_eq!(historical.workbench.drafts.get("current"), None);
    action(&f, "request", json!({"id":"request-one","text":"修改布局"})).unwrap();
    let count = f.record().workbench.goals.len();
    action(&f, "request", json!({"id":"request-one","text":"修改布局"})).unwrap();
    assert_eq!(f.record().workbench.requests.len(), 1);
    assert_eq!(f.record().workbench.goals.len(), count);
    assert!(action(&f, "request", json!({"id":"request-one","text":"不同内容"})).is_err());
    assert_eq!(initial.approvals.len(), f.record().approvals.len());
}
#[test]
fn workbench_deferral_never_resolves_or_skips_required_issues() {
    let f = Fixture::new();
    for (id, kind) in [("optional", "suggestion"), ("required", "blocker")] {
        f.apply("save-issue",json!({"id":id,"kind":kind,"status":"open","stage":"requirements","description":"测试问题","resumeWhen":"修复并复验"})).unwrap();
    }
    assert!(
        action(
            &f,
            "defer",
            json!({"id":"required","reason":"稍后","impact":"阻塞"})
        )
        .is_err()
    );
    action(
        &f,
        "defer",
        json!({"id":"optional","reason":"本次不做","impact":"保留当前表现"}),
    )
    .unwrap();
    assert_eq!(f.record().issues[0]["status"], "open");
    assert!(validation::clear_issues(&f.record(), "requirements").is_err());
    action(&f, "restore", json!({"id":"optional"})).unwrap();
    assert_eq!(f.record().workbench.deferrals[0]["status"], "restored");
}
#[test]
fn workbench_legacy_projection_is_stable_and_readonly() {
    let f = Fixture::new();
    f.service
        .database
        .with_connection(|c| {
            let mut r = load(c, &f.id)?.unwrap();
            r.workbench = WorkbenchState::default();
            c.execute(
                "UPDATE task_production SET body=?2 WHERE task_id=?1",
                rusqlite::params![f.id, serde_json::to_string(&r)?],
            )?;
            Ok(())
        })
        .unwrap();
    let first = f.record();
    let second = f.record();
    assert_eq!(
        serde_json::to_value(&first.workbench).unwrap(),
        serde_json::to_value(&second.workbench).unwrap()
    );
    assert!(
        f.service
            .database
            .with_connection(|c| Ok(load(c, &f.id)?.unwrap().workbench.goals.is_empty()))
            .unwrap()
    );
}
#[test]
fn workbench_import_preserves_originals_and_rejects_archive_traversal() {
    use base64::Engine as _;
    use std::io::{Cursor, Write};
    let f = Fixture::new();
    action(&f,"attach",json!({"name":"需求.md","base64":base64::engine::general_purpose::STANDARD.encode("# 原始要求\n保留图片") })).unwrap();
    let r = f.record();
    assert_eq!(r.artifacts.last().unwrap()["plannerInput"], true);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("../outside.md", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"bad").unwrap();
    let data = zip.finish().unwrap().into_inner();
    assert!(action(&f,"attach",json!({"name":"需求.zip","base64":base64::engine::general_purpose::STANDARD.encode(data)})).is_err());
    assert_eq!(f.record().artifacts.len(), r.artifacts.len());
}
