mod protocol;
mod registry;
mod tools;
mod pipe;
mod pipe_security;

use crate::foundation::{AppError, AppResult, AppSettings, SettingsService};
use crate::modules::archive_transfer::ArchiveTransferService;
use crate::modules::codex_terminal::CodexTerminalService;
use crate::modules::game_archives::ArchiveCatalogService;
use crate::modules::game_connections::GameConnectionService;
use crate::modules::grok_terminal::GrokTerminalService;
use crate::modules::instances::InstanceService;
use crate::modules::logs::LogService;
use crate::modules::runtime_bridge::RuntimeBridgeService;
use crate::modules::tasks::TaskService;
use axum::{Router, routing::post};
use parking_lot::Mutex;
use protocol::{HttpState, handle_command};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tools::DesktopToolDispatcher;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCliState {
    pub running: bool,
    pub endpoint: String,
    pub port: u16,
    pub tool_count: usize,
    pub last_error: String,
}

#[derive(Clone)]
pub struct DesktopCliDependencies {
    settings: SettingsService,
    tasks: TaskService,
    codex_terminal: CodexTerminalService,
    grok_terminal: GrokTerminalService,
    instances: InstanceService,
    runtime_bridge: RuntimeBridgeService,
    logs: LogService,
    connections: GameConnectionService,
    archives: ArchiveCatalogService,
    archive_transfers: ArchiveTransferService,
}

pub struct DesktopCliGameDependencies {
    instances: InstanceService,
    runtime_bridge: RuntimeBridgeService,
    logs: LogService,
    connections: GameConnectionService,
    archives: ArchiveCatalogService,
    archive_transfers: ArchiveTransferService,
}

impl DesktopCliGameDependencies {
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

impl DesktopCliDependencies {
    pub fn new(
        settings: SettingsService,
        tasks: TaskService,
        codex_terminal: CodexTerminalService,
        grok_terminal: GrokTerminalService,
        game: DesktopCliGameDependencies,
    ) -> Self {
        Self {
            settings,
            tasks,
            codex_terminal,
            grok_terminal,
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
pub struct DesktopCliService {
    inner: Arc<Mutex<ServerControl>>,
    dispatcher: DesktopToolDispatcher,
}

struct ServerControl {
    pipe: Option<tokio::task::JoinHandle<()>>,
    state: DesktopCliState,
    shutdown: Option<oneshot::Sender<()>>,
    generation: u64,
}

impl DesktopCliService {
    pub fn new(dependencies: DesktopCliDependencies) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ServerControl {
                pipe: None,
                state: DesktopCliState {
                    running: false,
                    endpoint: String::new(),
                    port: 0,
                    tool_count: registry::tool_count(),
                    last_error: String::new(),
                },
                shutdown: None,
                generation: 0,
            })),
            dispatcher: DesktopToolDispatcher::new(dependencies),
        }
    }

