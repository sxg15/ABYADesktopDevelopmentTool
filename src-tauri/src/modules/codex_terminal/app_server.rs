#[path = "native_interrupt.rs"]
mod native_interrupt;
use super::{CodexLaunchKind, ResolvedCodex};
use crate::foundation::{AppError, AppResult};
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tungstenite::Message;
use tungstenite::client::{IntoClientRequest, connect_with_config};
use tungstenite::error::CapacityError;
use tungstenite::http::HeaderValue;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::stream::MaybeTlsStream;
use uuid::Uuid;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const START_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_NATIVE_RESPONSE_BYTES: usize = 128 * 1024 * 1024;
const RESPONSE_TOO_LARGE: &str = "Codex app-server response exceeds the 128 MiB limit.";

type NotificationHandler = Arc<dyn Fn(Value) + Send + Sync>;

enum BridgeCommand {
    Request {
        method: String,
        params: Value,
        response: Sender<Result<Value, String>>,
        expires: Instant,
    },
    Notification {
        method: String,
        params: Value,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutableStamp {
    path: PathBuf,
    length: u64,
    modified: std::time::SystemTime,
}
impl ExecutableStamp {
    fn read(path: &std::path::Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        Some(Self {
            path: path.to_path_buf(),
            length: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}

#[derive(Clone)]
pub(super) struct AppServerHandle {
    pub endpoint: String,
    pub token: String,
    sender: Sender<BridgeCommand>,
    child: Arc<parking_lot::Mutex<Option<Box<dyn ChildWrapper>>>>,
    observed_turns: Arc<parking_lot::Mutex<HashMap<String, (String, String)>>>,
    token_file: Arc<PathBuf>,
    executable_stamp: Option<ExecutableStamp>,
    #[cfg(test)]
    test_alive: bool,
}

impl AppServerHandle {
    pub(super) fn ensure_idle(&self, thread: &str) -> AppResult<()> {
        let state = self.request(
            "thread/read",
            json!({"threadId":thread,"includeTurns":true}),
        )?;
        if state["thread"]["turns"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["status"] == "inProgress")
        {
            return Err(AppError::validation(
                "原会话仍在后台执行，请先暂停 AI 再修复连接。",
            ));
        }
        Ok(())
    }
    pub(super) fn start(
        codex: &ResolvedCodex,
        on_notification: NotificationHandler,
    ) -> AppResult<Self> {
        let executable_stamp = ExecutableStamp::read(&codex.path)
            .ok_or_else(|| AppError::validation("Codex executable is unavailable."))?;
        let port = reserve_loopback_port()?;
        let endpoint = format!("ws://127.0.0.1:{port}");
        let token = format!("abya-{}", Uuid::new_v4());
        let token_file =
            std::env::temp_dir().join(format!("abya-codex-app-server-{}.token", Uuid::new_v4()));
        fs::write(&token_file, &token)?;

        let child = spawn_app_server(codex, &endpoint, &token_file)?;
        let child = Arc::new(parking_lot::Mutex::new(Some(child)));
        if let Err(error) = wait_until_ready(port) {
            if let Some(mut process) = child.lock().take() {
                let _ = process.kill();
                let _ = process.wait();
            }
            let _ = fs::remove_file(&token_file);
            return Err(error);
        }

        let (sender, receiver) = mpsc::channel();
        let thread_endpoint = endpoint.clone();
        let thread_token = token.clone();
        let observed_turns = Arc::new(parking_lot::Mutex::new(HashMap::new()));
        let observations = observed_turns.clone();
        std::thread::spawn(move || {
            run_bridge(
                &thread_endpoint,
                &thread_token,
                receiver,
                Arc::new(move |event| {
                    native_interrupt::observe(&observations, &event);
                    on_notification(event);
                }),
            );
        });

        let handle = Self {
            endpoint,
            token,
            sender,
            child,
            observed_turns,
            token_file: Arc::new(token_file),
            executable_stamp: Some(executable_stamp),
            #[cfg(test)]
            test_alive: false,
        };
        if let Err(error) = handle.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "ABYA Desktop Development Tool",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        ) {
            handle.stop();
            return Err(error);
        }
        if let Err(error) = handle.notify("initialized", json!({})) {
            handle.stop();
            return Err(error);
        }
        Ok(handle)
    }

    pub(super) fn start_thread(
        &self,
        cwd: &str,
        developer_instructions: &str,
        task_id: &str,
        conversation_id: &str,
    ) -> AppResult<String> {
        let result = self.request(
            "thread/start",
            json!({
                "cwd": cwd,
                "developerInstructions": developer_instructions,
                "config": managed_thread_config(cwd, task_id, conversation_id),
                "experimentalRawEvents": false
            }),
        )?;
        let _ = super::settings::save(
            &super::conversation_directory(Path::new(cwd), conversation_id),
            &result,
        );
        result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| AppError::internal("Codex app-server did not return a thread ID."))
    }

    pub(super) fn persist_developer_instructions(
        &self,
        thread_id: &str,
        developer_instructions: &str,
    ) -> AppResult<()> {
        self.request(
            "thread/inject_items",
            json!({
                "threadId": thread_id,
                "items": [developer_instruction_item(developer_instructions)]
            }),
        )?;
        Ok(())
    }

    pub(super) fn resume_thread(
        &self,
        thread_id: &str,
        cwd: &str,
        developer_instructions: &str,
        task_id: &str,
        conversation_id: &str,
    ) -> AppResult<()> {
        let settings_directory = super::conversation_directory(Path::new(cwd), conversation_id);
        let saved_settings = super::settings::read(&settings_directory)?;
        let current = self.request(
            "thread/read",
            json!({"threadId":thread_id,"includeTurns":true}),
        )?;
        if current["thread"]["turns"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["status"] == "inProgress")
        {
            return Err(AppError::validation(
                "该会话仍在后台执行。请先暂停 AI，再继续或修复连接。",
            ));
        }
        // Resume of an already-loaded thread can retain its previous environment.
        self.ensure_idle(thread_id)?;
        let resumed = self.request(
            "thread/resume",
            json!({
                "threadId": thread_id,
                "cwd": cwd,
                "developerInstructions": developer_instructions,
                "config": managed_thread_config(cwd, task_id, conversation_id),
                "excludeTurns": true
            }),
        )?;
        if resumed["thread"]["id"].as_str() != Some(thread_id) {
            return Err(AppError::validation(
                "Codex 返回了不同的会话 ID，已阻止继续。",
            ));
        }
        if saved_settings["model"].is_string() {
            self.request(
                "thread/settings/update",
                super::settings::params(thread_id, &saved_settings),
            )?;
            super::settings::save(&settings_directory, &saved_settings)?;
        } else {
            super::settings::save(&settings_directory, &resumed)?;
        }
        Ok(())
    }

    pub(super) fn is_current(&self, codex: &ResolvedCodex) -> bool {
        if self.executable_stamp.is_none()
            || self.executable_stamp != ExecutableStamp::read(&codex.path)
        {
            return false;
        }
        self.is_alive()
    }

    pub(super) fn is_alive(&self) -> bool {
        #[cfg(test)]
        if self.test_alive {
            return true;
        }
        self.child
            .lock()
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }

    pub(super) fn stop(&self) {
        let _ = self.stop_confirmed();
    }

    pub(super) fn stop_confirmed(&self) -> AppResult<()> {
        // Kill through the owned job handle, never a process-name or recycled PID lookup.
        let mut slot = self.child.lock();
        if let Some(process) = slot.as_mut() {
            process.start_kill().map_err(AppError::internal)?;
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if process.try_wait().map_err(AppError::internal)?.is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(AppError::new(
                        "codex_stop_timeout",
                        "执行器尚未确认退出，请重试强制停止。",
                        "",
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        slot.take();
        let _ = self.sender.send(BridgeCommand::Shutdown);
        let _ = fs::remove_file(self.token_file.as_path());
        Ok(())
    }

    pub(super) fn pid(&self) -> Option<u32> {
        self.child.lock().as_ref().map(|child| child.id())
    }

    pub(super) fn request(&self, method: &str, params: Value) -> AppResult<Value> {
        self.request_with_timeout(method, params, REQUEST_TIMEOUT)
    }

    fn request_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> AppResult<Value> {
        let (response, receiver) = mpsc::channel();
        self.sender
            .send(BridgeCommand::Request {
                method: method.to_string(),
                params,
                response,
                expires: Instant::now() + timeout,
            })
            .map_err(|_| super::resume_failure::rpc_error(method, "connection channel closed"))?;
        receiver
            .recv_timeout(timeout)
            .map_err(|error| {
                super::resume_failure::rpc_error(
                    method,
                    match error {
                        mpsc::RecvTimeoutError::Timeout => {
                            "Timed out waiting for Codex app-server."
                        }
                        mpsc::RecvTimeoutError::Disconnected => {
                            "Codex app-server connection closed."
                        }
                    },
                )
            })?
            .map_err(|raw| super::resume_failure::rpc_error(method, &raw))
    }

    fn notify(&self, method: &str, params: Value) -> AppResult<()> {
        self.sender
            .send(BridgeCommand::Notification {
                method: method.to_string(),
                params,
            })
            .map_err(AppError::internal)
    }

    #[cfg(test)]
    pub(super) fn second_connection_for_test(&self) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut handle = self.clone();
        handle.sender = sender;
        let endpoint = self.endpoint.clone();
        let token = self.token.clone();
        let observed = self.observed_turns.clone();
        std::thread::spawn(move || {
            run_bridge(
                &endpoint,
                &token,
                receiver,
                Arc::new(move |event| {
                    native_interrupt::observe(&observed, &event);
                }),
            );
        });
        handle
            .request(
                "initialize",
                json!({"clientInfo":{"name":"abya_stop_test","version":"1"},
            "capabilities":{"experimentalApi":true}}),
            )
            .unwrap();
        handle.notify("initialized", json!({})).unwrap();
        handle
    }

    #[cfg(test)]
    pub(super) fn for_test(endpoint: &str, token: &str) -> Self {
        let (sender, _receiver) = mpsc::channel();
        Self {
            endpoint: endpoint.to_string(),
            token: token.to_string(),
            sender,
            child: Arc::new(parking_lot::Mutex::new(None)),
            observed_turns: Default::default(),
            token_file: Arc::new(PathBuf::new()),
            executable_stamp: None,
            test_alive: false,
        }
    }
    #[cfg(test)]
    pub(super) fn for_rpc_test(
        handler: impl Fn(&str, Value) -> Result<Value, String> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    BridgeCommand::Request {
                        method,
                        params,
                        response,
                        ..
                    } => {
                        let _ = response.send(handler(&method, params));
                    }
                    BridgeCommand::Shutdown => break,
                    _ => {}
                }
            }
        });
        Self {
            endpoint: "test".into(),
            token: String::new(),
            sender,
            child: Arc::new(parking_lot::Mutex::new(None)),
            observed_turns: Default::default(),
            token_file: Arc::new(PathBuf::new()),
            executable_stamp: None,
            test_alive: true,
        }
    }
}

pub(super) fn supports_app_server(codex: &ResolvedCodex) -> bool {
    let mut command = codex_process_command(codex);
    command
        .args(["app-server", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, super::CREATE_NO_WINDOW);
    command.status().is_ok_and(|status| status.success())
}

fn spawn_app_server(
    codex: &ResolvedCodex,
    endpoint: &str,
    token_file: &PathBuf,
) -> AppResult<Box<dyn ChildWrapper>> {
    let mut command = codex_process_command(codex);
    command
        .arg("app-server")
        .args(["--listen", endpoint])
        .args(["--ws-auth", "capability-token"])
        .arg("--ws-token-file")
        .arg(token_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, super::CREATE_NO_WINDOW);
    let mut owned = CommandWrap::from(command);
    owned.wrap(CreationFlags(
        windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(super::CREATE_NO_WINDOW),
    ));
    owned.wrap(JobObject);
    owned.spawn().map_err(AppError::internal)
}

pub(super) fn codex_process_command(codex: &ResolvedCodex) -> Command {
    match codex.kind {
        CodexLaunchKind::Executable => Command::new(&codex.path),
        CodexLaunchKind::CommandScript => {
            let comspec = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
            let mut command = Command::new(comspec);
            command.args(["/D", "/S", "/C", "call"]);
            command.arg(&codex.path);
            command
        }
    }
}

// Per-thread values must reach the process that executes tools, not only the remote TUI.
// Dotted overrides preserve unrelated user environment entries and permission settings.
fn managed_thread_config(cwd: &str, task_id: &str, conversation_id: &str) -> Value {
    let cli = crate::foundation::cli_environment::desktop_cli_path()
        .to_string_lossy()
        .into_owned();
    let values = [
        ("ABYA_DESKTOP_CLI", cli.as_str()),
        ("ABYA_DEVELOPMENT_TASK_ID", task_id),
        ("ABYA_DEVELOPMENT_CONVERSATION_ID", conversation_id),
        ("ABYA_DEVELOPMENT_PROVIDER", "codex"),
        ("ABYA_DEVELOPMENT_WORKSPACE", cwd),
    ];
    Value::Object(
        values
            .into_iter()
            .map(|(key, value)| {
                (
                    format!("shell_environment_policy.set.{key}"),
                    Value::String(value.into()),
                )
            })
            .chain(
                crate::foundation::cli_sessions::environment("codex", task_id, conversation_id)
                    .into_iter()
                    .map(|(key, value)| {
                        (
                            format!("shell_environment_policy.set.{key}"),
                            Value::String(value),
                        )
                    }),
            )
            .collect(),
    )
}

fn developer_instruction_item(developer_instructions: &str) -> Value {
    json!({
        "type": "message",
        "role": "developer",
        "content": [{
            "type": "input_text",
            "text": developer_instructions
        }]
    })
}

fn reserve_loopback_port() -> AppResult<u16> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
    Ok(listener.local_addr()?.port())
}

fn wait_until_ready(port: u16) -> AppResult<()> {
    let client = reqwest::blocking::Client::builder()
        // 就绪检查只访问本机 app-server，不经过代理。
        .no_proxy()
        .timeout(Duration::from_millis(500))
        .build()
        .map_err(AppError::internal)?;
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if client
            .get(format!("http://127.0.0.1:{port}/readyz"))
            .send()
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(AppError::internal(
        "Codex app-server did not become ready in time.",
    ))
}

fn run_bridge(
    endpoint: &str,
    token: &str,
    receiver: Receiver<BridgeCommand>,
    on_notification: NotificationHandler,
) {
    let Ok(mut request) = endpoint.into_client_request() else {
        fail_pending(receiver, "Invalid Codex app-server endpoint.");
        return;
    };
    let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}")) else {
        fail_pending(receiver, "Invalid Codex app-server authentication token.");
        return;
    };
    request.headers_mut().insert("Authorization", header);
    // Native history may arrive in one frame; bound both frames and reassembled messages.
    let config = WebSocketConfig::default()
        .max_frame_size(Some(MAX_NATIVE_RESPONSE_BYTES))
        .max_message_size(Some(MAX_NATIVE_RESPONSE_BYTES));
    let Ok((mut socket, _)) = connect_with_config(request, Some(config), 0) else {
        fail_pending(receiver, "Could not connect to Codex app-server.");
        return;
    };
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    }

    let mut next_id = 1_u64;
    let mut pending = HashMap::<u64, Sender<Result<Value, String>>>::new();
    let mut failure = "Codex app-server connection was closed.";
    loop {
        while let Ok(command) = receiver.try_recv() {
            match command {
                BridgeCommand::Request {
                    method,
                    params,
                    response,
                    expires,
                } => {
                    if Instant::now() >= expires {
                        let _ = response.send(Err("Codex request timeout before dispatch.".into()));
                        continue;
                    }
                    let id = next_id;
                    next_id += 1;
                    let message = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": method,
                        "params": params
                    });
                    if socket
                        .send(Message::Text(message.to_string().into()))
                        .is_err()
                    {
                        let _ = response.send(Err("Codex app-server connection was lost.".into()));
                    } else {
                        pending.insert(id, response);
                    }
                }
                BridgeCommand::Notification { method, params } => {
                    let message = json!({
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": params
                    });
                    let _ = socket.send(Message::Text(message.to_string().into()));
                }
                BridgeCommand::Shutdown => {
                    let _ = socket.close(None);
                    return;
                }
            }
        }

        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if value.get("method").is_none()
                    && let Some(id) = value.get("id").and_then(Value::as_u64)
                    && let Some(response) = pending.remove(&id)
                {
                    if let Some(error) = value.get("error") {
                        let _ = response.send(Err(error.to_string()));
                    } else {
                        let _ =
                            response.send(Ok(value.get("result").cloned().unwrap_or(Value::Null)));
                    }
                } else if value.get("method").is_some() {
                    on_notification(value);
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(tungstenite::Error::ConnectionClosed) => break,
            Err(tungstenite::Error::Capacity(CapacityError::MessageTooLong { .. })) => {
                failure = RESPONSE_TOO_LARGE;
                break;
            }
            Err(_) => break,
        }
    }

    for (_, response) in pending {
        let _ = response.send(Err(failure.into()));
    }
    fail_pending(receiver, failure);
}

