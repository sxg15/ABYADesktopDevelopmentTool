use super::McpLeases;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Value, json};
use std::sync::Arc;

// Pin the negotiated compatibility revision; do not pretend to implement newer semantics.
const VERSION: &str = "2025-11-25";
type Dispatch = Arc<dyn Fn(Value, String, String, Value) -> Value + Send + Sync>;
#[derive(Clone)]
pub struct McpGateway {
    pub leases: McpLeases,
    catalog: Arc<Vec<Value>>,
    dispatch: Dispatch,
}
impl McpGateway {
    pub fn new(
        catalog: Vec<Value>,
        dispatch: impl Fn(Value, String, String, Value) -> Value + Send + Sync + 'static,
    ) -> Self {
        Self {
            leases: McpLeases::default(),
            catalog: Arc::new(catalog),
            dispatch: Arc::new(dispatch),
        }
    }
    /// The caller must bind this router only on loopback, using the MCP edition's port.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/mcp", post(handle))
            .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
            .with_state(self.clone())
    }
}
fn result(id: Value, value: Value) -> Response {
    Json(json!({"jsonrpc":"2.0","id":id,"result":value})).into_response()
}
fn error(id: Value, code: i32, message: &str) -> Response {
    Json(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})).into_response()
}
fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}
pub(super) async fn handle(
    State(gateway): State<McpGateway>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    if headers.contains_key("origin") {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(token) = header(&headers, "authorization").and_then(|v| v.strip_prefix("Bearer "))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(context) = gateway.leases.context(token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    if request["jsonrpc"] != "2.0"
        || !request["method"].is_string()
        || (!id.is_null() && !id.is_string() && !id.is_number())
    {
        return error(id, -32600, "Invalid JSON-RPC request");
    }
    let method = request["method"].as_str().unwrap();
    if method == "initialize" {
        if id.is_null() || !request["params"]["protocolVersion"].is_string() {
            return error(id, -32602, "A protocol version and request ID are required");
        }
        let Some(session) = gateway.leases.begin(token) else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        let mut response = result(
            id,
            json!({"protocolVersion":VERSION,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"ABYA Desktop MCP","version":env!("CARGO_PKG_VERSION")}}),
        );
        response
            .headers_mut()
            .insert("Mcp-Session-Id", HeaderValue::from_str(&session).unwrap());
        return response;
    }
    let Some(session) = header(&headers, "mcp-session-id") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if header(&headers, "mcp-protocol-version") != Some(VERSION) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if !gateway
        .leases
        .ready(token, session, method == "notifications/initialized")
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    if method == "notifications/initialized" && id.is_null() {
        return StatusCode::ACCEPTED.into_response();
    }
    if id.is_null() {
        return StatusCode::ACCEPTED.into_response();
    }
    match method {
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({"tools":gateway.catalog.as_ref()})),
        "tools/call" => call(gateway, context, id, request["params"].clone()).await,
        _ => error(id, -32601, "Method not found"),
    }
}

async fn call(gateway: McpGateway, context: Value, id: Value, params: Value) -> Response {
    let Some(name) = params["name"].as_str().filter(|v| !v.is_empty()) else {
        return error(id, -32602, "Tool name is required");
    };
    if !gateway.catalog.iter().any(|v| v["name"] == name) {
        return error(id, -32602, "Unknown tool");
    }
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return error(id, -32602, "Tool arguments must be an object");
    }
    let operation = match params["_meta"]["abya/operationId"].as_str() {
        Some(s) => match uuid::Uuid::parse_str(s) {
            Ok(id) => id.to_string(),
            Err(_) => return error(id, -32602, "Invalid operation identity"),
        },
        None => uuid::Uuid::new_v4().to_string(),
    };
    let dispatch = gateway.dispatch.clone();
    let tool = name.to_owned();
    let request_operation = operation.clone();
    match tokio::task::spawn_blocking(move || dispatch(context, request_operation, tool, arguments))
        .await
    {
        Ok(mut value) => {
            if !value.is_object() {
                return error(id, -32603, "Invalid tool result");
            }
            value["_meta"] = json!({"abya/operationId":operation});
            result(id, value)
        }
        Err(_) => error(
            id,
            -32603,
            "Tool result unavailable; reconcile the operation before retrying",
        ),
    }
}
