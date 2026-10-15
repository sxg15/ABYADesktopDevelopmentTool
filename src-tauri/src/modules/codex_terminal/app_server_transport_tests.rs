use super::*;
use std::net::TcpStream;
use std::thread::JoinHandle;
use tungstenite::WebSocket;

// The handshake callback's error type is fixed by tungstenite.
#[allow(clippy::result_large_err)]
fn validate_auth(
    request: &tungstenite::handshake::server::Request,
    response: tungstenite::handshake::server::Response,
) -> Result<tungstenite::handshake::server::Response, tungstenite::handshake::server::ErrorResponse>
{
    assert_eq!(request.headers()["Authorization"], "Bearer test-token");
    Ok(response)
}

fn bridge_fixture(
    serve: impl FnOnce(WebSocket<TcpStream>) + Send + 'static,
) -> (AppServerHandle, JoinHandle<()>, JoinHandle<()>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let endpoint = format!("ws://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(REQUEST_TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(REQUEST_TIMEOUT)).unwrap();
        let socket = tungstenite::accept_hdr(stream, validate_auth).unwrap();
        serve(socket);
    });
    let (sender, receiver) = mpsc::channel();
    let mut handle = AppServerHandle::for_test(&endpoint, "test-token");
    handle.sender = sender;
    let bridge = std::thread::spawn(move || {
        run_bridge(&endpoint, "test-token", receiver, Arc::new(|_| {}));
    });
    (handle, bridge, server)
}

fn read_request(socket: &mut WebSocket<TcpStream>, method: &str) -> Value {
    let request: Value = serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(request["method"], method);
    request
}

#[test]
fn large_native_history_preserves_all_turns_and_keeps_bridge_usable() {
    let (handle, bridge, server) = bridge_fixture(|mut socket| {
        let request = read_request(&mut socket, "thread/read");
        assert_eq!(request["params"]["threadId"], "original-thread");
        let turns: Vec<_> = (0..11)
            .map(|index| {
                json!({
                    "id":format!("turn-{index}"), "status":"completed",
                    "items":[{"text":"x".repeat(1_800_000)}]
                })
            })
            .collect();
        let response = json!({"id":request["id"],"result":{"thread":{
            "id":"original-thread", "turns":turns
        }}})
        .to_string();
        assert!(response.len() > 16 * 1024 * 1024);
        socket.send(Message::Text(response.into())).unwrap();
        let next = read_request(&mut socket, "thread/list");
        socket
            .send(Message::Text(
                json!({"id":next["id"],"result":{"data":[]}})
                    .to_string()
                    .into(),
            ))
            .unwrap();
        let _ = socket.read();
    });
    let result = handle
        .request(
            "thread/read",
            json!({"threadId":"original-thread","includeTurns":true}),
        )
        .unwrap();
    assert_eq!(result["thread"]["id"], "original-thread");
    let turns = result["thread"]["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 11);
    for (index, turn) in turns.iter().enumerate() {
        assert_eq!(turn["id"], format!("turn-{index}"));
        assert_eq!(turn["items"][0]["text"].as_str().unwrap().len(), 1_800_000);
    }
    assert_eq!(
        handle.request("thread/list", json!({})).unwrap()["data"],
        json!([])
    );
    handle.stop();
    bridge.join().unwrap();
    server.join().unwrap();
}

#[test]
fn fragmented_native_response_can_exceed_the_old_message_limit() {
    use tungstenite::protocol::frame::{
        Frame,
        coding::{Data, OpCode},
    };
    let (handle, bridge, server) = bridge_fixture(|mut socket| {
        let request = read_request(&mut socket, "thread/resume");
        let prefix = format!("{{\"id\":{},\"result\":{{\"text\":\"", request["id"]);
        socket
            .send(Message::Frame(Frame::message(
                prefix.into_bytes(),
                OpCode::Data(Data::Text),
                false,
            )))
            .unwrap();
        for _ in 0..65 {
            socket
                .send(Message::Frame(Frame::message(
                    vec![b'x'; 1024 * 1024],
                    OpCode::Data(Data::Continue),
                    false,
                )))
                .unwrap();
        }
        socket
            .send(Message::Frame(Frame::message(
                b"\"}}".to_vec(),
                OpCode::Data(Data::Continue),
                true,
            )))
            .unwrap();
        let _ = socket.read();
    });
    let result = handle
        .request("thread/resume", json!({"threadId":"original-thread"}))
        .unwrap();
    assert_eq!(result["text"].as_str().unwrap().len(), 65 * 1024 * 1024);
    handle.stop();
    bridge.join().unwrap();
    server.join().unwrap();
}

#[test]
fn oversized_native_frame_returns_safe_capacity_error_before_reading_payload() {
    use std::io::Write;
    let (handle, bridge, server) = bridge_fixture(|mut socket| {
        let _ = read_request(&mut socket, "thread/read");
        // Advertise an oversized frame without allocating or transmitting its body.
        let mut header = vec![0x81, 127];
        header.extend_from_slice(&((MAX_NATIVE_RESPONSE_BYTES + 1) as u64).to_be_bytes());
        socket.get_mut().write_all(&header).unwrap();
        let _ = socket.read();
    });
    let error = handle
        .request("thread/read", json!({"threadId":"original-thread"}))
        .unwrap_err();
    assert_eq!(error.code, "codex_response_too_large");
    assert!(error.message.contains("128 MiB"));
    assert!(!error.to_string().contains("test-token"));
    assert!(!error.to_string().contains("original-thread"));
    handle.stop();
    bridge.join().unwrap();
    server.join().unwrap();
}
