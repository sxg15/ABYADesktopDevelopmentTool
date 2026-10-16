use super::*;
use crate::foundation::notifications::PlatformNotificationStatus;
use crate::modules::development_terminal::{ConversationAttention, TerminalProvider};
use crate::modules::tasks::ProductionView;
use rusqlite::params;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Default)]
struct FakeBackend {
    sent: Mutex<Vec<ToastMessage>>,
    removed: Mutex<Vec<String>>,
    unavailable: AtomicBool,
    fail: AtomicBool,
}
impl NotificationBackend for FakeBackend {
    fn status(&self) -> PlatformNotificationStatus {
        PlatformNotificationStatus {
            available: !self.unavailable.load(Ordering::SeqCst),
            reason: String::new(),
            identity: "test".into(),
            test_environment: true,
        }
    }
    fn show(&self, message: ToastMessage) -> AppResult<bool> {
        self.sent.lock().push(message);
        if self.fail.load(Ordering::SeqCst) {
            Err(AppError::validation("unknown delivery"))
        } else {
            Ok(true)
        }
    }
    fn remove(&self, key: &str) -> AppResult<()> {
        self.removed.lock().push(key.into());
        Ok(())
    }
    fn clear(&self) -> AppResult<()> {
        Ok(())
    }
    fn shutdown(&self) {}
}

