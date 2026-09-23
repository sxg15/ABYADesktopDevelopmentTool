use crate::foundation::{AppError, AppResult};
use crate::modules::instances::{InstanceService, RuntimeMcpEndpoint};
use parking_lot::Mutex;
use reqwest::blocking::{Client, Response};
use reqwest::header::HeaderValue;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBridgeState {
    pub instance_id: String,
    pub endpoint: String,
    pub connected: bool,
    pub runtime_instance_id: String,
    pub origin: String,
    pub game_version: String,
    pub platform: String,
    pub capabilities: Vec<String>,
    pub mcp_available: bool,
    pub server_name: String,
    pub server_version: String,
    pub instructions: String,
    pub last_error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
struct RuntimeMcpSession {
    session_id: String,
    server_name: String,
    server_version: String,
    instructions: String,
}

type RuntimeEndpointResolver = Arc<dyn Fn(&str) -> AppResult<RuntimeMcpEndpoint> + Send + Sync>;

#[derive(Clone)]
pub struct RuntimeBridgeService {
    instances: InstanceService,
    endpoint_resolver: RuntimeEndpointResolver,
    client: Client,
    sessions: Arc<Mutex<HashMap<String, RuntimeMcpSession>>>,
}

impl RuntimeBridgeService {
    pub fn new(instances: InstanceService) -> Self {
        let resolver_instances = instances.clone();
        Self {
            instances,
            endpoint_resolver: Arc::new(move |instance_id| {
                resolver_instances.runtime_mcp_endpoint(instance_id)
            }),
            client: Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .build()
                .expect("Runtime MCP HTTP client configuration is valid"),
            sessions: Default::default(),
        }
    }

    pub fn state(&self, instance_id: &str) -> RuntimeBridgeState {
        let instance = self.instances.read(instance_id).ok();
        match self.ensure_session(instance_id, Duration::from_secs(2)) {
            Ok(session) => RuntimeBridgeState {
                instance_id: instance_id.to_string(),
                endpoint: self.endpoint_for(instance_id),
                connected: true,
                runtime_instance_id: instance
                    .as_ref()
                    .and_then(|value| value.runtime_instance_id.clone())
                    .unwrap_or_default(),
                origin: "managed".into(),
                game_version: instance
                    .as_ref()
                    .map(|value| value.game_version.clone())
                    .unwrap_or_default(),
                platform: instance
                    .as_ref()
                    .map(|value| value.platform.clone())
                    .unwrap_or_default(),
                capabilities: instance
                    .as_ref()
                    .map(|value| value.runtime_capabilities.clone())
                    .unwrap_or_default(),
                mcp_available: true,
                server_name: session.server_name,
                server_version: session.server_version,
                instructions: session.instructions,
                last_error: String::new(),
            },
            Err(error) => RuntimeBridgeState {
                instance_id: instance_id.to_string(),
                endpoint: self.endpoint_for(instance_id),
                connected: false,
                runtime_instance_id: instance
                    .as_ref()
                    .and_then(|value| value.runtime_instance_id.clone())
                    .unwrap_or_default(),
                origin: instance
                    .as_ref()
                    .map(|value| value.origin.as_db().to_string())
                    .unwrap_or_default(),
                game_version: instance
                    .as_ref()
                    .map(|value| value.game_version.clone())
                    .unwrap_or_default(),
                platform: instance
                    .as_ref()
                    .map(|value| value.platform.clone())
                    .unwrap_or_default(),
                capabilities: instance
                    .as_ref()
                    .map(|value| value.runtime_capabilities.clone())
                    .unwrap_or_default(),
                mcp_available: false,
                server_name: String::new(),
                server_version: String::new(),
                instructions: String::new(),
                last_error: error.message,
            },
        }
    }

    pub fn list_tools(&self, instance_id: &str) -> AppResult<Value> {
        let result = self.rpc(
            instance_id,
            "tools/list",
            json!({}),
            Duration::from_secs(30),
        )?;
        Ok(result
            .pointer("/result/tools")
            .or_else(|| result.get("tools"))
            .cloned()
            .unwrap_or(result))
    }

    pub fn call_tool(
        &self,
        instance_id: &str,
        name: &str,
        arguments: Value,
    ) -> AppResult<RuntimeToolResult> {
        let response = self.rpc(
            instance_id,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
            Duration::from_secs(600),
        )?;
        let result = response.get("result").cloned().unwrap_or(response);
        let content = result
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "gameMcpProtocol",
                    "Game MCP returned no content array.",
                    result.to_string(),
                )
            })?;
        Ok(RuntimeToolResult {
            content,
            is_error: result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    pub fn wait_for_mcp(
        &self,
        instance_id: &str,
        timeout: Duration,
    ) -> AppResult<RuntimeBridgeState> {
        let started = Instant::now();
        let mut last_error = String::new();
        while started.elapsed() < timeout {
            match self.ensure_session(instance_id, Duration::from_secs(2)) {
                Ok(_) => return Ok(self.state(instance_id)),
                Err(error) => last_error = error.to_string(),
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(AppError::new(
            "gameMcpTimeout",
            "The game Runtime MCP did not become ready before timeout.",
            last_error,
        ))
    }

    pub fn forget_instance(&self, instance_id: &str) {
        self.sessions.lock().remove(instance_id);
    }

    fn endpoint_for(&self, instance_id: &str) -> String {
        (self.endpoint_resolver)(instance_id)
            .map(|value| value.endpoint)
            .unwrap_or_default()
    }

    fn ensure_session(&self, instance_id: &str, timeout: Duration) -> AppResult<RuntimeMcpSession> {
        if let Some(session) = self.sessions.lock().get(instance_id).cloned() {
            return Ok(session);
        }
        let endpoint = (self.endpoint_resolver)(instance_id)?;
        let response = self
            .client
            .post(&endpoint.endpoint)
            .bearer_auth(&endpoint.token)
            .timeout(timeout)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": Uuid::new_v4().to_string(),
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "ABYA Desktop Development Tool",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }))
            .send()
            .map_err(runtime_http_error)?;
        let session_id = response
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let body = decode_response(response)?;
        if session_id.is_empty() {
            return Err(AppError::new(
                "gameMcpProtocol",
                "Runtime MCP initialize returned no session ID.",
                body.to_string(),
            ));
        }
        if let Some(error) = body.get("error") {
            return Err(AppError::new(
                "gameMcpError",
                "Runtime MCP initialize failed.",
                error.to_string(),
            ));
        }
        let session = RuntimeMcpSession {
            session_id,
            server_name: body
                .pointer("/result/serverInfo/name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            server_version: body
                .pointer("/result/serverInfo/version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            instructions: body
                .pointer("/result/instructions")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        };
        self.sessions
            .lock()
            .insert(instance_id.to_string(), session.clone());
        Ok(session)
    }

    fn rpc(
        &self,
        instance_id: &str,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> AppResult<Value> {
        let mut retried = false;
        loop {
            let endpoint = (self.endpoint_resolver)(instance_id)?;
            let session = self.ensure_session(instance_id, Duration::from_secs(5))?;
            let response = self.send_rpc(&endpoint, &session, method, &params, timeout)?;
            if response.status().as_u16() == 404 && !retried {
                self.forget_instance(instance_id);
                retried = true;
                continue;
            }
            let body = decode_response(response)?;
            if let Some(error) = body.get("error") {
                return Err(AppError::new(
                    "gameMcpError",
                    format!("Runtime MCP request failed: {method}"),
                    error.to_string(),
                ));
            }
            return Ok(body);
        }
    }

    fn send_rpc(
        &self,
        endpoint: &RuntimeMcpEndpoint,
        session: &RuntimeMcpSession,
        method: &str,
        params: &Value,
        timeout: Duration,
    ) -> AppResult<Response> {
        self.client
            .post(&endpoint.endpoint)
            .bearer_auth(&endpoint.token)
            .header(
                "Mcp-Session-Id",
                HeaderValue::from_str(&session.session_id).map_err(AppError::internal)?,
            )
            .timeout(timeout)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": Uuid::new_v4().to_string(),
                "method": method,
                "params": params
            }))
            .send()
            .map_err(runtime_http_error)
    }

    #[cfg(test)]
    fn with_endpoint_for_test(instances: InstanceService, endpoint: RuntimeMcpEndpoint) -> Self {
        let endpoint = Arc::new(endpoint);
        Self {
            instances,
            endpoint_resolver: Arc::new(move |_| Ok((*endpoint).clone())),
            client: Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            sessions: Default::default(),
        }
    }
}

