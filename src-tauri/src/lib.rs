pub mod foundation;
mod modules;

use foundation::{
    AppPaths, AppResult, AppSettings, Database, DatabaseRepairReport, SettingsService,
};
use modules::archive_transfer::{
    ArchiveTransferRecord, ArchiveTransferService, ArchiveTransferTarget, StartArchiveTransferInput,
};
use modules::codex_terminal::{
    CodexConversation, CodexTerminalAvailability, CodexTerminalEvent, CodexTerminalService,
    CodexTerminalState, CodexWorkflowSnapshot,
};
use modules::desktop_cli::{
    DesktopCliDependencies, DesktopCliGameDependencies, DesktopCliService, DesktopCliState,
};
use modules::game_archives::{ArchiveCatalogService, ArchiveOption, TransferableArchive};
use modules::game_connections::{
    ConnectionRegistration, GameConnectionHandler, GameConnectionService, GameConnectionState,
    GameHello, LanInterface, LogBatch, LogBatchAck,
};
use modules::grok_terminal::{
    GrokConversation, GrokTerminalAvailability, GrokTerminalEvent, GrokTerminalService,
    GrokTerminalState,
};
use modules::instances::{
    GameInstance, InstanceLogRepairReport, InstanceRuntimeInfo, InstanceService,
    InstanceStopResult, LaunchInstanceInput, WindowVisibilityMode,
};
use modules::logs::{LogFilter, LogRepairReport, LogService, LogSession, RuntimeLogEvent};
use modules::runtime_bridge::{RuntimeBridgeService, RuntimeBridgeState};
use modules::tasks::{DevelopmentTask, TaskInput, TaskService, TaskStatus};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{Manager, State, ipc::Channel};

