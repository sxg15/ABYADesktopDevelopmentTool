use crate::foundation::{AppError, AppResult, cli_environment};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

fn drain(mut reader: impl Read, limit: usize) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chunk = [0u8; 8192];
    while let Ok(count) = reader.read(&mut chunk) {
        if count == 0 {
            break;
        }
        let retained = count.min(limit.saturating_sub(result.len()));
        result.extend_from_slice(&chunk[..retained]);
    }
    result
}
fn command(pid: u32, id: &str, session: &str, args: &[&str]) -> AppResult<Command> {
    let (node, script) = cli_environment::runtime_paths()?;
    let mut command = Command::new(node);
    command
        .arg(script)
        .args(args)
        .args(["--instance", &pid.to_string(), "--json"])
        .env("ABYA_CLI_SESSION_ID", session)
        .env("ABYA_CLI_DESKTOP_INSTANCE_ID", id)
        .env_remove("ABYA_CLI_REQUEST_ID")
        .env_remove("ABYA_CLI_TOKEN")
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .env_remove("ABYA_CLI_INSTANCE_DIR")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        // 隔离 APP 数据不改变 Unity 的 OS 私有注册目录；仍按 PID 和启动身份认证。
        if cfg!(test) || std::env::var("ABYA_TEST_MODE").as_deref() == Ok("1") {
            use windows::Win32::{
                System::Com::CoTaskMemFree,
                UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
            };
            unsafe {
                let folder = SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None)
                    .map_err(AppError::internal)?;
                let text = folder.to_string();
                CoTaskMemFree(Some(folder.0.cast()));
                command.env(
                    "ABYA_CLI_INSTANCE_DIR",
                    std::path::PathBuf::from(text.map_err(AppError::internal)?)
                        .join("AbyaPB/Cli/instances"),
                );
            }
        }
    }
    Ok(command)
}
fn cancel_runtime(pid: u32, id: &str, session: &str, operation: &str) {
    let Ok(mut command) = command(pid, id, session, &["cancel", operation]) else {
        return;
    };
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return;
    };
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}
#[allow(clippy::too_many_arguments)]
pub(super) fn execute(
    pid: u32,
    id: &str,
    args: &[&str],
    input: Option<Value>,
    session: &str,
    operation: &str,
    output: &Path,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
    memory_pressure: &dyn Fn() -> bool,
) -> AppResult<Value> {
    let mut command = command(pid, id, session, args)?;
    command
        .env("ABYA_CLI_REQUEST_ID", operation)
        .arg("--output-dir")
        .arg(output);
    if input.is_some() {
        command.args(["--input-file", "-"]);
    }
    let mut child = command.spawn().map_err(|_| {
        AppError::validation("Cannot start bundled Abya CLI; check Node and CLI installation.")
    })?;
    let stdin = child.stdin.take();
    let writer = thread::spawn(move || {
        if let (Some(mut stdin), Some(value)) = (stdin, input) {
            let _ = stdin.write_all(value.to_string().as_bytes());
        }
    });
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let reader = thread::spawn(move || drain(stdout, 16 * 1024 * 1024 + 1));
    let diagnostic = thread::spawn(move || drain(stderr, 64 * 1024));
    let start = Instant::now();
    let mut unknown = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => {
                unknown = true;
                break None;
            }
        }
        if cancelled.load(Ordering::SeqCst) || start.elapsed() >= timeout || memory_pressure() {
            unknown = true;
            if args.first() != Some(&"cancel") {
                cancel_runtime(pid, id, session, operation);
            }
            break None;
        }
        thread::sleep(Duration::from_millis(25));
    };
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = writer.join();
    let raw = reader.join().unwrap_or_default();
    let _ = diagnostic.join(); // Never persist raw output or credentials in workflow events.
    if unknown {
        return Err(AppError::new(
            "outcome_unknown",
            "CLI operation was interrupted; verify game state before retrying.",
            operation,
        ));
    }
    if raw.len() > 16 * 1024 * 1024 {
        return Err(AppError::validation("CLI output exceeded 16 MiB."));
    }
    let value: Value = serde_json::from_slice(&raw)
        .map_err(|_| AppError::validation("CLI returned invalid JSON."))?;
    if value
        .get("requestId")
        .is_some_and(|id| id.as_str() != Some(operation))
    {
        return Err(AppError::validation("CLI response identity mismatch."));
    }
    if !status.is_some_and(|s| s.success()) && value.get("content").is_none() {
        return Err(AppError::new(
            "runtimeCliFailed",
            value["message"].as_str().unwrap_or("Runtime CLI failed."),
            if value["executionState"] == "not_executed" {
                "not_executed"
            } else {
                value["errorCode"].as_str().unwrap_or("")
            },
        ));
    }
    Ok(value)
}
