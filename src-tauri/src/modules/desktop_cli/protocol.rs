use super::{registry, tools::DesktopToolDispatcher};
use axum::{
    Json,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone)]
pub(super) struct HttpState {
    token: String,
    dispatcher: DesktopToolDispatcher,
    seen: std::sync::Arc<parking_lot::Mutex<std::collections::HashSet<String>>>,
}
impl HttpState {
    pub(super) fn new(token: String, dispatcher: DesktopToolDispatcher) -> Self {
        Self {
            token,
            dispatcher,
            seen: Default::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CommandRequest {
    version: u32,
    request_id: String,
    command: String,
    #[serde(default = "empty")]
    arguments: Value,
    #[serde(default)]
    context: Option<Value>,
}
fn empty() -> Value {
    json!({})
}
fn error(status: StatusCode, code: &str) -> Response {
    (
        status,
        Json(json!({"schemaVersion":1,"success":false,"error":{"code":code}})),
    )
        .into_response()
}
fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let expected = format!("Bearer {token}");
    let actual = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let mut diff = actual.len() ^ expected.len();
    for (i, value) in expected.bytes().enumerate() {
        diff |= usize::from(value ^ actual.as_bytes().get(i).copied().unwrap_or(0));
    }
    diff == 0
}
pub(super) fn body_limit() -> DefaultBodyLimit {
    DefaultBodyLimit::max(4 * 1024 * 1024)
}

pub(super) async fn handle_command(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(input): Json<CommandRequest>,
) -> Response {
    if headers.contains_key("origin") {
        return error(StatusCode::FORBIDDEN, "invalid_origin");
    }
    if !authorized(&headers, &state.token) {
        return error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    if input.version != 1
        || uuid::Uuid::parse_str(&input.request_id).is_err()
        || !input.arguments.is_object()
    {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    {
        let mut seen = state.seen.lock();
        if seen.contains(&input.request_id) {
            return error(StatusCode::CONFLICT, "duplicate_request");
        }
        if seen.len() >= 100_000 {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "request_history_full_restart_required",
            );
        }
        seen.insert(input.request_id.clone());
    }
    let request_id = input.request_id;
    let response_id = request_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        if input.command == "capabilities" {
            return json!({"schemaVersion":1,"requestId":request_id,"success":true,"data":registry::tools()});
        }
        if input.command == "doctor" {
            let paths = crate::foundation::cli_environment::runtime_paths();
            return json!({"schemaVersion":1,"requestId":request_id,"success":paths.is_ok(),
                "data":{"protocolVersion":1,"runtimeCliInstalled":paths.is_ok()},
                "error": paths.err()});
        }
        let result = state.dispatcher.call_with_context(input.context, &request_id, &input.command, input.arguments);
        let data = result.content.first().and_then(|v|v.get("text")).and_then(Value::as_str)
            .and_then(|v|serde_json::from_str::<Value>(v).ok());
        json!({"schemaVersion":1,"requestId":request_id,"success":!result.is_error,
            "data":data,"content":result.content})
    }).await;
    match result {
        Ok(value) => Json(value).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"schemaVersion":1,
            "requestId":response_id,"success":false,"error":{"code":"execution_failed"}})),
        )
            .into_response(),
    }
}
