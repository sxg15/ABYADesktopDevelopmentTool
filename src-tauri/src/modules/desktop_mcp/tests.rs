use super::{McpGateway, protocol};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn protocol_requires_owned_initialized_session_and_does_not_dispatch_rejected_calls() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let task = uuid::Uuid::new_v4().to_string();
    let expected = task.clone();
    let gateway = McpGateway::new(
        vec![json!({"name":"probe","inputSchema":{"type":"object"}})],
        move |context, _, _, _| {
            assert_eq!(context["taskId"], expected);
            counter.fetch_add(1, Ordering::SeqCst);
            json!({"content":[{"type":"text","text":"ok"}],"isError":false})
        },
    );
    let context = json!({"provider":"codex","taskId":task,"conversationId":uuid::Uuid::new_v4()});
    let token = gateway.leases.issue(context).unwrap();
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}});
    assert_eq!(
        protocol::handle(State(gateway.clone()), HeaderMap::new(), Json(init.clone()))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    let response = protocol::handle(State(gateway.clone()), headers.clone(), Json(init)).await;
    assert_eq!(response.status(), StatusCode::OK);
    headers.insert(
        "mcp-session-id",
        response.headers()["mcp-session-id"].clone(),
    );
    headers.insert(
        "mcp-protocol-version",
        HeaderValue::from_static("2025-11-25"),
    );
    let call = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"probe","arguments":{}}});
    assert_eq!(
        protocol::handle(State(gateway.clone()), headers.clone(), Json(call.clone()))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let ready = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    assert_eq!(
        protocol::handle(State(gateway.clone()), headers.clone(), Json(ready))
            .await
            .status(),
        StatusCode::ACCEPTED
    );
    let mut foreign = headers.clone();
    foreign.insert(
        "origin",
        HeaderValue::from_static("https://example.invalid"),
    );
    assert_eq!(
        protocol::handle(State(gateway.clone()), foreign, Json(call.clone()))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        protocol::handle(State(gateway), headers, Json(call))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