struct Fixture {
    root: PathBuf,
    db: Database,
    service: TaskNotificationService,
    backend: Arc<FakeBackend>,
    foreground: Arc<AtomicBool>,
    activation: mpsc::Sender<String>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("abya-notification-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let db = Database::open(&paths).unwrap();
        let tasks = TaskService::new(db.clone(), paths.default_workspace_root.clone());
        let backend = Arc::new(FakeBackend::default());
        let foreground = Arc::new(AtomicBool::new(false));
        let flag = foreground.clone();
        let (activation, receiver) = mpsc::channel();
        let service = TaskNotificationService(Arc::new(Inner {
            codex: CodexTerminalService::new(tasks.clone()).unwrap(),
            grok: GrokTerminalService::new(tasks.clone()).unwrap(),
            tasks,
            settings: SettingsService::new(db.clone(), paths.default_workspace_root),
            store: NotificationStore(db.clone()),
            backend: backend.clone(),
            foreground: Arc::new(move || flag.load(Ordering::SeqCst)),
            state: Default::default(),
            scan: Default::default(),
            stopped: AtomicBool::new(false),
            worker: Default::default(),
            activation_input: Mutex::new(receiver),
            activations: Default::default(),
            changed: Arc::new(|_| {}),
            activated: Arc::new(|| {}),
            last_error: Default::default(),
        }));
        Self {
            root,
            db,
            service,
            backend,
            foreground,
            activation,
        }
    }
    fn task(&self, id: &str) {
        self.db.with_connection(|c| {
            c.execute("INSERT INTO development_tasks(id,title,status,created_at,updated_at,workspace_path) VALUES(?1,?2,'active','now','now',?3)",
                params![id,format!("翻卡片 {id}"),self.root.join(id).to_string_lossy()])?;
            Ok(())
        }).unwrap();
    }
    fn record(&self, id: &str, question: &str) -> Value {
        json!({"schema":"abya.game-development-workflow/v1","taskId":id,"revision":1,
            "workflowVersion":"2.1.0","policy":{"workflowVersion":"2.1.0","stages":[{"id":"requirements","name":"需求","approval":"requirements"}]},
            "questionMode":"ask","currentStage":"requirements","currentRound":0,"cycle":1,
            "stages":{"requirements":"in-progress"},"documents":{},"approvals":[],"versionDetails":{},
            "issues":[],"evidence":[],"rounds":[],"milestones":{},"checks":[],"knowledge":[],"skillPins":[],"updatedAt":"now",
            "questionGroups":[{"id":question,"title":"玩法","provider":"grok","conversationId":"existing","cycle":1,
                "questions":[{"id":"q1","text":"选择颜色"}],"draft":{},"answers":[],"status":"pending","publishedAt":"now","updatedAt":"now"}]})
    }
    fn save(&self, id: &str, record: &Value) {
        self.db.with_connection(|c| {
            c.execute("INSERT INTO task_production(task_id,revision,body) VALUES(?1,1,?2) ON CONFLICT(task_id) DO UPDATE SET body=excluded.body",
                params![id,record.to_string()])?; Ok(())
        }).unwrap();
    }
    fn refresh(&self) {
        self.service.refresh().unwrap();
    }
    fn activate(&self, key: &str) -> NotificationActivation {
        self.activation.send(key.into()).unwrap();
        self.service.process_activations();
        self.service.take_activations().pop().unwrap()
    }
    fn count(&self) -> usize {
        self.backend.sent.lock().len()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // SQLite can still hold its handles; cleanup is best effort and confined to this UUID fixture.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn baseline_foreground_and_disabled_items_do_not_reappear_on_blur() {
    let f = Fixture::new();
    f.task("old");
    f.save("old", &f.record("old", "q0"));
    f.refresh();
    assert_eq!(f.count(), 0);
    f.foreground.store(true, Ordering::SeqCst);
    f.save("old", &f.record("old", "q1"));
    f.refresh();
    f.foreground.store(false, Ordering::SeqCst);
    f.refresh();
    assert_eq!(f.count(), 0);
    f.service.set_enabled(false).unwrap();
    f.save("old", &f.record("old", "q2"));
    f.refresh();
    f.service.set_enabled(true).unwrap();
    f.refresh();
    assert_eq!(f.count(), 0);
    f.save("old", &f.record("old", "q3"));
    f.refresh();
    f.refresh();
    assert_eq!(f.count(), 1);
    assert!(
        !f.root.join("old").exists(),
        "read-only scans must not create workspaces or sync Skills"
    );
}

#[test]
fn all_tasks_are_observed_and_drafts_polling_and_restart_keep_one_receipt() {
    let f = Fixture::new();
    f.refresh();
    for id in ["unselected-a", "unselected-b"] {
        f.task(id);
        f.save(id, &f.record(id, "q"));
    }
    f.refresh();
    assert_eq!(f.count(), 2);
    let mut r = f.record("unselected-a", "q");
    r["revision"] = json!(9);
    r["updatedAt"] = json!("later");
    r["questionGroups"][0]["draft"] = json!({"q1":"Red"});
    f.save("unselected-a", &r);
    f.refresh();
    assert_eq!(f.count(), 2);
    *f.service.0.state.lock() = MonitorState::default();
    f.refresh();
    f.refresh();
    assert_eq!(f.count(), 2);
    // A later answer revision represents a new round, including after the restart baseline.
    r["questionGroups"][0]["answers"] =
        json!([{"revision":1,"answers":{"q1":"Red"},"submittedAt":"later"}]);
    f.save("unselected-a", &r);
    f.refresh();
    assert_eq!(f.count(), 3);
}

#[test]
fn unreadable_state_preserves_notice_until_resolved_and_clicks_only_navigate() {
    let f = Fixture::new();
    f.refresh();
    f.task("a");
    let mut r = f.record("a", "q");
    f.save("a", &r);
    f.refresh();
    let key = f.backend.sent.lock()[0].key.clone();
    assert_eq!(f.activate(&key).status, "current");
    assert_eq!(
        f.service
            .0
            .tasks
            .production_attention(&f.service.0.tasks.get_readonly("a").unwrap())
            .unwrap()
            .record
            .unwrap()
            .question_groups[0]
            .status,
        "pending"
    );
    f.service.0.store.delivered(&key, true).unwrap();
    f.backend.removed.lock().clear();
    f.db.with_connection(|c| {
        c.execute("UPDATE task_production SET body='broken'", [])?;
        Ok(())
    })
    .unwrap();
    f.refresh();
    assert!(f.service.snapshot("a").unavailable);
    assert!(f.backend.removed.lock().is_empty());
    assert_eq!(f.count(), 1);
    assert_eq!(f.activate(&key).status, "unavailable");
    r["questionGroups"][0]["status"] = json!("answered");
    f.save("a", &r);
    f.refresh();
    assert_eq!(f.activate(&key).status, "resolved");
    f.db.with_connection(|c| {
        c.execute("DELETE FROM development_tasks WHERE id='a'", [])?;
        Ok(())
    })
    .unwrap();
    f.refresh();
    assert_eq!(f.activate(&key).status, "missing");
    assert!(!f.root.join("a").exists());
}

#[test]
fn failed_delivery_os_disabled_and_deleted_tasks_never_replay() {
    let f = Fixture::new();
    f.refresh();
    f.task("a");
    f.backend.fail.store(true, Ordering::SeqCst);
    f.save("a", &f.record("a", "q1"));
    f.refresh();
    f.refresh();
    assert_eq!(f.count(), 1);
    f.backend.fail.store(false, Ordering::SeqCst);
    f.refresh();
    assert_eq!(f.count(), 1);
    f.backend.unavailable.store(true, Ordering::SeqCst);
    f.save("a", &f.record("a", "q2"));
    f.refresh();
    f.backend.unavailable.store(false, Ordering::SeqCst);
    f.refresh();
    assert_eq!(f.count(), 1);
    f.save("a", &f.record("a", "q3"));
    f.refresh();
    assert_eq!(f.count(), 2);
    let key = f.backend.sent.lock()[1].key.clone();
    f.db.with_connection(|c| {
        c.execute("DELETE FROM development_tasks WHERE id='a'", [])?;
        Ok(())
    })
    .unwrap();
    f.refresh();
    assert!(f.backend.removed.lock().contains(&key));
}

#[test]
fn projection_prioritizes_human_actions_and_keys_versions_not_timestamps() {
    let f = Fixture::new();
    f.task("a");
    let task = f.service.0.tasks.get_readonly("a").unwrap();
    let mut r = f.record("a", "q");
    r["documents"] = json!({"requirements":{"kind":"requirements","revision":2,"path":"a.md","sha256":"hash","content":"draft","submittedAt":"now"}});
    let mut view = ProductionView {
        art_design_status: serde_json::Value::Null,
        actions: json!({"review":{"requirements":""}}),
        stage_statuses: Default::default(),
        artifacts: vec![],
        policy: r["policy"].clone(),
        record: Some(serde_json::from_value(r).unwrap()),
        warnings: vec![],
        report_paths: vec![],
        available_update: false,
    };
    let mut terminal = ConversationAttention {
        provider: TerminalProvider::Codex,
        task_id: "a".into(),
        conversation_id: "original".into(),
        state: "waitingForTerminal".into(),
        turn_id: Some("turn".into()),
        request_id: None,
        interaction_id: Some("wait1".into()),
    };
    let first = projection::project(&task, &view, &[terminal.clone()]);
    assert_eq!(
        first
            .items
            .iter()
            .map(|i| i.kind.as_str())
            .collect::<Vec<_>>(),
        vec!["terminal", "question", "review"]
    );
    terminal.interaction_id = Some("wait2".into());
    let second = projection::project(&task, &view, &[terminal]);
    assert_ne!(first.items[0].key, second.items[0].key);
    assert_eq!(first.items[1].key, second.items[1].key);
    let doc = view
        .record
        .as_mut()
        .unwrap()
        .documents
        .get_mut("requirements")
        .unwrap();
    doc.revision += 1;
    doc.sha256 = "next".into();
    assert_ne!(
        first.items[2].key,
        projection::project(&task, &view, &[]).items[1].key
    );
    view.actions["review"]["requirements"] = json!("文件变化");
    assert_eq!(projection::project(&task, &view, &[]).items.len(), 1);
    let mut closed = task.clone();
    closed.status = TaskStatus::Archived;
    assert!(projection::project(&closed, &view, &[]).items.is_empty());
}

#[test]
fn unknown_paused_normal_completion_and_running_never_create_waits() {
    let f = Fixture::new();
    f.task("a");
    let task = f.service.0.tasks.get_readonly("a").unwrap();
    let view = f.service.0.tasks.production_attention(&task).unwrap();
    for state in [
        "unknown",
        "paused",
        "ready",
        "running",
        "waitingForResponse",
        "awaitingContinue",
    ] {
        let c = ConversationAttention {
            provider: TerminalProvider::Grok,
            task_id: "a".into(),
            conversation_id: "c".into(),
            state: state.into(),
            turn_id: None,
            request_id: None,
            interaction_id: None,
        };
        assert!(projection::project(&task, &view, &[c]).items.is_empty());
    }
    assert!(!f.root.join("a").exists());
}
