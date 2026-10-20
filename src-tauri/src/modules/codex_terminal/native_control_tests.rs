use super::*;

struct NativeCleanup(Vec<(AppServerHandle, String)>);
impl Drop for NativeCleanup {
    fn drop(&mut self) {
        let metadata = std::env::var("ABYA_TEST_CODEX").ok().and_then(|path| {
            AppServerHandle::start(
                &ResolvedCodex {
                    path: path.into(),
                    kind: CodexLaunchKind::Executable,
                },
                Arc::new(|_| {}),
            )
            .ok()
        });
        for (server, thread) in &self.0 {
            if server.is_alive() {
                let _ = server.interrupt_and_wait(thread);
            }
            server.stop();
            if let Some(metadata) = &metadata {
                let _ = metadata.request("thread/archive", json!({"threadId":thread}));
            }
        }
        if let Some(metadata) = metadata {
            metadata.stop();
        }
    }
}

fn connect(f: &mut Fixture, cleanup: &mut NativeCleanup) -> AppServerHandle {
    let server = AppServerHandle::start(
        &ResolvedCodex {
            path: std::env::var("ABYA_TEST_CODEX")
                .expect("Set ABYA_TEST_CODEX")
                .into(),
            kind: CodexLaunchKind::Executable,
        },
        Arc::new(|_| {}),
    )
    .unwrap();
    let cwd = f.service.tasks.get(&f.task).unwrap().workspace_path;
    let result = server.request("thread/start",json!({"cwd":cwd,"approvalPolicy":"never","sandbox":"read-only",
        "developerInstructions":"Isolated cancellation test. Do not use any tools or modify files."})).unwrap();
    let native = result["thread"]["id"].as_str().unwrap().to_string();
    cleanup.0.push((server.clone(), native.clone()));
    server.request("thread/inject_items",json!({"threadId":native,"items":[
        {"type":"message","role":"developer","content":[{"type":"input_text","text":"Isolated cancellation test. No tools."}]}
    ]})).unwrap();
    let mut c = f.service.conversation(&f.task, &f.conversation).unwrap();
    c.native_session_id = Some(native.clone());
    write_conversation(
        &conversation_directory(Path::new(&cwd), &f.conversation),
        &c,
    )
    .unwrap();
    f.service.thread_conversations.lock().remove(&f.native);
    f.service
        .thread_conversations
        .lock()
        .insert(native.clone(), (f.task.clone(), f.conversation.clone()));
    f.native = native;
    f.service
        .runtime_servers
        .lock()
        .insert(f.conversation.clone(), server.clone());
    server
}

fn start(server: &AppServerHandle, thread: &str) -> String {
    server
        .request(
            "turn/start",
            json!({"threadId":thread,"input":[{"type":"text",
        "text":"Think carefully and produce a full rigorous proof of the prime number theorem."}]}),
        )
        .unwrap()["turn"]["id"]
        .as_str()
        .unwrap()
        .into()
}

#[test]
#[ignore = "Real Codex inference in isolated conversations; set ABYA_TEST_CODEX and run alone"]
fn real_dual_connection_interrupt_pause_force_and_independent_runtime() {
    let mut cleanup = NativeCleanup(Vec::new());
    let mut f = Fixture::new();
    let mut other = Fixture::new();
    let server = connect(&mut f, &mut cleanup);
    let independent = connect(&mut other, &mut cleanup);
    let tui = server.second_connection_for_test();
    tui.request(
        "thread/resume",
        json!({"threadId":f.native,"excludeTurns":true}),
    )
    .unwrap();
    let first = start(&server, &f.native);
    f.service.interrupt_turn(&f.task, &f.conversation).unwrap();
    let second = start(&tui, &f.native);
    assert_ne!(first, second);
    f.service.interrupt_turn(&f.task, &f.conversation).unwrap();
    assert!(f.service.session(&f.conversation).is_ok());
    assert!(server.is_alive());
    let state = server
        .request(
            "thread/read",
            json!({"threadId":f.native,"includeTurns":true}),
        )
        .unwrap();
    assert_eq!(
        state["thread"]["turns"].as_array().unwrap().last().unwrap()["status"],
        "interrupted"
    );
    start(&tui, &f.native);
    f.service.stop(&f.conversation).unwrap();
    assert!(!server.is_alive());
    assert!(f.service.session(&f.conversation).is_err());
    assert!(independent.is_alive());
    let mut third = Fixture::new();
    let untouched = connect(&mut third, &mut cleanup);
    let untouched_turn = start(&untouched, &third.native);
    let forced_turn = start(&independent, &other.native);
    let saved_native = other.native.clone();
    other
        .service
        .force_stop(&other.task, &other.conversation)
        .unwrap();
    assert!(!independent.is_alive());
    assert!(untouched.is_alive());
    assert_eq!(
        other
            .service
            .native_session_id(&other.task, &other.conversation)
            .unwrap(),
        Some(saved_native)
    );
    let state = untouched
        .request(
            "thread/read",
            json!({"threadId":third.native,"includeTurns":true}),
        )
        .unwrap();
    assert_eq!(
        state["thread"]["turns"].as_array().unwrap().last().unwrap()["id"],
        untouched_turn
    );
    assert_ne!(
        state["thread"]["turns"].as_array().unwrap().last().unwrap()["status"],
        "interrupted"
    );
    assert!(!forced_turn.is_empty());
    println!(
        "native: automatic/manual interrupt acknowledged; pause/force exited; independent active turn unaffected; original identity retained"
    );
}
