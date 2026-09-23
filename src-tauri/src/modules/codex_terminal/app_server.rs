use super::{CodexLaunchKind, ResolvedCodex};
use crate::foundation::{AppError, AppResult};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, connect};
use uuid::Uuid;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const START_TIMEOUT: Duration = Duration::from_secs(10);

type NotificationHandler = Arc<dyn Fn(Value) + Send + Sync>;

enum BridgeCommand {
    Request {
        method: String,
        params: Value,
        response: Sender<Result<Value, String>>,
    },
    Notification {
        method: String,
        params: Value,
    },
    Shutdown,
}

#[derive(Clone)]
pub(super) struct AppServerHandle {
    pub endpoint: String,
    pub token: String,
    sender: Sender<BridgeCommand>,
    child: Arc<parking_lot::Mutex<Option<Child>>>,
    token_file: Arc<PathBuf>,
}

impl AppServerHandle {
    pub(super) fn start(
        codex: &ResolvedCodex,
        on_notification: NotificationHandler,
    ) -> AppResult<Self> {
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
        std::thread::spawn(move || {
            run_bridge(&thread_endpoint, &thread_token, receiver, on_notification);
        });

        let handle = Self {
            endpoint,
            token,
            sender,
            child,
            token_file: Arc::new(token_file),
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
    ) -> AppResult<String> {
        let result = self.request(
            "thread/start",
            json!({
                "cwd": cwd,
                "developerInstructions": developer_instructions,
                "experimentalRawEvents": false
            }),
        )?;
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
    ) -> AppResult<()> {
        self.request(
            "thread/resume",
            json!({
                "threadId": thread_id,
                "cwd": cwd,
                "developerInstructions": developer_instructions,
                "excludeTurns": true
            }),
        )?;
        Ok(())
    }

    pub(super) fn stop(&self) {
        let _ = self.sender.send(BridgeCommand::Shutdown);
        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(self.token_file.as_path());
    }

    fn request(&self, method: &str, params: Value) -> AppResult<Value> {
        let (response, receiver) = mpsc::channel();
        self.sender
            .send(BridgeCommand::Request {
                method: method.to_string(),
                params,
                response,
            })
            .map_err(AppError::internal)?;
        receiver
            .recv_timeout(REQUEST_TIMEOUT)
            .map_err(|_| AppError::internal("Timed out waiting for Codex app-server."))?
            .map_err(AppError::internal)
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
    pub(super) fn for_test(endpoint: &str, token: &str) -> Self {
        let (sender, _receiver) = mpsc::channel();
        Self {
            endpoint: endpoint.to_string(),
            token: token.to_string(),
            sender,
            child: Arc::new(parking_lot::Mutex::new(None)),
            token_file: Arc::new(PathBuf::new()),
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
    command.status().is_ok_and(|status| status.success())
}

fn spawn_app_server(
    codex: &ResolvedCodex,
    endpoint: &str,
    token_file: &PathBuf,
) -> AppResult<Child> {
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
    command.spawn().map_err(AppError::internal)
}

fn codex_process_command(codex: &ResolvedCodex) -> Command {
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
    let Ok((mut socket, _)) = connect(request) else {
        fail_pending(receiver, "Could not connect to Codex app-server.");
        return;
    };
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    }

    let mut next_id = 1_u64;
    let mut pending = HashMap::<u64, Sender<Result<Value, String>>>::new();
    loop {
        while let Ok(command) = receiver.try_recv() {
            match command {
                BridgeCommand::Request {
                    method,
                    params,
                    response,
                } => {
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
                if let Some(id) = value.get("id").and_then(Value::as_u64)
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
            Err(_) => break,
        }
    }

    for (_, response) in pending {
        let _ = response.send(Err("Codex app-server connection was closed.".into()));
    }
    fail_pending(receiver, "Codex app-server connection was closed.");
}

fn fail_pending(receiver: Receiver<BridgeCommand>, message: &str) {
    while let Ok(command) = receiver.try_recv() {
        if let BridgeCommand::Request { response, .. } = command {
            let _ = response.send(Err(message.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::developer_instruction_item;

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