fn decode_response(response: Response) -> AppResult<Value> {
    let status = response.status();
    let body = response.text().map_err(runtime_http_error)?;
    if !status.is_success() {
        return Err(AppError::new(
            "gameMcpHttp",
            format!("Runtime MCP returned HTTP {}.", status.as_u16()),
            body,
        ));
    }
    serde_json::from_str(&body).map_err(|error| {
        AppError::new(
            "gameMcpProtocol",
            "Runtime MCP returned invalid JSON.",
            error.to_string(),
        )
    })
}

fn runtime_http_error(error: reqwest::Error) -> AppError {
    AppError::new(
        if error.is_timeout() {
            "gameMcpTimeout"
        } else {
            "gameMcpConnection"
        },
        if error.is_timeout() {
            "The Runtime MCP request timed out."
        } else {
            "The desktop tool could not reach the Runtime MCP endpoint."
        },
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{AppPaths, Database};
    use crate::modules::game_connections::GameConnectionService;
    use axum::{
        Json, Router,
        http::{HeaderMap, HeaderValue, StatusCode},
        response::{IntoResponse, Response},
        routing::post,
    };
    use std::net::TcpListener;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    #[test]
    fn disconnected_state_is_non_secret_and_explicit() {
        let root = std::env::temp_dir().join(format!("abya-runtime-bridge-{}", Uuid::new_v4()));
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        std::fs::create_dir_all(&root).unwrap();
        let database = Database::open(&paths).unwrap();
        let instances = InstanceService::new(database, paths, GameConnectionService::new());
        let state = RuntimeBridgeService::new(instances).state("missing");
        assert!(!state.connected);
        assert!(!state.mcp_available);
        assert!(state.endpoint.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn authenticated_http_runtime_mcp_preserves_image_content() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        tauri::async_runtime::spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let router = Router::new().route("/mcp", post(fake_runtime_mcp));
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });
        std::thread::sleep(Duration::from_millis(50));

        let root =
            std::env::temp_dir().join(format!("abya-runtime-bridge-http-{}", Uuid::new_v4()));
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        std::fs::create_dir_all(&root).unwrap();
        let database = Database::open(&paths).unwrap();
        let instances = InstanceService::new(database, paths, GameConnectionService::new());
        let service = RuntimeBridgeService::with_endpoint_for_test(
            instances,
            RuntimeMcpEndpoint {
                endpoint: format!("http://127.0.0.1:{port}/mcp"),
                token: "runtime-test-token".into(),
            },
        );

        let tools = service.list_tools("instance").unwrap();
        assert_eq!(tools[0]["name"], "ui_capture_screenshot");
        let result = service
            .call_tool("instance", "ui_capture_screenshot", json!({}))
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content[0]["type"], "text");
        assert_eq!(result.content[1]["type"], "image");
        assert_eq!(result.content[1]["mimeType"], "image/png");
        assert_eq!(result.content[1]["data"], "iVBORw0KGgo=");

        let _ = shutdown_tx.send(());
        let _ = std::fs::remove_dir_all(root);
    }

    async fn fake_runtime_mcp(headers: HeaderMap, Json(request): Json<Value>) -> Response {
        if headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            != Some("Bearer runtime-test-token")
        {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        match request.get("method").and_then(Value::as_str) {
            Some("initialize") => {
                let mut response_headers = HeaderMap::new();
                response_headers.insert("Mcp-Session-Id", HeaderValue::from_static("session-1"));
                (
                    response_headers,
                    Json(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "protocolVersion": "2025-11-25",
                            "serverInfo": {
                                "name": "Fake Runtime MCP",
                                "version": "1"
                            },
                            "instructions": "Call read_me_first."
                        }
                    })),
                )
                    .into_response()
            }
            Some("tools/list") => Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [{ "name": "ui_capture_screenshot" }]
                }
            }))
            .into_response(),
            Some("tools/call") => Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [
                        { "type": "text", "text": "{\"width\":1,\"height\":1}" },
                        {
                            "type": "image",
                            "mimeType": "image/png",
                            "data": "iVBORw0KGgo="
                        }
                    ],
                    "isError": false
                }
            }))
            .into_response(),
            _ => StatusCode::NOT_FOUND.into_response(),
        }
    }
}