    pub fn start(&self, settings: &AppSettings) -> AppResult<DesktopCliState> {
        self.stop();
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), settings.desktop_cli_port);
        let listener = bind_loopback(address)?;
        listener.set_nonblocking(true)?;
        let state = HttpState::new(settings.desktop_cli_token.clone(), self.dispatcher.clone());
        let pipe = pipe::start(state.clone(), settings.desktop_cli_token.clone())?;
        let router = Router::new()
            .route("/api/v1/command", post(handle_command))
            .layer(protocol::body_limit())
            .with_state(state);
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
        let state = DesktopCliState {
            running: true,
            endpoint: format!(
                "http://127.0.0.1:{}/api/v1/command",
                settings.desktop_cli_port
            ),
            port: settings.desktop_cli_port,
            tool_count: registry::tool_count(),
            last_error: String::new(),
        };
        #[cfg(not(test))]
        crate::foundation::cli_environment::publish(&state.endpoint, &settings.desktop_cli_token)?;
        let mut control = self.inner.lock();
        control.state = state.clone();
        control.pipe = Some(pipe);
        control.shutdown = Some(shutdown_tx);
        Ok(state)
    }

    pub fn stop(&self) {
        crate::foundation::cli_sessions::deactivate();
        self.dispatcher.shutdown();
        let mut control = self.inner.lock();
        if let Some(pipe) = control.pipe.take() { pipe.abort(); }
        if let Some(shutdown) = control.shutdown.take() {
            let _ = shutdown.send(());
        }
        control.generation += 1;
        control.state.running = false;
    }

    pub fn state(&self) -> DesktopCliState {
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
                    "desktopCliBindFailed",
                    "Desktop CLI could not bind its loopback port.",
                    error.to_string(),
                ));
            }
        }
    }
    Err(AppError::new(
        "desktopCliBindFailed",
        "Desktop CLI could not restart because its loopback port is still in use.",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_default(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{AppPaths, Database};
    use reqwest::blocking::Client;
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use uuid::Uuid;

    struct Fixture {
        service: DesktopCliService,
        settings: AppSettings,
        tasks: TaskService,
        root: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.service.stop();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!("abya-desktop-cli-http-{}", Uuid::new_v4()));
        let paths = AppPaths {
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
            data_dir: root.clone(),
        };
        std::fs::create_dir_all(&paths.bootstrap_logs_dir).unwrap();
        std::fs::create_dir_all(&paths.reports_dir).unwrap();
        std::fs::create_dir_all(&paths.archive_transfers_dir).unwrap();
        std::fs::create_dir_all(&paths.default_archive_root).unwrap();
        let database = Database::open(&paths).unwrap();
        let settings_service =
            SettingsService::new(database.clone(), paths.default_workspace_root.clone());
        let tasks = TaskService::new(database.clone(), paths.default_workspace_root.clone());
        let codex_terminal = CodexTerminalService::new(tasks.clone()).unwrap();
        let grok_terminal = GrokTerminalService::new(tasks.clone()).unwrap();
        let connections = GameConnectionService::new();
        let archives = ArchiveCatalogService::new(paths.clone());
        let instances = InstanceService::new(database.clone(), paths.clone(), connections.clone());
        let runtime_bridge = RuntimeBridgeService::new(instances.clone(), tasks.clone());
        let logs = LogService::new(database.clone(), connections.clone());
        let archive_transfers = ArchiveTransferService::new(
            database,
            paths,
            archives.clone(),
            instances.clone(),
            connections.clone(),
        );
        let service = DesktopCliService::new(DesktopCliDependencies::new(
            settings_service,
            tasks.clone(),
            codex_terminal,
            grok_terminal,
            DesktopCliGameDependencies::new(
                instances,
                runtime_bridge,
                logs,
                connections,
                archives,
                archive_transfers,
            ),
        ));
        let port = available_port();
        Fixture {
            service,
            settings: AppSettings {
                game_executable_path: String::new(),
                workspace_root_path: root.join("workspaces").to_string_lossy().into_owned(),
                locale: "en".to_string(),
                desktop_cli_port: port,
                desktop_cli_token: "desktop-cli-test-token".to_string(),
                game_gateway_port: 47610,
                preferred_adapter_id: String::new(),
                lan_broadcast_enabled: true,
                tool_id: Uuid::new_v4().to_string(),
            },
            tasks,
            root,
        }
    }

    fn available_port() -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.local_addr().unwrap().port()
    }

    #[test]
    fn serves_cli_commands_and_rejects_legacy_protocol() {
        let fixture = fixture();
        let state = fixture.service.start(&fixture.settings).unwrap();
        let client = Client::new();
        let request = |command: &str, arguments: Value| {
            json!({"version":1,
            "requestId":Uuid::new_v4().to_string(),"command":command,"arguments":arguments})
        };
        assert_eq!(
            client
                .post(&state.endpoint)
                .json(&request("capabilities", json!({})))
                .send()
                .unwrap()
                .status()
                .as_u16(),
            401
        );
        assert_eq!(
            client
                .post(&state.endpoint)
                .bearer_auth(&fixture.settings.desktop_cli_token)
                .header("Origin", "http://untrusted.example")
                .json(&request("capabilities", json!({})))
                .send()
                .unwrap()
                .status()
                .as_u16(),
            403
        );
        let listed: Value = client
            .post(&state.endpoint)
            .bearer_auth(&fixture.settings.desktop_cli_token)
            .json(&request("capabilities", json!({})))
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(listed["success"], true);
        assert_eq!(
            listed["data"].as_array().unwrap().len(),
            registry::tool_count()
        );
        let create_request = request("development_task_create", json!({"title":"CLI test"}));
        let called: Value = client
            .post(&state.endpoint)
            .bearer_auth(&fixture.settings.desktop_cli_token)
            .json(&create_request)
            .send()
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(called["success"], true);
        assert_eq!(fixture.tasks.list().unwrap().len(), 1);
        assert_eq!(
            client
                .post(&state.endpoint)
                .bearer_auth(&fixture.settings.desktop_cli_token)
                .json(&create_request)
                .send()
                .unwrap()
                .status()
                .as_u16(),
            409
        );
        assert_eq!(fixture.tasks.list().unwrap().len(), 1);
        assert!(
            serde_json::to_value(&fixture.settings)
                .unwrap()
                .get("desktopCliToken")
                .is_none()
        );
        let legacy = format!("http://127.0.0.1:{}/mcp", state.port);
        assert_eq!(
            client
                .post(legacy)
                .bearer_auth(&fixture.settings.desktop_cli_token)
                .json(&json!({"method":"tools/list"}))
                .send()
                .unwrap()
                .status()
                .as_u16(),
            404
        );
    }
}