fn fail_pending(receiver: Receiver<BridgeCommand>, message: &str) {
    while let Ok(command) = receiver.try_recv() {
        if let BridgeCommand::Request { response, .. } = command {
            let _ = response.send(Err(message.to_string()));
        }
    }
}

#[cfg(test)]
#[path = "app_server_transport_tests.rs"]
mod transport_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_codex_at_the_same_path_invalidates_the_backend_stamp() {
        let file = std::env::temp_dir().join(format!("abya-codex-stamp-{}", Uuid::new_v4()));
        fs::write(&file, b"old binary").unwrap();
        let before = ExecutableStamp::read(&file).unwrap();
        assert_eq!(Some(before.clone()), ExecutableStamp::read(&file));
        fs::write(&file, b"updated binary with new model support").unwrap();
        assert_ne!(Some(before), ExecutableStamp::read(&file));
        fs::remove_file(&file).unwrap();
        assert_eq!(None, ExecutableStamp::read(&file));
    }

    #[test]
    fn backend_without_a_live_process_is_not_reused() {
        let file = std::env::temp_dir().join(format!("abya-codex-dead-{}", Uuid::new_v4()));
        fs::write(&file, b"binary").unwrap();
        let mut handle = AppServerHandle::for_test("ws://127.0.0.1:1", "test");
        handle.executable_stamp = ExecutableStamp::read(&file);
        let codex = ResolvedCodex {
            path: file.clone(),
            kind: CodexLaunchKind::Executable,
        };
        assert!(!handle.is_current(&codex));
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn backend_context_is_isolated_per_conversation_without_permission_overrides() {
        let first = managed_thread_config("D:/work/a", "task-a", "conversation-a");
        let second = managed_thread_config("D:/work/b", "task-b", "conversation-b");
        assert_eq!(
            first["shell_environment_policy.set.ABYA_DEVELOPMENT_TASK_ID"],
            "task-a"
        );
        assert_eq!(
            first["shell_environment_policy.set.ABYA_DEVELOPMENT_CONVERSATION_ID"],
            "conversation-a"
        );
        assert_eq!(
            second["shell_environment_policy.set.ABYA_DEVELOPMENT_TASK_ID"],
            "task-b"
        );
        assert_eq!(
            second["shell_environment_policy.set.ABYA_DEVELOPMENT_CONVERSATION_ID"],
            "conversation-b"
        );
        assert!(
            first["shell_environment_policy.set.ABYA_DESKTOP_CLI"]
                .as_str()
                .unwrap()
                .ends_with("abya-desktop.exe")
        );
        assert!([5, 8].contains(&first.as_object().unwrap().len()));
        assert!(
            first
                .as_object()
                .unwrap()
                .keys()
                .all(|key| key.starts_with("shell_environment_policy.set.ABYA_"))
        );
    }

    #[test]
    fn injected_thread_item_contains_only_the_managed_developer_message() {
        let item = developer_instruction_item("Plan before tools.");
        assert_eq!(item["type"], "message");
        assert_eq!(item["role"], "developer");
        assert_eq!(item["content"][0]["type"], "input_text");
        assert_eq!(item["content"][0]["text"], "Plan before tools.");
        assert_eq!(item["content"].as_array().map(Vec::len), Some(1));
    }
}
