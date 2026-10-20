use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn newest_turn_wins_over_stale_history_and_old_observation() {
    let snapshot = json!({"thread":{"id":"thread","turns":[
        {"id":"old","status":"inProgress"},{"id":"current","status":"inProgress"}
    ]}});
    assert_eq!(
        target(&snapshot, "thread", None).unwrap().as_deref(),
        Some("current")
    );
    assert_eq!(
        target(
            &snapshot,
            "thread",
            Some(("old".into(), "inProgress".into()))
        )
        .unwrap()
        .as_deref(),
        Some("current")
    );
    let idle = json!({"thread":{"id":"thread","turns":[{"id":"done","status":"completed"}]}});
    assert_eq!(target(&idle, "thread", None).unwrap(), None);
    assert!(target(&idle, "foreign", None).is_err());
    assert!(
        target(
            &json!({"thread":{"id":"thread","status":{"type":"active"},"turns":[]}}),
            "thread",
            None
        )
        .is_err()
    );
    assert!(
        target(
            &json!({"thread":{"id":"thread","status":{"type":"notLoaded"},"turns":[]}}),
            "thread",
            None
        )
        .is_err()
    );
}

#[test]
fn live_event_supplies_new_turn_when_read_lags_and_old_completion_cannot_replace_it() {
    let turns = parking_lot::Mutex::new(HashMap::new());
    for (method, id, status) in [
        ("turn/started", "new", "inProgress"),
        ("turn/completed", "old", "completed"),
    ] {
        observe(
            &turns,
            &json!({"method":method,"params":{"threadId":"thread","turn":{"id":id,"status":status}}}),
        );
    }
    let snapshot = json!({"thread":{"id":"thread","turns":[{"id":"old","status":"completed"}]}});
    assert_eq!(
        target(&snapshot, "thread", turns.lock().get("thread").cloned())
            .unwrap()
            .as_deref(),
        Some("new")
    );
}

#[test]
fn changed_turn_refreshes_once_and_confirms_exact_replacement() {
    let phase = Arc::new(AtomicUsize::new(0));
    let p = phase.clone();
    let server = AppServerHandle::for_rpc_test(move |method, params| {
        let phase = p.load(Ordering::SeqCst);
        match method {
            "thread/read" => Ok(json!({"thread":{"id":"thread","turns":[
                {"id":if phase == 0 {"old"} else {"new"},"status":if phase < 2 {"inProgress"} else {"interrupted"}}
            ]}})),
            "turn/interrupt" if phase == 0 => {
                assert_eq!(params["turnId"], "old");
                p.store(1, Ordering::SeqCst);
                Err(
                    json!({"code":-32600,"message":"expected active turn id old but found new"})
                        .to_string(),
                )
            }
            "turn/interrupt" => {
                assert_eq!(params["turnId"], "new");
                p.store(2, Ordering::SeqCst);
                Ok(json!({}))
            }
            _ => panic!("unexpected request"),
        }
    });
    server.interrupt_and_wait("thread").unwrap();
    assert_eq!(phase.load(Ordering::SeqCst), 2);
}

#[test]
fn arbitrary_rejection_is_not_retried_or_reported_as_preparation_failure() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let server = AppServerHandle::for_rpc_test(move |method, _| {
        if method == "thread/read" {
            Ok(json!({"thread":{"id":"thread","turns":[{"id":"active","status":"inProgress"}]}}))
        } else {
            c.fetch_add(1, Ordering::SeqCst);
            Err(json!({"code":-32600,"message":"private-secret unknown rejection"}).to_string())
        }
    });
    let error = server.interrupt_and_wait("thread").unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(error.message.contains("中断"));
    assert!(!error.to_string().contains("private-secret"));
}

#[cfg(windows)]
#[test]
fn owned_job_fixture() {
    if std::env::var_os("ABYA_JOB_FIXTURE_SLEEP").is_some() {
        std::thread::sleep(Duration::from_secs(60));
    } else if let Some(path) = std::env::var_os("ABYA_JOB_FIXTURE_PID") {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "modules::codex_terminal::app_server::native_interrupt::tests::owned_job_fixture",
                "--nocapture",
            ])
            .env_remove("ABYA_JOB_FIXTURE_PID")
            .env("ABYA_JOB_FIXTURE_SLEEP", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            super::super::super::CREATE_NO_WINDOW,
        );
        let mut child = command.spawn().unwrap();
        fs::write(path, child.id().to_string()).unwrap();
        child.wait().unwrap();
    }
}

#[test]
fn accepted_interrupt_without_terminal_ack_has_a_finite_deadline() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let server = AppServerHandle::for_rpc_test(move |method, _| {
        if method == "thread/read" {
            Ok(json!({"thread":{"id":"thread","turns":[{"id":"busy","status":"inProgress"}]}}))
        } else {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(json!({}))
        }
    });
    let started = Instant::now();
    let error = server.interrupt_and_wait("thread").unwrap_err();
    assert_eq!(error.code, "codex_interrupt_timeout");
    assert!(started.elapsed() < Duration::from_secs(23));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[cfg(windows)]
#[test]
fn force_stop_owned_job_terminates_descendant_and_preserves_unrelated_job() {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    let root = std::env::temp_dir().join(format!("abya-stop-job-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let pid_file = root.join("child.pid");
    let fixture = "modules::codex_terminal::app_server::native_interrupt::tests::owned_job_fixture";
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", fixture, "--nocapture"])
        .env("ABYA_JOB_FIXTURE_PID", &pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            fs::File::create(root.join("stderr.log")).unwrap(),
        ));
    std::os::windows::process::CommandExt::creation_flags(
        &mut command,
        super::super::super::CREATE_NO_WINDOW,
    );
    let mut job = CommandWrap::from(command);
    job.wrap(CreationFlags(
        windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(
            super::super::super::CREATE_NO_WINDOW,
        ),
    ));
    job.wrap(JobObject);
    let child = job.spawn().unwrap();
    let handle = AppServerHandle::for_test("unused", "");
    *handle.child.lock() = Some(child);
    let mut other_command = Command::new(std::env::current_exe().unwrap());
    other_command
        .args(["--exact", fixture, "--nocapture"])
        .env("ABYA_JOB_FIXTURE_SLEEP", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    std::os::windows::process::CommandExt::creation_flags(
        &mut other_command,
        super::super::super::CREATE_NO_WINDOW,
    );
    let mut other_job = CommandWrap::from(other_command);
    other_job.wrap(CreationFlags(
        windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(
            super::super::super::CREATE_NO_WINDOW,
        ),
    ));
    other_job.wrap(JobObject);
    let other = AppServerHandle::for_test("unused", "");
    *other.child.lock() = Some(other_job.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pid_file.exists() {
        assert!(
            Instant::now() < deadline,
            "{}",
            fs::read_to_string(root.join("stderr.log")).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let pid: u32 = fs::read_to_string(&pid_file).unwrap().parse().unwrap();
    let child_handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    assert!(!child_handle.is_null());
    handle.stop_confirmed().unwrap();
    assert_eq!(
        unsafe { WaitForSingleObject(child_handle, 3000) },
        0,
        "descendant remained alive"
    );
    unsafe { CloseHandle(child_handle) };
    assert!(other.is_alive(), "unrelated job was killed");
    other.stop_confirmed().unwrap();
    fs::remove_file(pid_file).unwrap();
    fs::remove_file(root.join("stderr.log")).unwrap();
    fs::remove_dir(root).unwrap();
}