struct AppState {
    paths: AppPaths,
    database: Database,
    settings: SettingsService,
    tasks: TaskService,
    codex_terminal: CodexTerminalService,
    grok_terminal: GrokTerminalService,
    connections: GameConnectionService,
    archives: ArchiveCatalogService,
    archive_transfers: ArchiveTransferService,
    instances: InstanceService,
    runtime_bridge: RuntimeBridgeService,
    logs: LogService,
    desktop_cli: DesktopCliService,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageRepairReport {
    database: DatabaseRepairReport,
    logs: LogRepairReport,
    instance_logs: InstanceLogRepairReport,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSettingsInput {
    game_executable_path: String,
    workspace_root_path: String,
    locale: String,
    game_gateway_port: u16,
    preferred_adapter_id: String,
    lan_broadcast_enabled: bool,
}

struct GameConnectionCoordinator {
    instances: InstanceService,
    logs: LogService,
}

impl GameConnectionHandler for GameConnectionCoordinator {
    fn connected(
        &self,
        hello: &GameHello,
        remote_address: std::net::SocketAddr,
    ) -> AppResult<ConnectionRegistration> {
        let mut registration = self.instances.register_connection(hello, remote_address)?;
        let server_session_id = if hello.log_session_id.trim().is_empty() {
            &hello.runtime_instance_id
        } else {
            &hello.log_session_id
        };
        let log_state = self.logs.connection_started(
            &registration.instance_id,
            server_session_id,
            &format!("ws://{remote_address}"),
        )?;
        registration.log_session_id = log_state.session_id;
        registration.resume_after_sequence = log_state.latest_sequence;
        registration.logs_enabled = log_state.enabled;
        Ok(registration)
    }

    fn disconnected(&self, instance_id: &str) {
        self.instances.mark_disconnected(instance_id);
        self.logs.connection_stopped(instance_id);
    }

    fn ingest_logs(&self, instance_id: &str, batch: &LogBatch) -> AppResult<LogBatchAck> {
        self.logs.ingest_batch(instance_id, batch)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetTaskStatusInput {
    id: String,
    status: TaskStatus,
}

#[tauri::command]
fn get_app_paths(state: State<'_, AppState>) -> AppPaths {
    state.paths.clone()
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.settings.get()
}

#[tauri::command]
fn update_settings(
    input: UpdateSettingsInput,
    state: State<'_, AppState>,
) -> AppResult<AppSettings> {
    let settings = state.settings.update_public(
        input.game_executable_path,
        input.workspace_root_path,
        input.locale,
        input.game_gateway_port,
        input.preferred_adapter_id,
        input.lan_broadcast_enabled,
    )?;
    state
        .tasks
        .set_workspace_root(PathBuf::from(&settings.workspace_root_path));
    state.desktop_cli.start(&settings)?;
    state.connections.start(&settings)?;
    Ok(settings)
}

#[tauri::command]
fn regenerate_desktop_cli_token(state: State<'_, AppState>) -> AppResult<AppSettings> {
    let settings = state.settings.regenerate_token()?;
    state.desktop_cli.start(&settings)?;
    Ok(settings)
}

#[tauri::command]
fn get_desktop_cli_state(state: State<'_, AppState>) -> DesktopCliState {
    state.desktop_cli.state()
}

#[tauri::command]
fn restart_desktop_cli(state: State<'_, AppState>) -> AppResult<DesktopCliState> {
    state.desktop_cli.start(&state.settings.get()?)
}

#[tauri::command]
fn list_lan_interfaces(state: State<'_, AppState>) -> AppResult<Vec<LanInterface>> {
    state.connections.list_interfaces()
}

#[tauri::command]
fn get_game_connection_state(state: State<'_, AppState>) -> GameConnectionState {
    state.connections.state()
}

#[tauri::command]
fn restart_game_connections(state: State<'_, AppState>) -> AppResult<GameConnectionState> {
    state.connections.start(&state.settings.get()?)
}

#[tauri::command]
async fn repair_storage(state: State<'_, AppState>) -> AppResult<StorageRepairReport> {
    let connections = state.connections.clone();
    let instances = state.instances.clone();
    let logs_service = state.logs.clone();
    let database = state.database.clone();
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        connections.stop();
        instances.stop_all();
        let logs = logs_service.repair_storage()?;
        let instance_logs = instances.repair_log_files()?;
        let database = database.repair_storage(&paths)?;
        Ok(StorageRepairReport {
            database,
            logs,
            instance_logs,
        })
    })
    .await
    .map_err(foundation::AppError::internal)?
}

#[tauri::command]
fn list_tasks(state: State<'_, AppState>) -> AppResult<Vec<DevelopmentTask>> {
    state.tasks.list()
}

#[tauri::command]
fn create_task(input: TaskInput, state: State<'_, AppState>) -> AppResult<DevelopmentTask> {
    state.tasks.create(input)
}

#[tauri::command]
fn update_task(
    id: String,
    input: TaskInput,
    state: State<'_, AppState>,
) -> AppResult<DevelopmentTask> {
    state.tasks.update(&id, input)
}

#[tauri::command]
fn set_task_status(
    input: SetTaskStatusInput,
    state: State<'_, AppState>,
) -> AppResult<DevelopmentTask> {
    state.tasks.set_status(&input.id, input.status)
}

#[tauri::command(async)]
fn delete_task(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.tasks.ensure_deletable(&id)?;
    state.codex_terminal.stop_task(&id);
    state.grok_terminal.stop_task(&id);
    state.tasks.delete(&id)
}

#[tauri::command]
fn get_codex_terminal_availability(state: State<'_, AppState>) -> CodexTerminalAvailability {
    state.codex_terminal.availability()
}

#[tauri::command]
fn open_codex_terminal(
    task_id: String,
    conversation_id: String,
    columns: u16,
    rows: u16,
    on_event: Channel<CodexTerminalEvent>,
    state: State<'_, AppState>,
) -> AppResult<CodexTerminalState> {
    state
        .codex_terminal
        .open(&task_id, &conversation_id, columns, rows, on_event)
}

#[tauri::command]
fn write_codex_terminal(
    conversation_id: String,
    data: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.codex_terminal.write(&conversation_id, &data)
}

#[tauri::command]
fn resize_codex_terminal(
    conversation_id: String,
    columns: u16,
    rows: u16,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.codex_terminal.resize(&conversation_id, columns, rows)
}

#[tauri::command(async)]
fn stop_codex_terminal(conversation_id: String, state: State<'_, AppState>) -> AppResult<()> {
    state
        .runtime_bridge
        .cancel_session(&format!("codex-{conversation_id}"));
    state.codex_terminal.stop(&conversation_id)
}

#[tauri::command]
fn list_codex_conversations(
    task_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<CodexConversation>> {
    state.codex_terminal.list_conversations(&task_id)
}

#[tauri::command]
fn create_codex_conversation(
    task_id: String,
    title: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<CodexConversation> {
    state.codex_terminal.create_conversation(&task_id, title)
}

#[tauri::command]
fn rename_codex_conversation(
    task_id: String,
    conversation_id: String,
    title: String,
    state: State<'_, AppState>,
) -> AppResult<CodexConversation> {
    state
        .codex_terminal
        .rename_conversation(&task_id, &conversation_id, &title)
}

#[tauri::command]
fn get_codex_workflow(
    task_id: String,
    conversation_id: String,
    state: State<'_, AppState>,
) -> AppResult<CodexWorkflowSnapshot> {
    state.codex_terminal.workflow(&task_id, &conversation_id)
}

#[tauri::command(async)]
fn delete_codex_conversation(
    task_id: String,
    conversation_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .codex_terminal
        .delete_conversation(&task_id, &conversation_id)
}

#[tauri::command]
fn get_grok_terminal_availability(state: State<'_, AppState>) -> GrokTerminalAvailability {
    state.grok_terminal.availability()
}

#[tauri::command]
fn open_grok_terminal(
    task_id: String,
    conversation_id: String,
    columns: u16,
    rows: u16,
    on_event: Channel<GrokTerminalEvent>,
    state: State<'_, AppState>,
) -> AppResult<GrokTerminalState> {
    state
        .grok_terminal
        .open(&task_id, &conversation_id, columns, rows, on_event)
}

#[tauri::command]
fn write_grok_terminal(
    conversation_id: String,
    data: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.grok_terminal.write(&conversation_id, &data)
}

#[tauri::command]
fn resize_grok_terminal(
    conversation_id: String,
    columns: u16,
    rows: u16,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.grok_terminal.resize(&conversation_id, columns, rows)
}

#[tauri::command(async)]
fn stop_grok_terminal(conversation_id: String, state: State<'_, AppState>) -> AppResult<()> {
    state
        .runtime_bridge
        .cancel_session(&format!("grok-{conversation_id}"));
    state.grok_terminal.stop(&conversation_id)
}

#[tauri::command]
fn list_grok_conversations(
    task_id: String,
    state: State<'_, AppState>,
) -> AppResult<Vec<GrokConversation>> {
    state.grok_terminal.list_conversations(&task_id)
}

#[tauri::command]
fn create_grok_conversation(
    task_id: String,
    title: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<GrokConversation> {
    state.grok_terminal.create_conversation(&task_id, title)
}

#[tauri::command]
fn rename_grok_conversation(
    task_id: String,
    conversation_id: String,
    title: String,
    state: State<'_, AppState>,
) -> AppResult<GrokConversation> {
    state
        .grok_terminal
        .rename_conversation(&task_id, &conversation_id, &title)
}

#[tauri::command]
fn get_grok_workflow(
    task_id: String,
    conversation_id: String,
    state: State<'_, AppState>,
) -> AppResult<CodexWorkflowSnapshot> {
    state.grok_terminal.workflow(&task_id, &conversation_id)
}

#[tauri::command(async)]
fn delete_grok_conversation(
    task_id: String,
    conversation_id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state
        .grok_terminal
        .delete_conversation(&task_id, &conversation_id)
}

#[tauri::command]
fn list_instances(
    task_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<GameInstance>> {
    state.instances.list(task_id.as_deref())
}

#[tauri::command]
fn get_instance(id: String, state: State<'_, AppState>) -> AppResult<InstanceRuntimeInfo> {
    state.instances.get(&id)
}

#[tauri::command]
fn discover_archives(
    root: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ArchiveOption>> {
    state.archives.discover(root.as_deref())
}

#[tauri::command]
fn list_archive_transfer_targets(
    state: State<'_, AppState>,
) -> AppResult<Vec<ArchiveTransferTarget>> {
    state.archive_transfers.list_targets()
}

#[tauri::command]
fn list_archive_transfer_sources(
    state: State<'_, AppState>,
) -> AppResult<Vec<TransferableArchive>> {
    state.archive_transfers.list_sources()
}

#[tauri::command]
fn inspect_archive_transfer_source(
    main_archive_path: String,
    state: State<'_, AppState>,
) -> AppResult<TransferableArchive> {
    state.archive_transfers.inspect_source(&main_archive_path)
}

#[tauri::command]
fn start_archive_transfer(
    input: StartArchiveTransferInput,
    state: State<'_, AppState>,
) -> AppResult<ArchiveTransferRecord> {
    state.archive_transfers.start(input)
}

#[tauri::command]
fn list_archive_transfers(state: State<'_, AppState>) -> AppResult<Vec<ArchiveTransferRecord>> {
    state.archive_transfers.list()
}

#[tauri::command]
fn get_archive_transfer(
    id: String,
    state: State<'_, AppState>,
) -> AppResult<ArchiveTransferRecord> {
    state.archive_transfers.get(&id)
}

#[tauri::command]
fn cancel_archive_transfer(
    id: String,
    state: State<'_, AppState>,
) -> AppResult<ArchiveTransferRecord> {
    state.archive_transfers.cancel(&id)
}

#[tauri::command]
fn launch_instance(
    input: LaunchInstanceInput,
    state: State<'_, AppState>,
) -> AppResult<GameInstance> {
    state.instances.launch(input)
}

#[tauri::command]
fn stop_instance(id: String, state: State<'_, AppState>) -> AppResult<InstanceStopResult> {
    let _ = state.logs.stop(&id);
    let result = state.instances.stop(&id)?;
    if !result.process_alive {
        state.runtime_bridge.forget_instance(&id);
    }
    Ok(result)
}

#[tauri::command]
fn set_instance_window_visibility(
    id: String,
    visibility_mode: WindowVisibilityMode,
    state: State<'_, AppState>,
) -> AppResult<GameInstance> {
    state.instances.set_window_visibility(&id, visibility_mode)
}

#[tauri::command]
fn delete_instance(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.instances.delete(&id)?;
    state.runtime_bridge.forget_instance(&id);
    Ok(())
}

#[tauri::command]
async fn read_runtime_artifact(
    task_id: String,
    path: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let bridge = state.runtime_bridge.clone();
    tauri::async_runtime::spawn_blocking(move || bridge.read_artifact(&task_id, &path))
        .await
        .map_err(foundation::AppError::internal)?
}

#[tauri::command]
async fn get_runtime_bridge_state(
    instance_id: String,
    state: State<'_, AppState>,
) -> AppResult<RuntimeBridgeState> {
    let bridge = state.runtime_bridge.clone();
    tauri::async_runtime::spawn_blocking(move || bridge.state(&instance_id))
        .await
        .map_err(foundation::AppError::internal)
}

#[tauri::command]
fn list_log_sessions(
    instance_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<LogSession>> {
    state.logs.list_sessions(instance_id.as_deref())
}

#[tauri::command]
fn list_log_sources(state: State<'_, AppState>) -> AppResult<Vec<modules::logs::LogSource>> {
    state.logs.list_sources()
}

#[tauri::command]
fn start_log_collection(instance_id: String, state: State<'_, AppState>) -> AppResult<LogSession> {
    state.logs.start(&instance_id)
}

#[tauri::command]
fn stop_log_collection(instance_id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.logs.stop(&instance_id)
}

#[tauri::command]
fn delete_log_session(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.logs.delete_session(&id)
}

#[tauri::command]
fn query_log_events(
    filter: LogFilter,
    state: State<'_, AppState>,
) -> AppResult<Vec<RuntimeLogEvent>> {
    state.logs.query(filter)
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let paths = AppPaths::discover()?;
            let database = Database::open(&paths)?;
            let settings =
                SettingsService::new(database.clone(), paths.default_workspace_root.clone());
            let current_settings = settings.get()?;
            let tasks = TaskService::new(
                database.clone(),
                PathBuf::from(&current_settings.workspace_root_path),
            );
            let codex_terminal = CodexTerminalService::new(tasks.clone())?;
            let grok_terminal = GrokTerminalService::new(tasks.clone())?;
            let connections = GameConnectionService::new();
            let archives = ArchiveCatalogService::new(paths.clone());
            let instances =
                InstanceService::new(database.clone(), paths.clone(), connections.clone());
            let runtime_bridge = RuntimeBridgeService::new(instances.clone(), tasks.clone());
            let logs = LogService::new(database.clone(), connections.clone());
            let archive_transfers = ArchiveTransferService::new(
                database.clone(),
                paths.clone(),
                archives.clone(),
                instances.clone(),
                connections.clone(),
            );
            connections.set_handler(std::sync::Arc::new(GameConnectionCoordinator {
                instances: instances.clone(),
                logs: logs.clone(),
            }));
            let desktop_cli = DesktopCliService::new(DesktopCliDependencies::new(
                settings.clone(),
                tasks.clone(),
                codex_terminal.clone(),
                grok_terminal.clone(),
                DesktopCliGameDependencies::new(
                    instances.clone(),
                    runtime_bridge.clone(),
                    logs.clone(),
                    connections.clone(),
                    archives.clone(),
                    archive_transfers.clone(),
                ),
            ));
            connections.start(&current_settings)?;
            desktop_cli.start(&current_settings)?;
            app.manage(AppState {
                paths,
                database: database.clone(),
                settings,
                tasks,
                codex_terminal,
                grok_terminal,
                connections,
                archives,
                archive_transfers,
                instances,
                runtime_bridge,
                logs,
                desktop_cli,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_paths,
            get_settings,
            update_settings,
            regenerate_desktop_cli_token,
            get_desktop_cli_state,
            restart_desktop_cli,
            list_lan_interfaces,
            get_game_connection_state,
            restart_game_connections,
            repair_storage,
            list_tasks,
            create_task,
            update_task,
            set_task_status,
            delete_task,
            get_codex_terminal_availability,
            open_codex_terminal,
            write_codex_terminal,
            resize_codex_terminal,
            stop_codex_terminal,
            list_codex_conversations,
            create_codex_conversation,
            rename_codex_conversation,
            get_codex_workflow,
            delete_codex_conversation,
            get_grok_terminal_availability,
            open_grok_terminal,
            write_grok_terminal,
            resize_grok_terminal,
            stop_grok_terminal,
            list_grok_conversations,
            create_grok_conversation,
            rename_grok_conversation,
            get_grok_workflow,
            delete_grok_conversation,
            list_instances,
            get_instance,
            discover_archives,
            list_archive_transfer_targets,
            list_archive_transfer_sources,
            inspect_archive_transfer_source,
            start_archive_transfer,
            list_archive_transfers,
            get_archive_transfer,
            cancel_archive_transfer,
            launch_instance,
            stop_instance,
            set_instance_window_visibility,
            delete_instance,
            get_runtime_bridge_state,
            read_runtime_artifact,
            list_log_sources,
            list_log_sessions,
            start_log_collection,
            stop_log_collection,
            delete_log_session,
            query_log_events,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build ABYA Desktop Development Tool");

    app.run(|app_handle, event| {
        if matches!(
            event,
            tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
        ) {
            let state = app_handle.state::<AppState>();
            state.archive_transfers.stop_all();
            state.codex_terminal.stop_all();
            state.grok_terminal.stop_all();
            state.instances.stop_all();
            state.connections.stop();
            state.desktop_cli.stop();
        }
    });
}
