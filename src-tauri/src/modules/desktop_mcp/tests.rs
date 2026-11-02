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
    let malformed = json!({"jsonrpc":"2.0","id":3,"method":"notifications/initialized"});
    protocol::handle(State(gateway.clone()), headers.clone(), Json(malformed)).await;
    assert_eq!(
        protocol::handle(State(gateway.clone()), headers.clone(), Json(call.clone()))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
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

#[tokio::test]
async fn cancellation_is_scoped_and_reaches_the_original_operation() {
    use std::time::{Duration, Instant};
    let entered = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_cancel = cancelled.clone();
    let started = entered.clone();
    let flag = cancelled.clone();
    let operation = uuid::Uuid::new_v4().to_string();
    let expected = operation.clone();
    let gateway = McpGateway::new(
        vec![json!({"name":"game_runtime_call_tool"})],
        move |_, _, _, _| {
            started.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !flag.load(Ordering::SeqCst) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            json!({"content":[],"isError":true})
        },
    )
    .with_cancel(move |_, instance, op| {
        assert_eq!(instance, "owned");
        assert_eq!(op, expected);
        cancelled.store(true, Ordering::SeqCst);
    });
    let token=gateway.leases.issue(json!({"provider":"codex","taskId":uuid::Uuid::new_v4(),"conversationId":uuid::Uuid::new_v4()})).unwrap();
    let session = gateway.leases.begin(&token).unwrap();
    assert!(gateway.leases.ready(&token, &session, true));
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    headers.insert("mcp-session-id", HeaderValue::from_str(&session).unwrap());
    headers.insert(
        "mcp-protocol-version",
        HeaderValue::from_static("2025-11-25"),
    );
    let call = json!({"jsonrpc":"2.0","id":77,"method":"tools/call","params":{"name":"game_runtime_call_tool","arguments":{"instanceId":"owned","requestId":operation}}});
    let owner = gateway.clone();
    let owned_headers = headers.clone();
    let running =
        tokio::spawn(
            async move { protocol::handle(State(owner), owned_headers, Json(call)).await },
        );
    let deadline = Instant::now() + Duration::from_secs(2);
    while !entered.load(Ordering::SeqCst) && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(entered.load(Ordering::SeqCst));
    let other_token=gateway.leases.issue(json!({"provider":"codex","taskId":uuid::Uuid::new_v4(),"conversationId":uuid::Uuid::new_v4()})).unwrap();
    let other_session = gateway.leases.begin(&other_token).unwrap();
    assert!(gateway.leases.ready(&other_token, &other_session, true));
    let mut other = headers.clone();
    other.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {other_token}")).unwrap(),
    );
    other.insert(
        "mcp-session-id",
        HeaderValue::from_str(&other_session).unwrap(),
    );
    protocol::handle(
        State(gateway.clone()),
        other,
        Json(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":77}})),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!observed_cancel.load(Ordering::SeqCst));
    let reply = protocol::handle(
        State(gateway),
        headers,
        Json(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":77}})),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::ACCEPTED);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), running)
            .await
            .unwrap()
            .unwrap()
            .status(),
        StatusCode::OK
    );
}
