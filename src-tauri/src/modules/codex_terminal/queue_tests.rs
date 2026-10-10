use super::intake_receipt::IntakeContinueStatus as S;
use super::*;
use crate::foundation::{AppPaths, Database};
use crate::modules::tasks::{ProductionMutation, TaskInput};
use serde_json::Value;

#[derive(Debug)]
struct Killer;
impl ChildKiller for Killer {
    fn kill(&mut self) -> std::io::Result<()> {
        Ok(())
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(Killer)
    }
}
type StartGate = Arc<Mutex<Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>>>;
struct Fixture {
    service: CodexTerminalService,
    task: String,
    conversation: String,
    native: String,
    root: PathBuf,
    turns: Arc<Mutex<Vec<Value>>>,
    calls: Arc<Mutex<Vec<Value>>>,
    start_gate: StartGate,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("abya-queue-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("db.sqlite"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfer"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let tasks = TaskService::new(
            Database::open(&paths).unwrap(),
            paths.default_workspace_root,
        );
        let task = tasks
            .create(TaskInput {
                title: "Queue controlled fixture".into(),
                description: String::new(),
            })
            .unwrap();
        tasks
            .production_update(ProductionMutation {
                task_id: task.id.clone(),
                expected_revision: 0,
                operation: "initialize".into(),
                data: json!({"questionMode":"ask","playerMode":"single"}),
            })
            .unwrap();
        let service = CodexTerminalService::new(tasks).unwrap();
        let mut c = service.create_conversation(&task.id, None).unwrap();
        let native = Uuid::new_v4().to_string();
        c.native_session_id = Some(native.clone());
        let dir = conversation_directory(Path::new(&task.workspace_path), &c.id);
        write_conversation(&dir, &c).unwrap();
        let turns = Arc::new(Mutex::new(vec![]));
        let calls = Arc::new(Mutex::new(vec![]));
        let start_gate: StartGate = Default::default();
        let gate = start_gate.clone();
        let t = turns.clone();
        let events = calls.clone();
        let n = native.clone();
        let server = AppServerHandle::for_rpc_test(move |method, params| match method {
            "thread/read" => Ok(json!({"thread":{"id":n,"turns":t.lock().clone()}})),
            "turn/start" => {
                if let Some((entered, release)) = gate.lock().take() {
                    let _ = entered.send(());
                    release.recv_timeout(Duration::from_secs(5)).unwrap();
                }
                events.lock().push(params);
                let turn = json!({"id":Uuid::new_v4().to_string(),"status":"inProgress"});
                t.lock().push(turn.clone());
                Ok(json!({"turn":turn}))
            }
            "turn/interrupt" => {
                for x in t.lock().iter_mut() {
                    if x["status"] == "inProgress" {
                        x["status"] = json!("interrupted");
                    }
                }
                Ok(json!({}))
            }
            _ => Err(format!("Unexpected RPC {method}")),
        });
        service.runtime_servers.lock().insert(c.id.clone(), server);
        service
            .thread_conversations
            .lock()
            .insert(native.clone(), (task.id.clone(), c.id.clone()));
        let pair = native_pty_system().openpty(pty_size(80, 24)).unwrap();
        service.sessions.lock().insert(
            c.id.clone(),
            LiveTerminal {
                task_id: task.id.clone(),
                session_id: Uuid::new_v4().to_string(),
                state: Arc::new(Mutex::new(CodexTerminalState {
                    task_id: task.id.clone(),
                    conversation_id: c.id.clone(),
                    status: CodexTerminalStatus::Running,
                    pid: None,
                    working_directory: task.workspace_path,
                    exit_code: None,
                    last_error: String::new(),
                })),
                master: Arc::new(Mutex::new(pair.master)),
                writer: Arc::new(Mutex::new(Box::new(std::io::sink()))),
                transcript: Arc::new(Mutex::new(
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(dir.join("transcript.log"))
                        .unwrap(),
                )),
                killer: Arc::new(Mutex::new(Box::new(Killer))),
                subscriber: Default::default(),
            },
        );
        Self {
            service,
            task: task.id,
            conversation: c.id,
            native,
            root,
            turns,
            calls,
            start_gate,
        }
    }
    fn revision(&self) -> u64 {
        self.service
            .tasks
            .production_get(&self.task)
            .unwrap()
            .record
            .unwrap()
            .revision
    }
    fn mutate(&self, operation: &str, data: Value) {
        self.service
            .tasks
            .production_update(ProductionMutation {
                task_id: self.task.clone(),
                expected_revision: self.revision(),
                operation: operation.into(),
                data,
            })
            .unwrap();
    }
    fn question(&self, id: &str) {
        self.mutate("publish-questions",json!({"id":id,"title":"one fast question","provider":"codex","conversationId":self.conversation,
        "nativeSessionId":self.native,"questions":[{"id":"mode","text":"Choose?","options":["one","two"]}]}));
    }
    fn answer(&self, id: &str) -> QuestionSubmission {
        self.service
            .submit_questions(ProductionMutation {
                task_id: self.task.clone(),
                expected_revision: self.revision(),
                operation: "submit-answers".into(),
                data: json!({"id":id,"answers":{"mode":"one"}}),
            })
            .unwrap()
    }
    fn busy(&self, id: &str) {
        self.turns
            .lock()
            .push(json!({"id":id,"status":"inProgress"}));
    }
    fn complete(&self) {
        self.turns.lock().last_mut().unwrap()["status"] = json!("completed");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.service.stop_all();
        let resolved = self.root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        let _ = std::fs::remove_dir_all(resolved);
    }
}

