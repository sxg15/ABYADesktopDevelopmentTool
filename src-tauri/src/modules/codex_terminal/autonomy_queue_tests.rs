use super::*;
use sha2::{Digest, Sha256};
fn seed(f: &Fixture) {
    let task = f.service.tasks.get(&f.task).unwrap();
    let db = rusqlite::Connection::open(f.root.join("db.sqlite")).unwrap();
    let body: String = db
        .query_row(
            "SELECT body FROM task_production WHERE task_id=?1",
            [&f.task],
            |r| r.get(0),
        )
        .unwrap();
    let mut r: Value = serde_json::from_str(&body).unwrap();
    r["policy"]["artDesignVersion"] = Value::Null;
    r["policy"]["stages"][1]["approval"] = Value::Null;
    r["policy"]["implementationExecution"] = json!({"mode":"autonomous"});
    r["feedback"] = json!([{"id":"user-feedback","stage":"review","status":"awaiting-recheck","description":"User will verify after implementation"}]);
    r["currentStage"] = json!("implementation");
    r["stages"]["implementation"] = json!("in-progress");
    r["workbench"]["goals"] = json!([]);
    r["workbench"]["activeGoalId"] = Value::Null;
    for kind in ["requirements", "plan"] {
        let path = format!("artifacts/{kind}-test.md");
        let content = format!("Controlled {kind}");
        let full = Path::new(&task.workspace_path).join(&path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, &content).unwrap();
        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        r["documents"][kind] = json!({"kind":kind,"revision":1,"path":path,"sha256":hash,"content":content,"submittedAt":Utc::now().to_rfc3339(),"gameVersion":null,"artifactBindings":{}});
        r["approvals"].as_array_mut().unwrap().push(json!({"kind":kind,"documentRevision":1,"documentHash":hash,"decision":"accepted","feedback":"controlled fixture","decidedAt":Utc::now().to_rfc3339()}));
        r["stages"][kind] = json!("passed");
    }
    db.execute(
        "UPDATE task_production SET body=?2 WHERE task_id=?1",
        rusqlite::params![f.task, r.to_string()],
    )
    .unwrap();
}
#[test]
fn native_completion_continues_with_platform_issue_and_recheck_without_human_resume() {
    let f = Fixture::new();
    seed(&f);
    f.mutate("save-issue",json!({"id":"pb","stage":"implementation","kind":"blocker","status":"open","description":"platform limitation","repairTarget":"abya-source","resumeWhen":"new build"}));
    finish_auto_fixture(&f, "first", true);
    f.service
        .auto_complete_locked(&f.task, &f.conversation, "first", true)
        .unwrap();
    assert_eq!(f.calls.lock().len(), 1);
    let prompt = f.calls.lock()[0]["input"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(prompt.contains("Lua 自主执行"));
    let turn = f.turns.lock().last().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    finish_auto_fixture(&f, &turn, true);
    f.service
        .auto_complete_locked(&f.task, &f.conversation, &turn, true)
        .unwrap();
    assert_eq!(f.calls.lock().len(), 2);
    f.service
        .auto_complete_locked(&f.task, &f.conversation, &turn, true)
        .unwrap();
    assert_eq!(f.calls.lock().len(), 2);
    f.service.cancelled.lock().insert(f.conversation.clone());
    let turn = f.turns.lock().last().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    finish_auto_fixture(&f, &turn, true);
    f.service
        .auto_complete_locked(&f.task, &f.conversation, &turn, true)
        .unwrap();
    assert_eq!(f.calls.lock().len(), 2);
}
#[test]
fn dependency_wakeup_never_clears_manual_pause_or_starts_a_turn() {
    let f = Fixture::new();
    seed(&f);
    f.service.cancelled.lock().insert(f.conversation.clone());
    f.service
        .wake_owned_autonomy(&f.task, &f.conversation, f.revision())
        .unwrap();
    assert!(f.service.cancelled.lock().contains(&f.conversation));
    assert!(f.calls.lock().is_empty());
}
#[test]
fn semantic_progress_reports_are_not_unfinished_native_tools() {
    let f = Fixture::new();
    seed(&f);
    finish_auto_fixture(&f, "reported", true);
    let task = f.service.tasks.get(&f.task).unwrap();
    let dir = conversation_directory(Path::new(&task.workspace_path), &f.conversation);
    let mut workflow = load_workflow(&dir, &f.task, &f.conversation).unwrap();
    workflow.record_activity(
        "reported",
        Some("semantic"),
        CodexActivityKind::Analysis,
        CodexActivityStatus::Progress,
        "Progress summary",
        "",
        "desktopCliReport",
    );
    workflow.record_activity(
        "reported",
        Some("declared-command"),
        CodexActivityKind::Command,
        CodexActivityStatus::Started,
        "Declared milestone",
        "",
        "desktopCliReport",
    );
    save_workflow(&dir, &workflow, "semanticFixture").unwrap();
    assert!(!auto_progress::pending_tools(&workflow));
    f.service
        .auto_complete_locked(&f.task, &f.conversation, "reported", true)
        .unwrap();
    assert_eq!(f.calls.lock().len(), 1);
    workflow.record_activity(
        "reported",
        Some("real-command"),
        CodexActivityKind::Command,
        CodexActivityStatus::Started,
        "Actual running tool",
        "",
        "desktopCli",
    );
    assert!(auto_progress::pending_tools(&workflow));
}
