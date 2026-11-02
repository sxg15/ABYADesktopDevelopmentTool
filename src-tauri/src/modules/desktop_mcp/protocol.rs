use super::McpLeases;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// Pin the negotiated compatibility revision; do not pretend to implement newer semantics.
const VERSION: &str = "2025-11-25";
type Dispatch = Arc<dyn Fn(Value, String, String, Value) -> Value + Send + Sync>;
type Cancel = Arc<dyn Fn(Value, String, String) + Send + Sync>;
#[derive(Clone)]
struct Pending {
    operation: String,
    context: Value,
    instance: String,
    cancelled: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}
type Active = Arc<Mutex<HashMap<String, Pending>>>;
struct Completion {
    active: Active,
    key: String,
    finished: Arc<AtomicBool>,
}
impl Drop for Completion {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::SeqCst);
        self.active.lock().remove(&self.key);
    }
}

type Authorize = Arc<dyn Fn(&str) -> Option<Value> + Send + Sync>;
#[derive(Clone)]
pub struct McpGateway {
    pub leases: McpLeases,
    catalog: Arc<Vec<Value>>,
    dispatch: Dispatch,
    authorize: Option<Authorize>,
    cancel: Option<Cancel>,
    active: Active,
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
            authorize: None,
            cancel: None,
            active: Default::default(),
        }
    }
    pub fn with_authorizer(
        mut self,
        authorize: impl Fn(&str) -> Option<Value> + Send + Sync + 'static,
    ) -> Self {
        self.authorize = Some(Arc::new(authorize));
        self
    }
    pub fn with_cancel(
        mut self,
        cancel: impl Fn(Value, String, String) + Send + Sync + 'static,
    ) -> Self {
        self.cancel = Some(Arc::new(cancel));
        self
    }
    fn context(&self, token: &str) -> Option<Value> {
        match &self.authorize {
            Some(f) => f(token),
            None => self.leases.context(token),
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
    let Some(context) = gateway.context(token) else {
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
        let Some(session) = gateway.leases.begin_authenticated(token) else {
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
    if method.starts_with("notifications/") && !id.is_null() {
        return error(id, -32600, "Notifications cannot have request IDs");
    }
    if !gateway
        .leases
        .ready_authenticated(token, session, method == "notifications/initialized")
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    if method == "notifications/initialized" && id.is_null() {
        return StatusCode::ACCEPTED.into_response();
    }
    if method == "notifications/cancelled" {
        let key = format!("{}:{}", session, request["params"]["requestId"]);
        if let Some(pending) = gateway.active.lock().get(&key) {
            pending.cancelled.store(true, Ordering::SeqCst);
        }
        return StatusCode::ACCEPTED.into_response();
    }
    if id.is_null() {
        return StatusCode::ACCEPTED.into_response();
    }
    match method {
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({"tools":gateway.catalog.as_ref()})),
        "tools/call" => {
            call(
                gateway,
                context,
                token.to_owned(),
                session.to_owned(),
                id,
                request["params"].clone(),
            )
            .await
        }
        _ => error(id, -32601, "Method not found"),
    }
}

async fn call(
    gateway: McpGateway,
    context: Value,
    token: String,
    session: String,
    id: Value,
    params: Value,
) -> Response {
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
    if name == "game_runtime_call_tool"
        && let (Some(meta), Some(argument)) = (
            params["_meta"]["abya/operationId"].as_str(),
            arguments["requestId"].as_str(),
        )
        && meta != argument
    {
        return error(id, -32602, "Conflicting operation identities");
    }
    let supplied = params["_meta"]["abya/operationId"].as_str().or_else(|| {
        if name == "game_runtime_call_tool" {
            arguments["requestId"].as_str()
        } else {
            None
        }
    });
    let operation = match supplied {
        Some(s) => match uuid::Uuid::parse_str(s) {
            Ok(id) => id.to_string(),
            Err(_) => return error(id, -32602, "Invalid operation identity"),
        },
        None => uuid::Uuid::new_v4().to_string(),
    };
    let key = format!("{session}:{id}");
    let pending = Pending {
        operation: operation.clone(),
        context: context.clone(),
        instance: if name == "game_runtime_call_tool" {
            arguments["instanceId"].as_str().unwrap_or("").into()
        } else {
            String::new()
        },
        cancelled: Arc::new(AtomicBool::new(false)),
        finished: Arc::new(AtomicBool::new(false)),
    };
    {
        let mut active = gateway.active.lock();
        if active.contains_key(&key) {
            return error(id, -32600, "Request is already in progress");
        }
        active.insert(key.clone(), pending.clone());
    }
    let watched = pending.clone();
    let cancel = gateway.cancel.clone();
    tokio::spawn(async move {
        while !watched.finished.load(Ordering::SeqCst) {
            if watched.cancelled.load(Ordering::SeqCst)
                && !watched.instance.is_empty()
                && let Some(callback) = cancel.clone()
            {
                let context = watched.context.clone();
                let instance = watched.instance.clone();
                let operation = watched.operation.clone();
                let _ = tokio::task::spawn_blocking(move || callback(context, instance, operation))
                    .await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });
    let dispatch = gateway.dispatch.clone();
    let tool = name.to_owned();
    let request_operation = operation.clone();
    let finished = pending.finished.clone();
    let active = gateway.active.clone();
    match tokio::task::spawn_blocking(move||{
        let _completion=Completion{active,key,finished};
        if pending.cancelled.load(Ordering::SeqCst)||gateway.context(&token).as_ref()!=Some(&context){
            return json!({"isError":true,"content":[{"type":"text","text":"Request cancelled or authorization revoked before dispatch; not_executed"}]});
        }
        dispatch(context,request_operation,tool,arguments)
    }).await {
        Ok(mut value)=>{
            if !value.is_object(){return error(id,-32603,"Invalid tool result");}
            value["_meta"]=json!({"abya/operationId":operation});
            if let Some(content)=value["content"].as_array_mut(){content.push(json!({"type":"text","text":json!({"abyaOperationId":operation}).to_string()}));}
            result(id,value)
        },
        Err(_)=>error(id,-32603,"Tool result unavailable; reconcile the operation before retrying"),
    }
}
