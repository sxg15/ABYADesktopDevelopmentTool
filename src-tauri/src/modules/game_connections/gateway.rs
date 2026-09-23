use super::discovery::{advertisement, preferred_interface, private_ipv4_interfaces};
use super::interfaces::GameConnectionHandler;
use super::models::{
    ArchiveTransferEvent, ArchiveTransferOffer, DISCONNECT_TIMEOUT_MS, GATEWAY_PATH,
    GameConnectionState, HELLO_TIMEOUT_MS, LanInterface, MAX_LOG_BATCH_BYTES, MAX_LOG_BATCH_EVENTS,
    MAX_MESSAGE_BYTES, PROTOCOL_VERSION, RECONNECT_GRACE_MS, SessionSnapshot,
};
use super::protocol::{ClientMessage, ServerMessage};
use super::sessions::{SessionRegistry, send_json};
use crate::foundation::{AppError, AppResult, AppSettings};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::Response;
use axum::{Router, routing::get};
use futures_util::{SinkExt, StreamExt};
use parking_lot::{Mutex, RwLock};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Clone)]
pub struct GameConnectionService {
    control: Arc<Mutex<GatewayControl>>,
    sessions: SessionRegistry,
    handler: Arc<RwLock<Option<Arc<dyn GameConnectionHandler>>>>,
}

struct GatewayControl {
    state: GameConnectionState,
    shutdown: Option<oneshot::Sender<()>>,
}

impl GameConnectionService {
    pub fn new() -> Self {
        Self {
            control: Arc::new(Mutex::new(GatewayControl {
                state: GameConnectionState::default(),
                shutdown: None,
            })),
            sessions: SessionRegistry::new(),
            handler: Default::default(),
        }
    }

    pub fn set_handler(&self, handler: Arc<dyn GameConnectionHandler>) {
        *self.handler.write() = Some(handler);
    }

    pub fn list_interfaces(&self) -> AppResult<Vec<LanInterface>> {
        private_ipv4_interfaces()
    }

