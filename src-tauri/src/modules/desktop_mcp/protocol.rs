use super::registry;
use super::tools::DesktopToolDispatcher;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

#[derive(Clone)]
pub(super) struct HttpState {
    token: String,
    sessions: Arc<Mutex<HashSet<String>>>,
    dispatcher: DesktopToolDispatcher,
}

impl HttpState {
    pub(super) fn new(token: String, dispatcher: DesktopToolDispatcher) -> Self {
        Self {
            token,
            sessions: Default::default(),
            dispatcher,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default = "empty_arguments")]
    arguments: Value,
}

fn empty_arguments() -> Value {
    json!({})
}

pub(super) async fn handle_mcp(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    if !authorized(&headers, &state.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Bearer authentication is required."})),
        )
            .into_response();
    }
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if method != "initialize" && !valid_session(&headers, &state) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "MCP session is missing or expired."})),
        )
            .into_response();
    }
    match method {
        "initialize" => initialize(id, &state),
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({ "tools": registry::tools() })),
        "tools/call" => {
            let session_id = session_id(&headers);
            let params = match serde_json::from_value::<ToolCallParams>(
                request.get("params").cloned().unwrap_or(Value::Null),
            ) {
                Ok(params) if !params.name.trim().is_empty() => params,
                Ok(_) => return rpc_error(id, -32602, "Tool name is required."),
                Err(error) => return rpc_error(id, -32602, &format!("Invalid tool call: {error}")),
            };
            let dispatcher = state.dispatcher.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                dispatcher.call_for_session(session_id.as_deref(), &params.name, params.arguments)
            })
            .await;
            match result {
                Ok(result) => rpc_result(id, json!(result)),
                Err(error) => rpc_error(id, -32603, &format!("Tool execution failed: {error}")),
            }
        }
        _ => rpc_error(id, -32601, "Method not found."),
    }
}

fn initialize(id: Value, state: &HttpState) -> Response {
    let session_id = Uuid::new_v4().to_string();
    state.sessions.lock().insert(session_id.clone());
    let mut response_headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&session_id) {
        response_headers.insert("Mcp-Session-Id", value);
    }
    (
        StatusCode::OK,
        response_headers,
        Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": "ABYA Desktop Development Tool",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        })),
    )
        .into_response()
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == format!("Bearer {token}"))
}

fn valid_session(headers: &HeaderMap, state: &HttpState) -> bool {
    session_id(headers).is_some_and(|value| state.sessions.lock().contains(&value))
}

fn session_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Mcp-Session-Id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn rpc_result(id: Value, result: Value) -> Response {
    Json(json!({"jsonrpc": "2.0", "id": id, "result": result})).into_response()
}

fn rpc_error(id: Value, code: i32, message: &str) -> Response {
    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_authentication_requires_exact_token() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer expected".parse().unwrap());
        assert!(authorized(&headers, "expected"));
        assert!(!authorized(&headers, "different"));
    }
}