#[test]
fn document_decision_queues_while_plain_continue_does_not_interrupt() {
    let f = Fixture::new();
    f.busy("document-publisher");
    assert_eq!(
        f.service
            .continue_after_revision(&f.task, &f.conversation, None)
            .unwrap()
            .status,
        S::Running
    );
    let revision = f.revision();
    let q = f
        .service
        .continue_after_revision(&f.task, &f.conversation, Some(revision))
        .unwrap();
    assert_eq!(q.status, S::Queued);
    assert!(f.calls.lock().is_empty());
    assert_eq!(
        q.request_id,
        f.service
            .continue_after_revision(&f.task, &f.conversation, Some(revision))
            .unwrap()
            .request_id
    );
    f.complete();
    assert_eq!(
        f.service
            .flush_continuation(&f.task, &f.conversation, q.request_id.as_ref().unwrap())
            .unwrap()
            .status,
        S::Started
    );
    assert_eq!(f.calls.lock().len(), 1);
    assert_eq!(
        f.service
            .flush_continuation(&f.task, &f.conversation, q.request_id.as_ref().unwrap())
            .unwrap()
            .status,
        S::Running
    );
    assert_eq!(f.calls.lock().len(), 1);
}

#[test]
fn ten_fast_answers_wait_for_native_completion_and_dispatch_once() {
    let f = Fixture::new();
    for i in 0..10 {
        let group = format!("group-{i}");
        let publisher = format!("publisher-{i}");
        f.busy(&publisher);
        f.question(&group);
        let queued = f.answer(&group).continuation.unwrap();
        assert_eq!(queued.status, S::Queued);
        assert_eq!(f.calls.lock().len(), i);
        let duplicate = f.service.continue_questions(&f.task, &group, 1).unwrap();
        assert_eq!(duplicate.request_id, queued.request_id);
        assert_eq!(duplicate.status, S::Queued);
        assert_eq!(f.calls.lock().len(), i);
        f.complete();
        f.service.queue_event(&f.native, &publisher, true);
        let start = Instant::now();
        while f.calls.lock().len() != i + 1 {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        let _guard = f.service.lifecycle_lock.lock();
        assert_eq!(f.calls.lock().len(), i + 1);
        drop(_guard);
        assert_eq!(
            f.service
                .continue_questions(&f.task, &group, 1)
                .unwrap()
                .status,
            S::Running
        );
        f.complete();
        assert_eq!(
            f.service
                .continue_questions(&f.task, &group, 1)
                .unwrap()
                .status,
            S::Completed
        );
    }
}

#[test]
fn pause_cancels_queue_and_late_completion_cannot_restart_it() {
    let f = Fixture::new();
    f.busy("publisher");
    f.question("one");
    let q = f.answer("one").continuation.unwrap();
    f.service.stop(&f.conversation).unwrap();
    f.service.queue_event(&f.native, "publisher", true);
    assert_eq!(
        f.service
            .flush_continuation(&f.task, &f.conversation, q.request_id.as_ref().unwrap())
            .unwrap()
            .status,
        S::Paused
    );
    assert!(f.calls.lock().is_empty());
}

#[test]
fn newer_answer_replaces_only_unsent_version_and_restart_does_not_arm_queue() {
    let f = Fixture::new();
    f.busy("publisher");
    f.question("one");
    let first = f.answer("one").continuation.unwrap();
    f.service
        .tasks
        .production_answer(ProductionMutation {
            task_id: f.task.clone(),
            expected_revision: f.revision(),
            operation: "amend".into(),
            data: json!({"id":"one"}),
        })
        .unwrap();
    let second = f.answer("one").continuation.unwrap();
    assert_ne!(first.request_id, second.request_id);
    let restarted = CodexTerminalService::new(f.service.tasks.clone()).unwrap();
    assert_eq!(
        restarted
            .flush_continuation(
                &f.task,
                &f.conversation,
                second.request_id.as_ref().unwrap()
            )
            .unwrap()
            .status,
        S::Paused
    );
    f.complete();
    f.service
        .flush_continuation(
            &f.task,
            &f.conversation,
            second.request_id.as_ref().unwrap(),
        )
        .unwrap();
    assert_eq!(f.calls.lock().len(), 1);
    assert!(
        f.calls.lock()[0]["input"][0]["text"]
            .as_str()
            .unwrap()
            .contains("第 2 版")
    );
}

#[test]
fn next_question_or_document_wait_prevents_redundant_inference() {
    let f = Fixture::new();
    f.busy("publisher");
    f.question("one");
    let q = f.answer("one").continuation.unwrap();
    f.question("two");
    f.complete();
    assert_eq!(
        f.service
            .flush_continuation(&f.task, &f.conversation, q.request_id.as_ref().unwrap())
            .unwrap()
            .status,
        S::WaitingForAnswers
    );
    assert!(f.calls.lock().is_empty());
}

#[test]
fn pause_racing_with_dispatch_finishes_interrupted_without_a_second_turn() {
    let f = Fixture::new();
    f.question("race");
    let (entered, ready) = std::sync::mpsc::channel();
    let (release, gate) = std::sync::mpsc::channel();
    *f.start_gate.lock() = Some((entered, gate));
    let service = f.service.clone();
    let task = f.task.clone();
    let revision = f.revision();
    let submit = std::thread::spawn(move || {
        service.submit_questions(ProductionMutation {
            task_id: task,
            expected_revision: revision,
            operation: "submit-answers".into(),
            data: json!({"id":"race","answers":{"mode":"one"}}),
        })
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let service = f.service.clone();
    let conversation = f.conversation.clone();
    let pause = std::thread::spawn(move || service.stop(&conversation));
    let deadline = Instant::now();
    while !f.service.cancelled.lock().contains(&f.conversation) {
        assert!(deadline.elapsed() < Duration::from_secs(3));
        std::thread::yield_now();
    }
    release.send(()).unwrap();
    submit.join().unwrap().unwrap();
    pause.join().unwrap().unwrap();
    assert_eq!(f.calls.lock().len(), 1);
    assert_eq!(f.turns.lock().last().unwrap()["status"], "interrupted");
}

#[test]
fn failed_producer_does_not_silently_start_a_queued_model_turn() {
    let f = Fixture::new();
    f.busy("publisher");
    f.question("one");
    let q = f.answer("one").continuation.unwrap();
    f.turns.lock().last_mut().unwrap()["status"] = json!("failed");
    f.service.queue_event(&f.native, "publisher", false);
    let deadline = Instant::now();
    loop {
        let state = f.service.task_control(&f.task, &f.conversation).unwrap();
        if state.state == "paused" {
            break;
        }
        assert!(deadline.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        f.service
            .flush_continuation(&f.task, &f.conversation, q.request_id.as_ref().unwrap())
            .unwrap()
            .status,
        S::Paused
    );
    assert!(f.calls.lock().is_empty());
}