    pub fn start(&self, settings: &AppSettings) -> AppResult<GameConnectionState> {
        self.stop();
        let interfaces = private_ipv4_interfaces()?;
        let preferred = preferred_interface(&interfaces, &settings.preferred_adapter_id).cloned();
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, settings.game_gateway_port))
            .map_err(|error| {
                AppError::new(
                    "gameGatewayBindFailed",
                    "The game connection gateway could not bind its LAN port.",
                    error.to_string(),
                )
            })?;
        listener.set_nonblocking(true)?;

        let advertised_addresses = interfaces
            .iter()
            .map(|interface| {
                format!(
                    "ws://{}:{}{}",
                    interface.address, settings.game_gateway_port, GATEWAY_PATH
                )
            })
            .collect::<Vec<_>>();
        let preferred_endpoint = preferred
            .as_ref()
            .map(|interface| {
                format!(
                    "ws://{}:{}{}",
                    interface.address, settings.game_gateway_port, GATEWAY_PATH
                )
            })
            .unwrap_or_default();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let state = GatewayHttpState {
            service: self.clone(),
            tool_id: settings.tool_id.clone(),
        };
        let router = Router::new()
            .route(GATEWAY_PATH, get(upgrade_websocket))
            .with_state(state);
        let control = self.control.clone();
        let service = self.clone();
        let discovery_settings = settings.clone();
        tauri::async_runtime::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(value) => value,
                Err(error) => {
                    let mut control = control.lock();
                    control.state.running = false;
                    control.state.last_error = error.to_string();
                    return;
                }
            };
            let discovery = if discovery_settings.lan_broadcast_enabled {
                Some(tauri::async_runtime::spawn(run_broadcasts(
                    interfaces,
                    discovery_settings.tool_id,
                    discovery_settings.game_gateway_port,
                )))
            } else {
                None
            };
            let result = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
            if let Some(discovery) = discovery {
                discovery.abort();
            }
            service.disconnect_all();
            let mut control = control.lock();
            control.state.running = false;
            control.state.broadcast_running = false;
            if let Err(error) = result {
                control.state.last_error = error.to_string();
            }
        });

        let state = GameConnectionState {
            running: true,
            broadcast_running: settings.lan_broadcast_enabled && !advertised_addresses.is_empty(),
            bind_endpoint: format!(
                "ws://0.0.0.0:{}{}",
                settings.game_gateway_port, GATEWAY_PATH
            ),
            preferred_endpoint,
            gateway_port: settings.game_gateway_port,
            discovery_port: super::models::DISCOVERY_PORT,
            path: GATEWAY_PATH.to_string(),
            protocol_version: PROTOCOL_VERSION,
            preferred_adapter_id: preferred.map(|value| value.id).unwrap_or_default(),
            advertised_addresses,
            connected_count: self.sessions.count(),
            tool_id: settings.tool_id.clone(),
            last_error: String::new(),
        };
        let mut control = self.control.lock();
        control.state = state.clone();
        control.shutdown = Some(shutdown_tx);
        Ok(state)
    }

    pub fn stop(&self) {
        let mut control = self.control.lock();
        if let Some(shutdown) = control.shutdown.take() {
            let _ = shutdown.send(());
        }
        control.state.running = false;
        control.state.broadcast_running = false;
        self.disconnect_all();
    }

    pub fn state(&self) -> GameConnectionState {
        let mut state = self.control.lock().state.clone();
        state.connected_count = self.sessions.count();
        state
    }

    pub fn preferred_endpoint(&self) -> AppResult<String> {
        let state = self.state();
        if !state.running {
            return Err(AppError::validation(
                "Start the game connection gateway before launching an instance.",
            ));
        }
        if state.preferred_endpoint.is_empty() {
            return Err(AppError::validation(
                "No private IPv4 adapter is available for game launch arguments.",
            ));
        }
        Ok(state.preferred_endpoint)
    }

    pub fn begin_archive_transfer(
        &self,
        instance_id: &str,
        offer: ArchiveTransferOffer,
    ) -> AppResult<tokio::sync::mpsc::UnboundedReceiver<ArchiveTransferEvent>> {
        self.sessions.begin_archive_transfer(instance_id, offer)
    }

    pub fn send_archive_chunk(
        &self,
        instance_id: &str,
        transfer_id: &str,
        offset: u64,
        payload: &[u8],
    ) -> AppResult<()> {
        self.sessions
            .send_archive_chunk(instance_id, transfer_id, offset, payload)
    }

    pub fn complete_archive_transfer(&self, instance_id: &str, transfer_id: &str) -> AppResult<()> {
        self.sessions
            .complete_archive_transfer(instance_id, transfer_id)
    }

    pub fn cancel_archive_transfer(&self, instance_id: &str, transfer_id: &str, reason: &str) {
        self.sessions
            .cancel_archive_transfer(instance_id, transfer_id, reason);
    }

    pub fn finish_archive_transfer(&self, transfer_id: &str) {
        self.sessions.finish_archive_transfer(transfer_id);
    }

    pub fn set_logs_enabled(&self, instance_id: &str, enabled: bool) -> AppResult<()> {
        self.sessions.send_logs_control(instance_id, enabled)
    }

    pub(crate) fn session(&self, instance_id: &str) -> Option<SessionSnapshot> {
        self.sessions.snapshot(instance_id)
    }

    pub(crate) fn sessions(&self) -> Vec<SessionSnapshot> {
        self.sessions.snapshots()
    }

    fn disconnect_all(&self) {
        let instance_ids = self.sessions.instance_ids();
        self.sessions.stop_all();
        if let Some(handler) = self.handler.read().clone() {
            for instance_id in instance_ids {
                handler.disconnected(&instance_id);
            }
        }
    }
}

#[derive(Clone)]
struct GatewayHttpState {
    service: GameConnectionService,
    tool_id: String,
}

