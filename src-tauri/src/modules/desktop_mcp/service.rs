use super::{registry, tools};

use crate::foundation::{AppError, AppResult, AppSettings, SettingsService};
use crate::modules::archive_transfer::ArchiveTransferService;
use crate::modules::codex_terminal::CodexTerminalService;
use crate::modules::game_archives::ArchiveCatalogService;
use crate::modules::game_connections::GameConnectionService;
use crate::modules::instances::InstanceService;
use crate::modules::logs::LogService;
use crate::modules::runtime_bridge::RuntimeBridgeService;
use crate::modules::tasks::TaskService;

use parking_lot::Mutex;

use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tools::DesktopToolDispatcher;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopMcpState {
    pub running: bool,
    pub endpoint: String,
    pub port: u16,
    pub tool_count: usize,
    pub last_error: String,
}

#[derive(Clone)]
pub struct DesktopMcpDependencies {
    pub(super) settings: SettingsService,
    pub(super) tasks: TaskService,
    pub(super) codex_terminal: CodexTerminalService,
    pub(super) instances: InstanceService,
    pub(super) runtime_bridge: RuntimeBridgeService,
    pub(super) logs: LogService,
    pub(super) connections: GameConnectionService,
    pub(super) archives: ArchiveCatalogService,
    pub(super) archive_transfers: ArchiveTransferService,
}

pub struct DesktopMcpGameDependencies {
    instances: InstanceService,
    runtime_bridge: RuntimeBridgeService,
    logs: LogService,
    connections: GameConnectionService,
    archives: ArchiveCatalogService,
    archive_transfers: ArchiveTransferService,
}

impl DesktopMcpGameDependencies {
    pub fn new(
        instances: InstanceService,
        runtime_bridge: RuntimeBridgeService,
        logs: LogService,
        connections: GameConnectionService,
        archives: ArchiveCatalogService,
        archive_transfers: ArchiveTransferService,
    ) -> Self {
        Self {
            instances,
            runtime_bridge,
            logs,
            connections,
            archives,
            archive_transfers,
        }
    }
}

impl DesktopMcpDependencies {
    pub fn new(
        settings: SettingsService,
        tasks: TaskService,
        codex_terminal: CodexTerminalService,
        game: DesktopMcpGameDependencies,
    ) -> Self {
        Self {
            settings,
            tasks,
            codex_terminal,
            instances: game.instances,
            runtime_bridge: game.runtime_bridge,
            logs: game.logs,
            connections: game.connections,
            archives: game.archives,
            archive_transfers: game.archive_transfers,
        }
    }
}

#[derive(Clone)]
pub struct DesktopMcpService {
    inner: Arc<Mutex<ServerControl>>,
    dispatcher: DesktopToolDispatcher,
    runtime_bridge: RuntimeBridgeService,
}

struct ServerControl {
    state: DesktopMcpState,
    shutdown: Option<oneshot::Sender<()>>,
    generation: u64,
}

impl DesktopMcpService {
    pub fn new(dependencies: DesktopMcpDependencies) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ServerControl {
                state: DesktopMcpState {
                    running: false,
                    endpoint: String::new(),
                    port: 0,
                    tool_count: registry::tool_count(),
                    last_error: String::new(),
                },
                shutdown: None,
                generation: 0,
            })),
            runtime_bridge: dependencies.runtime_bridge.clone(),
            dispatcher: DesktopToolDispatcher::new(dependencies),
        }
    }

    pub fn start(&self, settings: &AppSettings) -> AppResult<DesktopMcpState> {
        self.stop_listener(false);
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), settings.desktop_mcp_port);
        let listener = bind_loopback(address)?;
        listener.set_nonblocking(true)?;
        let dispatcher = self.dispatcher.clone();
        let catalog = registry::tools()
            .into_iter()
            .map(|t| serde_json::to_value(t).expect("tool schema"))
            .collect();
        let runtime = self.runtime_bridge.clone();
        let gateway = crate::modules::desktop_mcp::McpGateway::new(
            catalog,
            move |context, operation, name, arguments| {
                serde_json::to_value(dispatcher.call_with_context(
                    Some(context),
                    &operation,
                    &name,
                    arguments,
                ))
                .expect("tool result")
            },
        )
        .with_authorizer(crate::foundation::mcp_sessions::mcp_context)
        .with_cancel(move |context, instance, operation| {
            if let Some(conversation) = context["conversationId"].as_str() {
                let _ = runtime.cancel(&instance, &operation, &format!("codex-{conversation}"));
            }
        });
        let router = gateway.router();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let control = self.inner.clone();
        let generation = control.lock().generation;
        tauri::async_runtime::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => listener,
                Err(error) => {
                    let mut control = control.lock();
                    if control.generation != generation {
                        return;
                    }
                    control.state.running = false;
                    control.state.last_error = error.to_string();
                    return;
                }
            };
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
            let mut control = control.lock();
            if control.generation != generation {
                return;
            }
            control.state.running = false;
            if let Err(error) = result {
                control.state.last_error = error.to_string();
            }
        });
        let state = DesktopMcpState {
            running: true,
            endpoint: format!("http://127.0.0.1:{}/mcp", settings.desktop_mcp_port),
            port: settings.desktop_mcp_port,
            tool_count: registry::tool_count(),
            last_error: String::new(),
        };
        crate::foundation::mcp_sessions::activate(state.endpoint.clone());
        let mut control = self.inner.lock();
        control.state = state.clone();
        control.shutdown = Some(shutdown_tx);
        Ok(state)
    }

    pub fn stop(&self) {
        self.stop_listener(true);
    }

    fn stop_listener(&self, revoke: bool) {
        if revoke {
            crate::foundation::mcp_sessions::deactivate();
            self.dispatcher.shutdown();
        }
        let mut control = self.inner.lock();
        if let Some(shutdown) = control.shutdown.take() {
            let _ = shutdown.send(());
        }
        control.generation += 1;
        control.state.running = false;
    }

    pub fn state(&self) -> DesktopMcpState {
        self.inner.lock().state.clone()
    }
}

fn bind_loopback(address: SocketAddr) -> AppResult<TcpListener> {
    let mut last_error = None;
    for _ in 0..20 {
        match TcpListener::bind(address) {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                return Err(AppError::new(
                    "desktopMcpBindFailed",
                    "Desktop MCP could not bind its loopback port.",
                    error.to_string(),
                ));
            }
        }
    }
    Err(AppError::new(
        "desktopMcpBindFailed",
        "Desktop MCP could not restart because its loopback port is still in use.",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_default(),
    ))
}