async fn upgrade_websocket(
    websocket: WebSocketUpgrade,
    ConnectInfo(remote_address): ConnectInfo<SocketAddr>,
    State(state): State<GatewayHttpState>,
) -> Response {
    websocket
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, remote_address, state))
}

async fn handle_socket(socket: WebSocket, remote_address: SocketAddr, state: GatewayHttpState) {
    let (mut websocket_tx, mut websocket_rx) = socket.split();
    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<Message>();
    let writer = tauri::async_runtime::spawn(async move {
        while let Some(message) = outgoing_rx.recv().await {
            if websocket_tx.send(message).await.is_err() {
                break;
            }
        }
    });

    let hello =
        match tokio::time::timeout(Duration::from_millis(HELLO_TIMEOUT_MS), websocket_rx.next())
            .await
        {
            Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<ClientMessage>(&text).ok(),
            _ => None,
        };
    let Some(ClientMessage::Hello(hello)) = hello else {
        send_json(
            &outgoing_tx,
            &ServerMessage::Error {
                code: "helloRequired".into(),
                message: "The first WebSocket message must be hello.".into(),
            },
        );
        writer.abort();
        return;
    };
    if hello.protocol_version != PROTOCOL_VERSION || hello.runtime_instance_id.trim().is_empty() {
        send_json(
            &outgoing_tx,
            &ServerMessage::Error {
                code: "protocolMismatch".into(),
                message: format!("Game connection protocol {PROTOCOL_VERSION} is required."),
            },
        );
        writer.abort();
        return;
    }
    let Some(handler) = state.service.handler.read().clone() else {
        send_json(
            &outgoing_tx,
            &ServerMessage::Error {
                code: "gatewayNotReady".into(),
                message: "The desktop connection coordinator is not ready.".into(),
            },
        );
        writer.abort();
        return;
    };
    let registration = match handler.connected(&hello, remote_address) {
        Ok(value) => value,
        Err(error) => {
            send_json(
                &outgoing_tx,
                &ServerMessage::Error {
                    code: error.code,
                    message: error.message,
                },
            );
            writer.abort();
            return;
        }
    };
    let connection_id = Uuid::new_v4().to_string();
    let snapshot = SessionSnapshot {
        instance_id: registration.instance_id.clone(),
        connection_id: connection_id.clone(),
        remote_address: remote_address.to_string(),
        origin: registration.origin.clone(),
        game_version: hello.game_version.clone(),
        platform: hello.platform.clone(),
        capabilities: hello.capabilities.clone(),
    };
    state.service.sessions.insert(snapshot, outgoing_tx.clone());
    send_json(
        &outgoing_tx,
        &ServerMessage::Welcome {
            protocol_version: PROTOCOL_VERSION,
            tool_id: state.tool_id,
            connection_id: connection_id.clone(),
            instance_id: registration.instance_id.clone(),
            origin: registration.origin,
            heartbeat_interval_ms: super::models::HEARTBEAT_INTERVAL_MS,
            disconnect_timeout_ms: DISCONNECT_TIMEOUT_MS,
            reconnect_grace_ms: RECONNECT_GRACE_MS,
            log_session_id: registration.log_session_id,
            resume_after_sequence: registration.resume_after_sequence,
            logs_enabled: registration.logs_enabled,
        },
    );

    let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if state.service.sessions.is_timed_out(&registration.instance_id, &connection_id) {
                    send_json(&outgoing_tx, &ServerMessage::Disconnect {
                        reason: "Heartbeat timeout.".into(),
                    });
                    break;
                }
            }
            incoming = websocket_rx.next() => {
                let Some(Ok(message)) = incoming else {
                    break;
                };
                state.service.sessions.touch(&registration.instance_id, &connection_id);
                match message {
                    Message::Text(text) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(ClientMessage::Heartbeat) => {}
                            Ok(ClientMessage::LogBatch(batch)) => {
                                let size = text.len();
                                if batch.events.len() > MAX_LOG_BATCH_EVENTS || size > MAX_LOG_BATCH_BYTES {
                                    send_json(&outgoing_tx, &ServerMessage::Error {
                                        code: "logBatchTooLarge".into(),
                                        message: "A log batch may contain at most 100 events and 256 KiB.".into(),
                                    });
                                    continue;
                                }
                                match handler.ingest_logs(&registration.instance_id, &batch) {
                                    Ok(ack) => send_json(&outgoing_tx, &ServerMessage::LogAck {
                                        batch_id: batch.batch_id,
                                        latest_sequence: ack.latest_sequence,
                                        accepted_events: ack.accepted_events,
                                    }),
                                    Err(error) => send_json(&outgoing_tx, &ServerMessage::Error {
                                        code: error.code,
                                        message: error.message,
                                    }),
                                }
                            }
                            Ok(ClientMessage::ArchiveTransferDecision {
                                transfer_id,
                                accepted,
                                reason,
                            }) => {
                                state.service.sessions.resolve_archive_transfer(
                                    &registration.instance_id,
                                    &transfer_id,
                                    ArchiveTransferEvent::Decision { accepted, reason },
                                );
                            }
                            Ok(ClientMessage::ArchiveTransferAck {
                                transfer_id,
                                received_bytes,
                            }) => {
                                state.service.sessions.resolve_archive_transfer(
                                    &registration.instance_id,
                                    &transfer_id,
                                    ArchiveTransferEvent::Ack { received_bytes },
                                );
                            }
                            Ok(ClientMessage::ArchiveTransferProgress {
                                transfer_id,
                                phase,
                                percent,
                                message,
                            }) => {
                                state.service.sessions.resolve_archive_transfer(
                                    &registration.instance_id,
                                    &transfer_id,
                                    ArchiveTransferEvent::Progress {
                                        phase,
                                        percent,
                                        message,
                                    },
                                );
                            }
                            Ok(ClientMessage::ArchiveTransferResult {
                                transfer_id,
                                status,
                                installed_path,
                                error_code,
                                message,
                            }) => {
                                state.service.sessions.resolve_archive_transfer(
                                    &registration.instance_id,
                                    &transfer_id,
                                    ArchiveTransferEvent::Result {
                                        status,
                                        installed_path,
                                        error_code,
                                        message,
                                    },
                                );
                            }
                            Ok(ClientMessage::Disconnect { reason }) => {
                                let _ = reason;
                                break;
                            }
                            Ok(ClientMessage::Hello(_)) => {
                                send_json(&outgoing_tx, &ServerMessage::Error {
                                    code: "duplicateHello".into(),
                                    message: "hello may only be sent once per connection.".into(),
                                });
                            }
                            Err(error) => send_json(&outgoing_tx, &ServerMessage::Error {
                                code: "invalidMessage".into(),
                                message: error.to_string(),
                            }),
                        }
                    }
                    Message::Ping(payload) => {
                        let _ = outgoing_tx.send(Message::Pong(payload));
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
    }
    if state
        .service
        .sessions
        .remove_if_current(&registration.instance_id, &connection_id)
    {
        handler.disconnected(&registration.instance_id);
    }
    writer.abort();
}

async fn run_broadcasts(interfaces: Vec<LanInterface>, tool_id: String, gateway_port: u16) {
    let display_name = std::env::var("COMPUTERNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "ABYA Desktop Development Tool".into());
    let payload = advertisement(&tool_id, &display_name, gateway_port);
    let mut sockets = Vec::new();
    for interface in interfaces {
        let Ok(ip) = interface.address.parse::<Ipv4Addr>() else {
            continue;
        };
        let Ok(broadcast) = interface.broadcast_address.parse::<Ipv4Addr>() else {
            continue;
        };
        if let Ok(socket) = tokio::net::UdpSocket::bind((ip, 0)).await
            && socket.set_broadcast(true).is_ok()
        {
            sockets.push((
                socket,
                SocketAddr::new(IpAddr::V4(broadcast), super::models::DISCOVERY_PORT),
            ));
        }
    }
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    loop {
        interval.tick().await;
        for (socket, target) in &sockets {
            let _ = socket.send_to(&payload, target).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::AppSettings;
    use crate::modules::game_connections::{
        ConnectionRegistration, GameHello, LogBatch, LogBatchAck,
    };
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeHandler {
        batches: AtomicUsize,
    }

    impl GameConnectionHandler for FakeHandler {
        fn connected(
            &self,
            _hello: &GameHello,
            _remote_address: SocketAddr,
        ) -> AppResult<ConnectionRegistration> {
            Ok(ConnectionRegistration {
                instance_id: "desktop-instance".into(),
                origin: "managed".into(),
                log_session_id: "desktop-log-session".into(),
                resume_after_sequence: 4,
                logs_enabled: true,
            })
        }

        fn disconnected(&self, _instance_id: &str) {}

        fn ingest_logs(&self, _instance_id: &str, batch: &LogBatch) -> AppResult<LogBatchAck> {
            self.batches.fetch_add(1, Ordering::SeqCst);
            Ok(LogBatchAck {
                latest_sequence: batch
                    .events
                    .iter()
                    .filter_map(|event| event["sequence"].as_i64())
                    .max()
                    .unwrap_or_default(),
                accepted_events: batch.events.len(),
            })
        }
    }

    #[test]
    fn default_state_exposes_locked_protocol_values() {
        let state = GameConnectionState::default();
        assert_eq!(state.path, "/game/v1/connect");
        assert_eq!(state.discovery_port, 47611);
        assert_eq!(state.protocol_version, 1);
    }

    #[test]
    fn server_messages_are_typed() {
        let value = serde_json::to_value(ServerMessage::LogsControl { enabled: false }).unwrap();
        assert_eq!(value, json!({"type":"logs_control","enabled":false}));
    }

    #[test]
    fn fake_game_can_connect_and_send_logs() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let port = TcpListener::bind(("127.0.0.1", 0))
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            let service = GameConnectionService::new();
            let handler = Arc::new(FakeHandler {
                batches: AtomicUsize::new(0),
            });
            service.set_handler(handler.clone());
            service
                .start(&AppSettings {
                    game_executable_path: String::new(),
                    workspace_root_path: String::new(),
                    locale: "en-US".into(),
                    desktop_mcp_port: port.saturating_add(1),
                    desktop_mcp_token: "test".into(),
                    game_gateway_port: port,
                    preferred_adapter_id: String::new(),
                    lan_broadcast_enabled: false,
                    tool_id: "tool-id".into(),
                })
                .unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;

            let (mut socket, _) =
                tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}{GATEWAY_PATH}"))
                    .await
                    .unwrap();
            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    json!({
                        "type": "hello",
                        "protocolVersion": 1,
                        "runtimeInstanceId": "runtime-1",
                        "instanceId": "desktop-instance",
                        "displayName": "Fake Game",
                        "gameVersion": "1.0",
                        "platform": "Windows",
                        "capabilities": ["logs"],
                        "logSessionId": "runtime-log-session"
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            let welcome = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let welcome: Value = serde_json::from_str(&welcome).unwrap();
            assert_eq!(welcome["type"], "welcome");
            assert_eq!(welcome["resumeAfterSequence"], 4);

            socket
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    json!({
                        "type": "log_batch",
                        "batchId": "batch-1",
                        "sessionId": "runtime-log-session",
                        "events": [{ "sequence": 5, "message": "connected" }]
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            let ack = socket.next().await.unwrap().unwrap().into_text().unwrap();
            let ack: Value = serde_json::from_str(&ack).unwrap();
            assert_eq!(ack["type"], "log_ack");
            assert_eq!(ack["latestSequence"], 5);
            assert_eq!(handler.batches.load(Ordering::SeqCst), 1);

            service.stop();
        });
    }
}
