use super::{DesktopCliDependencies, registry};
use crate::foundation::{AppError, AppResult};
use crate::modules::archive_transfer::StartArchiveTransferInput;
use crate::modules::codex_terminal::{CodexActivityKind, CodexActivityStatus};
use crate::modules::development_terminal::TerminalProvider;
use crate::modules::instances::{
    ArchiveSelection, LaunchInstanceInput, LaunchMode, LaunchProfile, ProcessState, WindowMode,
    WindowVisibilityMode,
};
use crate::modules::logs::LogFilter;
use crate::modules::tasks::{TaskInput, TaskStatus};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
#[cfg(test)]
use uuid::Uuid;

type ConversationBinding = (TerminalProvider, String, String);

#[derive(Clone)]
pub(crate) struct DesktopToolDispatcher {
    dependencies: DesktopCliDependencies,
    conversation_bindings: Arc<Mutex<HashMap<String, ConversationBinding>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolCallResult {
    pub(super) content: Vec<Value>,
    pub(super) is_error: bool,
}

impl DesktopToolDispatcher {
    pub(crate) fn new(dependencies: DesktopCliDependencies) -> Self {
        Self {
            dependencies,
            conversation_bindings: Default::default(),
        }
    }

    pub(super) fn shutdown(&self) {
        self.dependencies.runtime_bridge.shutdown();
        self.conversation_bindings.lock().clear();
    }

    // 每条命令重新验证归属；上下文 ID 不是认证凭据。
    pub(super) fn call_with_context(
        &self,
        context: Option<Value>,
        request_id: &str,
        name: &str,
        mut arguments: Value,
    ) -> ToolCallResult {
        let Some(context) = context else {
            if matches!(name, "development_task_list" | "development_task_create") {
                return self.dispatch(None, name, arguments, request_id);
            }
            return ToolCallResult::error(AppError::validation(
                "A task conversation context is required.",
            ));
        };
        let input: ConversationBindInput = match decode(context.clone()) {
            Ok(value) => value,
            Err(error) => return ToolCallResult::error(error),
        };
        let session = format!(
            "{}-{}",
            match input.provider {
                TerminalProvider::Codex => "codex",
                TerminalProvider::Grok => "grok",
            },
            input.conversation_id
        );
        if let Err(error) = self.bind_conversation(Some(&session), context) {
            return ToolCallResult::error(error);
        }
        if let Some(task) = arguments.get("taskId").and_then(Value::as_str)
            && task != input.task_id
        {
            return ToolCallResult::error(AppError::validation("Task context mismatch."));
        }
        let referenced_instance =
            if let Some(id) = arguments.get("instanceId").and_then(Value::as_str) {
                Some(id.to_string())
            } else if let Some(id) = arguments.get("transferId").and_then(Value::as_str) {
                match self.dependencies.archive_transfers.get(id) {
                    Ok(record) => Some(record.instance_id),
                    Err(error) => return ToolCallResult::error(error),
                }
            } else if name == "game_log_query" {
                let id = arguments["sessionId"].as_str().unwrap_or("");
                match self.dependencies.logs.list_sessions(None) {
                    Ok(sessions) => match sessions.into_iter().find(|session| session.id == id) {
                        Some(session) => Some(session.instance_id),
                        None => return ToolCallResult::error(AppError::not_found("Log session")),
                    },
                    Err(error) => return ToolCallResult::error(error),
                }
            } else {
                None
            };
        if let Some(id) = referenced_instance.as_deref() {
            match self.dependencies.instances.read(id) {
                Ok(instance)
                    if instance.task_id.as_deref() == Some(&input.task_id)
                        || (instance.origin
                            == crate::modules::instances::InstanceOrigin::External
                            && (name.starts_with("game_log_")
                                || name.starts_with("game_archive_transfer_"))) => {}
                _ => {
                    return ToolCallResult::error(AppError::validation(
                        "Instance is not owned by this task.",
                    ));
                }
            }
        }
        if name == "game_instance_list" {
            arguments["taskId"] = json!(input.task_id);
        }
        if name == "game_runtime_cancel" {
            let id = arguments["instanceId"].as_str().unwrap_or("");
            let operation = arguments["operationId"].as_str().unwrap_or("");
            return match self
                .dependencies
                .runtime_bridge
                .cancel(id, operation, &session)
            {
                Ok(value) => ToolCallResult::success(value),
                Err(error) => ToolCallResult::error(error),
            };
        }
        if name == "development_conversation_bind" {
            return ToolCallResult::success(json!({"bound":true}));
        }
        self.dispatch(Some(&session), name, arguments, request_id)
    }

    #[cfg(test)]
    pub(crate) fn call(&self, name: &str, arguments: Value) -> ToolCallResult {
        self.call_for_session(None, name, arguments)
    }

    #[cfg(test)]
    pub(crate) fn call_for_session(
        &self,
        session_id: Option<&str>,
        name: &str,
        arguments: Value,
    ) -> ToolCallResult {
        self.dispatch(session_id, name, arguments, &Uuid::new_v4().to_string())
    }

    fn dispatch(
        &self,
        session_id: Option<&str>,
        name: &str,
        arguments: Value,
        operation_id: &str,
    ) -> ToolCallResult {
        if name == "development_conversation_bind" {
            return match self.bind_conversation(session_id, arguments) {
                Ok(value) => ToolCallResult::success(value),
                Err(error) => ToolCallResult::error(error),
            };
        }
        if name == "development_conversation_activity_report" {
            return match self.report_activity(session_id, arguments) {
                Ok(value) => ToolCallResult::success(value),
                Err(error) => ToolCallResult::error(error),
            };
        }
        let binding = session_id
            .and_then(|session_id| self.conversation_bindings.lock().get(session_id).cloned());
        let activity_id = operation_id.to_string();
        let activity_arguments: Value = arguments
            .as_object()
            .map(|values| {
                Value::Object(
                    values
                        .iter()
                        .filter(|(key, _)| {
                            matches!(
                                key.as_str(),
                                "taskId"
                                    | "instanceId"
                                    | "toolName"
                                    | "transferId"
                                    | "sessionId"
                                    | "mode"
                                    | "targetState"
                            )
                        })
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                )
            })
            .unwrap_or_else(|| json!({}));
        if let Some((provider, task_id, conversation_id)) = binding.as_ref() {
            let _ = self.record_desktop_tool_activity(
                *provider,
                task_id,
                conversation_id,
                &activity_id,
                name,
                &activity_arguments,
                CodexActivityStatus::Started,
            );
        }
        if matches!(name, "game_runtime_call_tool") {
            let result = match self.call_runtime_tool(
                arguments,
                session_id.unwrap_or("desktop"),
                operation_id,
            ) {
                Ok(result) => ToolCallResult {
                    content: result.content,
                    is_error: result.is_error,
                },
                Err(error) => ToolCallResult::error(error),
            };
            self.complete_recorded_tool(binding, &activity_id, name, &activity_arguments, &result);
            return result;
        }
        let result = match self.invoke(name, arguments) {
            Ok(value) => ToolCallResult::success(value),
            Err(error) => ToolCallResult::error(error),
        };
        self.complete_recorded_tool(binding, &activity_id, name, &activity_arguments, &result);
        result
    }

    fn complete_recorded_tool(
        &self,
        binding: Option<ConversationBinding>,
        activity_id: &str,
        name: &str,
        arguments: &Value,
        result: &ToolCallResult,
    ) {
        if let Some((provider, task_id, conversation_id)) = binding {
            let _ = self.record_desktop_tool_activity(
                provider,
                &task_id,
                &conversation_id,
                activity_id,
                name,
                &json!({
                    "arguments": arguments,
                    "isError": result.is_error,
                    "artifacts": result.content.iter().filter(|v|v["type"]=="image")
                        .filter_map(|v|v["path"].as_str()).collect::<Vec<_>>()
                }),
                if result.is_error {
                    CodexActivityStatus::Failed
                } else {
                    CodexActivityStatus::Completed
                },
            );
        }
    }

    fn bind_conversation(&self, session_id: Option<&str>, arguments: Value) -> AppResult<Value> {
        let session_id = session_id.ok_or_else(|| {
            AppError::validation("An validated CLI context is required for conversation binding.")
        })?;
        let input: ConversationBindInput = decode(arguments)?;
        let conversation = match input.provider {
            TerminalProvider::Codex => serialize(
                self.dependencies
                    .codex_terminal
                    .bind_conversation(&input.task_id, &input.conversation_id)?,
            )?,
            TerminalProvider::Grok => serialize(
                self.dependencies
                    .grok_terminal
                    .bind_conversation(&input.task_id, &input.conversation_id)?,
            )?,
        };
        self.conversation_bindings.lock().insert(
            session_id.to_string(),
            (input.provider, input.task_id, input.conversation_id),
        );
        Ok(json!({
            "provider": input.provider,
            "conversation": conversation
        }))
    }

    fn report_activity(&self, session_id: Option<&str>, arguments: Value) -> AppResult<Value> {
        let session_id = session_id.ok_or_else(|| {
            AppError::validation("An validated CLI context is required for activity reporting.")
        })?;
        let (provider, task_id, conversation_id) = self
            .conversation_bindings
            .lock()
            .get(session_id)
            .cloned()
            .ok_or_else(|| {
                AppError::validation(
                    "Call development_conversation_bind before reporting conversation activity.",
                )
            })?;
        let input: ConversationActivityInput = decode(arguments)?;
        match provider {
            TerminalProvider::Codex => {
                serialize(self.dependencies.codex_terminal.record_reported_activity(
                    &task_id,
                    &conversation_id,
                    input.status,
                    input.kind,
                    &input.summary,
                    input.detail.as_deref().unwrap_or_default(),
                )?)
            }
            TerminalProvider::Grok => {
                serialize(self.dependencies.grok_terminal.record_reported_activity(
                    &task_id,
                    &conversation_id,
                    input.status,
                    input.kind,
                    &input.summary,
                    input.detail.as_deref().unwrap_or_default(),
                )?)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_desktop_tool_activity(
        &self,
        provider: TerminalProvider,
        task_id: &str,
        conversation_id: &str,
        activity_id: &str,
        tool_name: &str,
        arguments: &Value,
        status: CodexActivityStatus,
    ) -> AppResult<()> {
        match provider {
            TerminalProvider::Codex => self
                .dependencies
                .codex_terminal
                .record_desktop_tool_activity(
                    task_id,
                    conversation_id,
                    activity_id,
                    tool_name,
                    arguments,
                    status,
                )
                .map(|_| ()),
            TerminalProvider::Grok => self
                .dependencies
                .grok_terminal
                .record_desktop_tool_activity(
                    task_id,
                    conversation_id,
                    activity_id,
                    tool_name,
                    arguments,
                    status,
                )
                .map(|_| ()),
        }
    }

    fn invoke(&self, name: &str, arguments: Value) -> AppResult<Value> {
        if !arguments.is_object() {
            return Err(AppError::validation(
                "Desktop CLI tool arguments must be an object.",
            ));
        }
        match name {
            "desktop_get_capabilities" => {
                let _: EmptyInput = decode(arguments)?;
                Ok(json!({
                    "toolCount": registry::tool_count(),
                    "tools": registry::tool_names(),
                    "security": {
                        "loopbackOnly": true,
                        "bearerAuthentication": true,
                        "returnsSecrets": false
                    },
                    "limits": {
                        "rawGameArgumentsAccepted": false,
                        "maxLogQueryEvents": 1000,
                        "maxActiveArchiveTransfers": 2,
                        "maxArchiveTransfersPerTarget": 1,
                        "archiveTransferChunkBytes": 262144
                    },
                    "gameGateway": self.dependencies.connections.state()
                }))
            }
            "development_task_list" => {
                let input: TaskListInput = decode(arguments)?;
                let mut tasks = self.dependencies.tasks.list()?;
                if let Some(status) = input.status {
                    tasks.retain(|task| task.status == status);
                }
                serialize(tasks)
            }
            "development_task_create" => {
                let input: TaskCreateInput = decode(arguments)?;
                serialize(self.dependencies.tasks.create(TaskInput {
                    title: input.title,
                    description: input.description,
                })?)
            }
            "development_task_update" => {
                let input: TaskUpdateInput = decode(arguments)?;
                let description = match input.description {
                    Some(description) => description,
                    None => {
                        self.dependencies
                            .tasks
                            .list()?
                            .into_iter()
                            .find(|task| task.id == input.task_id)
                            .ok_or_else(|| AppError::not_found("Task"))?
                            .description
                    }
                };
                serialize(self.dependencies.tasks.update(
                    &input.task_id,
                    TaskInput {
                        title: input.title,
                        description,
                    },
                )?)
            }
            "development_task_set_status" => {
                let input: TaskStatusInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .tasks
                        .set_status(&input.task_id, input.status)?,
                )
            }
            "game_instance_list" => {
                let input: InstanceListInput = decode(arguments)?;
                serialize(self.dependencies.instances.list(input.task_id.as_deref())?)
            }
            "game_instance_get" => {
                let input: InstanceIdInput = decode(arguments)?;
                serialize(self.dependencies.instances.get(&input.instance_id)?)
            }
            "game_archive_list" => {
                let input: ArchiveListInput = decode(arguments)?;
                serialize(self.dependencies.archives.discover(input.root.as_deref())?)
            }
            "game_archive_transfer_target_list" => {
                let _: EmptyInput = decode(arguments)?;
                serialize(self.dependencies.archive_transfers.list_targets()?)
            }
            "game_archive_transfer_source_list" => {
                let _: EmptyInput = decode(arguments)?;
                serialize(self.dependencies.archive_transfers.list_sources()?)
            }
            "game_archive_transfer_start" => {
                let input: ArchiveTransferStartInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .archive_transfers
                        .start(StartArchiveTransferInput {
                            instance_id: input.instance_id,
                            main_archive_path: input.main_archive_path,
                        })?,
                )
            }
            "game_archive_transfer_get" => {
                let input: ArchiveTransferIdInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .archive_transfers
                        .get(&input.transfer_id)?,
                )
            }
            "game_archive_transfer_cancel" => {
                let input: ArchiveTransferIdInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .archive_transfers
                        .cancel(&input.transfer_id)?,
                )
            }
            "game_instance_launch" => {
                let input: CliLaunchInput = decode(arguments)?;
                let executable_path = match input.executable_path {
                    Some(path) if !path.trim().is_empty() => path,
                    _ => self.dependencies.settings.get()?.game_executable_path,
                };
                if executable_path.trim().is_empty() {
                    return Err(AppError::validation(
                        "Configure the game executable path in desktop settings or provide executablePath.",
                    ));
                }
                serialize(self.dependencies.instances.launch(LaunchInstanceInput {
                    task_id: input.task_id,
                    name: input.name,
                    executable_path,
                    profile: LaunchProfile {
                        mode: input.mode,
                        window_mode: input.window_mode,
                        visibility_mode: input.visibility_mode,
                        width: input.width,
                        height: input.height,
                        archive: input.archive,
                        host_instance_id: input.host_instance_id,
                        exit_on_failure: input.exit_on_failure,
                        language: input.language,
                    },
                })?)
            }
            "game_instance_set_window_visibility" => {
                let input: InstanceWindowVisibilityInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .instances
                        .set_window_visibility(&input.instance_id, input.visibility_mode)?,
                )
            }
            "game_instance_stop" => {
                let input: InstanceIdInput = decode(arguments)?;
                let _ = self.dependencies.logs.stop(&input.instance_id);
                let result = self.dependencies.instances.stop(&input.instance_id)?;
                if !result.process_alive {
                    self.dependencies
                        .runtime_bridge
                        .forget_instance(&input.instance_id);
                }
                serialize(result)
            }
            "game_instance_wait_for_state" => {
                let input: WaitForStateInput = decode(arguments)?;
                serialize(self.dependencies.instances.wait_for_state(
                    &input.instance_id,
                    input.target_state,
                    validated_timeout(input.timeout_ms)?,
                )?)
            }
            "game_instance_wait_for_cli" => {
                let input: WaitForCliInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .runtime_bridge
                        .wait_for_cli(&input.instance_id, validated_timeout(input.timeout_ms)?)?,
                )
            }
            "game_instance_get_launch_report" => {
                let input: InstanceIdInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .instances
                        .read_launch_report(&input.instance_id)?,
                )
            }
            "game_runtime_get_state" => {
                let input: InstanceIdInput = decode(arguments)?;
                serialize(self.dependencies.runtime_bridge.state(&input.instance_id))
            }
            "game_runtime_list_tools" => {
                let input: InstanceIdInput = decode(arguments)?;
                self.dependencies
                    .runtime_bridge
                    .list_tools(&input.instance_id)
            }
            "game_log_collection_start" => {
                let input: InstanceIdInput = decode(arguments)?;
                serialize(self.dependencies.logs.start(&input.instance_id)?)
            }
            "game_log_collection_stop" => {
                let input: InstanceIdInput = decode(arguments)?;
                self.dependencies.logs.stop(&input.instance_id)?;
                Ok(json!({ "instanceId": input.instance_id, "stopped": true }))
            }
            "game_log_session_list" => {
                let input: LogSessionListInput = decode(arguments)?;
                serialize(
                    self.dependencies
                        .logs
                        .list_sessions(input.instance_id.as_deref())?,
                )
            }
            "game_log_source_list" => {
                let _: EmptyInput = decode(arguments)?;
                serialize(self.dependencies.logs.list_sources()?)
            }
            "game_log_query" => {
                let input: LogQueryInput = decode(arguments)?;
                if let Some(limit) = input.limit
                    && !(1..=1000).contains(&limit)
                {
                    return Err(AppError::validation(
                        "Log query limit must be between 1 and 1000.",
                    ));
                }
                let exists = self
                    .dependencies
                    .logs
                    .list_sessions(None)?
                    .into_iter()
                    .any(|session| session.id == input.session_id);
                if !exists {
                    return Err(AppError::not_found("Log session"));
                }
                serialize(self.dependencies.logs.query(LogFilter {
                    session_id: input.session_id,
                    severity: input.severity,
                    provider: input.provider,
                    event_name: input.event_name,
                    contains: input.contains,
                    before_sequence: input.before_sequence,
                    limit: input.limit,
                })?)
            }
            _ => Err(AppError::new(
                "unknownDesktopCliTool",
                format!("Desktop CLI tool was not found: {name}"),
                "",
            )),
        }
    }

    fn call_runtime_tool(
        &self,
        arguments: Value,
        session_id: &str,
        operation_id: &str,
    ) -> AppResult<crate::modules::runtime_bridge::RuntimeToolResult> {
        if !arguments.is_object() {
            return Err(AppError::validation(
                "Desktop CLI tool arguments must be an object.",
            ));
        }
        let input: GameCliCallInput = decode(arguments)?;
        if input.tool_name.trim().is_empty() {
            return Err(AppError::validation("Game CLI tool name is required."));
        }
        if !input.arguments.is_object() {
            return Err(AppError::validation(
                "Game CLI tool arguments must be an object.",
            ));
        }
        self.dependencies.runtime_bridge.call_tool_scoped(
            &input.instance_id,
            &input.tool_name,
            input.arguments,
            session_id,
            operation_id,
        )
    }
}

impl ToolCallResult {
    fn success(value: Value) -> Self {
        Self {
            content: vec![json!({
                "type": "text",
                "text": value.to_string()
            })],
            is_error: false,
        }
    }

    fn error(error: AppError) -> Self {
        let text = serde_json::to_string(&error).unwrap_or_else(|_| {
            json!({
                "code": "internal",
                "message": "Desktop CLI tool failed.",
                "detail": error.to_string()
            })
            .to_string()
        });
        Self {
            content: vec![json!({ "type": "text", "text": text })],
            is_error: true,
        }
    }
}

fn decode<T: DeserializeOwned>(arguments: Value) -> AppResult<T> {
    serde_json::from_value(arguments).map_err(|error| {
        AppError::new(
            "invalidToolArguments",
            "Desktop CLI tool arguments are invalid.",
            error.to_string(),
        )
    })
}

fn serialize(value: impl Serialize) -> AppResult<Value> {
    serde_json::to_value(value).map_err(AppError::internal)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskListInput {
    status: Option<TaskStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskCreateInput {
    title: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskUpdateInput {
    task_id: String,
    title: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskStatusInput {
    task_id: String,
    status: TaskStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConversationBindInput {
    #[serde(default)]
    provider: TerminalProvider,
    task_id: String,
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConversationActivityInput {
    status: CodexActivityStatus,
    kind: CodexActivityKind,
    summary: String,
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstanceListInput {
    task_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstanceIdInput {
    instance_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArchiveListInput {
    root: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArchiveTransferStartInput {
    instance_id: String,
    main_archive_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArchiveTransferIdInput {
    transfer_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CliLaunchInput {
    task_id: String,
    name: String,
    executable_path: Option<String>,
    #[serde(default = "default_launch_mode")]
    mode: LaunchMode,
    #[serde(default = "default_window_mode")]
    window_mode: WindowMode,
    #[serde(default = "default_visibility_mode")]
    visibility_mode: WindowVisibilityMode,
    #[serde(default = "default_width")]
    width: u16,
    #[serde(default = "default_height")]
    height: u16,
    archive: Option<ArchiveSelection>,
    host_instance_id: Option<String>,
    #[serde(default = "default_exit_on_failure")]
    exit_on_failure: bool,
    #[serde(default)]
    language: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstanceWindowVisibilityInput {
    instance_id: String,
    visibility_mode: WindowVisibilityMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WaitForStateInput {
    instance_id: String,
    target_state: ProcessState,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WaitForCliInput {
    instance_id: String,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GameCliCallInput {
    instance_id: String,
    tool_name: String,
    #[serde(default = "empty_object")]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LogSessionListInput {
    instance_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LogQueryInput {
    session_id: String,
    severity: Option<String>,
    provider: Option<String>,
    event_name: Option<String>,
    contains: Option<String>,
    before_sequence: Option<i64>,
    limit: Option<u16>,
}

fn default_launch_mode() -> LaunchMode {
    LaunchMode::Offline
}

fn default_window_mode() -> WindowMode {
    WindowMode::Windowed
}

fn default_visibility_mode() -> WindowVisibilityMode {
    WindowVisibilityMode::Background
}

fn default_width() -> u16 {
    1280
}

fn default_height() -> u16 {
    720
}

fn default_exit_on_failure() -> bool {
    true
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn validated_timeout(timeout_ms: u64) -> AppResult<Duration> {
    if !(100..=600_000).contains(&timeout_ms) {
        return Err(AppError::validation(
            "Timeout must be between 100 and 600000 milliseconds.",
        ));
    }
    Ok(Duration::from_millis(timeout_ms))
}

fn empty_object() -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{AppPaths, Database, SettingsService};
    use crate::modules::archive_transfer::ArchiveTransferService;
    use crate::modules::codex_terminal::CodexTerminalService;
    use crate::modules::desktop_cli::DesktopCliGameDependencies;
    use crate::modules::game_archives::ArchiveCatalogService;
    use crate::modules::game_connections::GameConnectionService;
    use crate::modules::grok_terminal::GrokTerminalService;
    use crate::modules::instances::InstanceService;
    use crate::modules::logs::LogService;
    use crate::modules::runtime_bridge::RuntimeBridgeService;
    use crate::modules::tasks::TaskService;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct Fixture {
        dispatcher: DesktopToolDispatcher,
        tasks: TaskService,
        codex_terminal: CodexTerminalService,
        grok_terminal: GrokTerminalService,
        root: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!("abya-desktop-cli-{}", Uuid::new_v4()));
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
        let settings = SettingsService::new(database.clone(), paths.default_workspace_root.clone());
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
        Fixture {
            dispatcher: DesktopToolDispatcher::new(DesktopCliDependencies::new(
                settings,
                tasks.clone(),
                codex_terminal.clone(),
                grok_terminal.clone(),
                DesktopCliGameDependencies::new(
                    instances,
                    runtime_bridge,
                    logs,
                    connections,
                    archives,
                    archive_transfers,
                ),
            )),
            tasks,
            codex_terminal,
            grok_terminal,
            root,
        }
    }

    fn result_json(result: ToolCallResult) -> Value {
        serde_json::from_str(result.content[0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    #[ignore = "Requires ABYA_CLI_SMOKE_PLAYER, ABYA_CLI_SMOKE_ARCHIVE and ABYA_CLI_SMOKE_OUTPUT"]
    fn real_player_cli_pipeline_with_both_provider_contexts() {
        let executable = std::env::var("ABYA_CLI_SMOKE_PLAYER").unwrap();
        let archive_source = PathBuf::from(std::env::var("ABYA_CLI_SMOKE_ARCHIVE").unwrap());
        let output = PathBuf::from(std::env::var("ABYA_CLI_SMOKE_OUTPUT").unwrap());
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(
            output.join("summary.json"),
            r#"{"success":false,"phase":"running"}"#,
        )
        .unwrap();
        let fixture = fixture();
        fn copy_archive(source: &std::path::Path, target: &std::path::Path) {
            std::fs::create_dir_all(target).unwrap();
            for item in std::fs::read_dir(source).unwrap() {
                let item = item.unwrap();
                let kind = item.file_type().unwrap();
                assert!(!kind.is_symlink());
                if kind.is_dir() {
                    copy_archive(&item.path(), &target.join(item.file_name()));
                } else {
                    std::fs::copy(item.path(), target.join(item.file_name())).unwrap();
                }
            }
        }
        let archive_path = fixture.root.join("IsolatedGameData/Archives/Smoke");
        copy_archive(&archive_source, &archive_path);
        let archive_path = archive_path.to_string_lossy().into_owned();
        let deps = &fixture.dispatcher.dependencies;
        let mut settings = deps.settings.get().unwrap();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        settings.game_gateway_port = listener.local_addr().unwrap().port();
        drop(listener);
        settings.lan_broadcast_enabled = false;
        deps.connections
            .set_handler(Arc::new(crate::GameConnectionCoordinator {
                instances: deps.instances.clone(),
                logs: deps.logs.clone(),
            }));
        deps.connections.start(&settings).unwrap();
        let task = fixture
            .tasks
            .create(TaskInput {
                title: "CLI migration smoke".into(),
                description: String::new(),
            })
            .unwrap();
        let archive = deps
            .archives
            .inspect_main(&PathBuf::from(&archive_path).join("Main.PBArc"))
            .unwrap();
        let codex = fixture
            .codex_terminal
            .create_conversation(&task.id, None)
            .unwrap();
        let context = json!({"provider":"codex","taskId":task.id,"conversationId":codex.id});
        let launched = fixture.dispatcher.call_with_context(
            Some(context.clone()),
            &Uuid::new_v4().to_string(),
            "game_instance_launch",
            json!({"taskId":task.id,"name":"CLI verification","executablePath":executable,
                "mode":"editor","exitOnFailure":false,"width":800,"height":600,
                "archive":{"archiveGuid":archive.archive_guid,"archivePath":archive_path,"archiveName":archive.archive_name,"levelGuid":"","levelName":""}}),
        );
        assert!(!launched.is_error, "{:?}", launched);
        let instance_id = result_json(launched)["id"].as_str().unwrap().to_string();
        struct StopInstance(InstanceService, String);
        impl Drop for StopInstance {
            fn drop(&mut self) {
                let _ = self.0.stop(&self.1);
            }
        }
        let _stop = StopInstance(deps.instances.clone(), instance_id.clone());
        let ready = deps
            .runtime_bridge
            .wait_for_cli(&instance_id, Duration::from_secs(120));
        if let Err(error) = &ready {
            panic!("CLI readiness failed: {error}");
        }
        let startup = std::time::Instant::now();
        loop {
            let report = deps.instances.read_launch_report(&instance_id).unwrap();
            std::fs::write(
                output.join("launch-report.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            assert_ne!(
                report.report["phase"], "failed",
                "Launch report: {:?}",
                report
            );
            if report.report["success"] == true && report.report["phase"] == "ready" {
                break;
            }
            assert!(
                startup.elapsed() < Duration::from_secs(120),
                "Archive readiness timed out"
            );
            std::thread::sleep(Duration::from_millis(250));
        }
        let catalog = deps.runtime_bridge.list_tools(&instance_id).unwrap();
        assert_eq!(catalog.as_array().unwrap().len(), 239);
        let lua = "--[[ABYA-LUA\n{\"schemaVersion\":1,\"apiVersion\":\"3.0\",\"name\":\"cli-smoke\",\"description\":\"Pure validation\",\"context\":\"automation\",\"side\":\"universal\",\"entry\":\"main\",\"requires\":[],\"parameters\":[],\"returns\":[{\"name\":\"value\",\"type\":\"Float\",\"required\":true,\"description\":\"Result\"}]}\nABYA-LUA]]\nfunction main(input) return 42 end";
        for (tool, arguments) in [
            ("read_me_first", json!({})),
            ("runtime_get_game_state", json!({})),
            ("lua_execute", json!({"script":lua})),
            ("ui_capture_screenshot", json!({"maxWidth":800})),
        ] {
            let result = fixture.dispatcher.call_with_context(
                Some(context.clone()),
                &Uuid::new_v4().to_string(),
                "game_runtime_call_tool",
                json!({"instanceId":instance_id,"toolName":tool,"arguments":arguments}),
            );
            std::fs::write(
                output.join(format!("{tool}.json")),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
            assert!(!result.is_error, "{tool}: {:?}", result);
            if tool == "ui_capture_screenshot" {
                let image = result
                    .content
                    .iter()
                    .find_map(|v| v["path"].as_str())
                    .expect("Screenshot file missing");
                std::fs::copy(image, output.join("runtime.png")).unwrap();
            }
        }
        let grok = fixture
            .grok_terminal
            .create_conversation(&task.id, None)
            .unwrap();
        let result = fixture.dispatcher.call_with_context(
            Some(json!({"provider":"grok","taskId":task.id,"conversationId":grok.id})),
            &Uuid::new_v4().to_string(),
            "game_runtime_call_tool",
            json!({"instanceId":instance_id,"toolName":"runtime_get_status","arguments":{}}),
        );
        assert!(!result.is_error, "Grok context: {:?}", result);
        let multiplayer = std::env::var("ABYA_CLI_SMOKE_MULTIPLAYER").as_deref() == Ok("1");
        if multiplayer {
            deps.instances.stop(&instance_id).unwrap();
            let mut peers: Vec<String> = Vec::new();
            let mut peer_guards = Vec::new();
            for mode in ["lan-host", "lan-client"] {
                let mut input = json!({"taskId":task.id,"name":mode,"executablePath":executable,
                    "mode":mode,"exitOnFailure":false,"width":800,"height":600});
                if mode == "lan-host" {
                    input["archive"] = json!({"archiveGuid":archive.archive_guid,"archivePath":archive_path,
                        "archiveName":archive.archive_name,"levelGuid":"","levelName":""});
                } else {
                    input["hostInstanceId"] = json!(peers[0]);
                }
                let launched = fixture.dispatcher.call_with_context(
                    Some(context.clone()),
                    &Uuid::new_v4().to_string(),
                    "game_instance_launch",
                    input,
                );
                assert!(!launched.is_error, "{mode}: {:?}", launched);
                let id = result_json(launched)["id"].as_str().unwrap().to_string();
                peer_guards.push(StopInstance(deps.instances.clone(), id.clone()));
                deps.runtime_bridge
                    .wait_for_cli(&id, Duration::from_secs(120))
                    .unwrap();
                let start = std::time::Instant::now();
                loop {
                    let report = deps.instances.read_launch_report(&id).unwrap();
                    std::fs::write(
                        output.join(format!("{mode}-launch.json")),
                        serde_json::to_vec_pretty(&report).unwrap(),
                    )
                    .unwrap();
                    assert_ne!(report.report["phase"], "failed", "{mode}: {:?}", report);
                    if report.report["success"] == true && report.report["phase"] == "ready" {
                        break;
                    }
                    assert!(
                        start.elapsed() < Duration::from_secs(120),
                        "{mode} readiness timeout"
                    );
                    std::thread::sleep(Duration::from_millis(250));
                }
                let state = fixture.dispatcher.call_with_context(
                    Some(context.clone()),
                    &Uuid::new_v4().to_string(),
                    "game_runtime_call_tool",
                    json!({"instanceId":id,"toolName":"runtime_get_game_state","arguments":{}}),
                );
                assert!(!state.is_error);
                let state_data = result_json(state);
                std::fs::write(
                    output.join(format!("{mode}-state.json")),
                    serde_json::to_vec_pretty(&state_data).unwrap(),
                )
                .unwrap();
                assert_eq!(state_data["context"]["clientConnected"], true);
                assert_eq!(state_data["context"]["networkGameplayReady"], true);
                if mode == "lan-host" {
                    assert_eq!(state_data["context"]["serverActive"], true);
                }
                peers.push(id);
            }
            assert_ne!(
                deps.instances.read(&peers[0]).unwrap().pid,
                deps.instances.read(&peers[1]).unwrap().pid
            );
            assert!(!deps.logs.list_sessions(Some(&peers[0])).unwrap().is_empty());
            assert!(!deps.logs.list_sessions(Some(&peers[1])).unwrap().is_empty());
        }
        std::fs::write(
            output.join("summary.json"),
            json!({"success":true,"capabilities":239,
            "providers":["codex","grok"],"pureLua":true,"screenshot":true,"multiplayer":multiplayer})
            .to_string(),
        )
        .unwrap();
        deps.connections.stop();
    }

    #[test]
    fn runtime_artifact_preview_cannot_escape_task_directory() {
        let fixture = fixture();
        let task = fixture
            .tasks
            .create(TaskInput {
                title: "Artifacts".into(),
                description: String::new(),
            })
            .unwrap();
        let root = std::path::PathBuf::from(&task.workspace_path).join("artifacts/runtime");
        std::fs::create_dir_all(&root).unwrap();
        let image = root.join("sample.png");
        std::fs::write(&image, b"image bytes").unwrap();
        let bridge = &fixture.dispatcher.dependencies.runtime_bridge;
        assert!(
            bridge
                .read_artifact(&task.id, &image.to_string_lossy())
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        let outside = std::path::PathBuf::from(&task.workspace_path).join("private.png");
        std::fs::write(&outside, b"private").unwrap();
        assert!(
            bridge
                .read_artifact(&task.id, &outside.to_string_lossy())
                .is_err()
        );
        let script = root.join("script.html");
        std::fs::write(&script, b"html").unwrap();
        assert!(
            bridge
                .read_artifact(&task.id, &script.to_string_lossy())
                .is_err()
        );
    }

    #[test]
    fn validated_context_cannot_update_another_task() {
        let fixture = fixture();
        let task = fixture
            .tasks
            .create(TaskInput {
                title: "Owner".into(),
                description: String::new(),
            })
            .unwrap();
        let other = fixture
            .tasks
            .create(TaskInput {
                title: "Unchanged".into(),
                description: String::new(),
            })
            .unwrap();
        let conversation = fixture
            .codex_terminal
            .create_conversation(&task.id, None)
            .unwrap();
        let context = json!({"provider":"codex","taskId":task.id,"conversationId":conversation.id});
        let result = fixture.dispatcher.call_with_context(
            Some(context.clone()),
            &Uuid::new_v4().to_string(),
            "development_task_update",
            json!({"taskId":other.id,"title":"Incorrect"}),
        );
        assert!(result.is_error);
        assert_eq!(fixture.tasks.get(&other.id).unwrap().title, "Unchanged");
        let bound = fixture.dispatcher.call_with_context(
            Some(context),
            &Uuid::new_v4().to_string(),
            "development_conversation_bind",
            json!({}),
        );
        assert!(!bound.is_error);
    }

    #[test]
    fn creates_and_lists_development_tasks() {
        let fixture = fixture();
        let created = fixture.dispatcher.call(
            "development_task_create",
            json!({ "title": "MCP Task", "description": "Created by test" }),
        );
        assert!(!created.is_error);
        let created = result_json(created);
        assert_eq!(created["title"], "MCP Task");

        let listed = fixture
            .dispatcher
            .call("development_task_list", json!({ "status": "active" }));
        assert!(!listed.is_error);
        let listed = result_json(listed);
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["id"], created["id"]);
    }

    #[test]
    fn reports_unknown_and_invalid_tool_calls_as_tool_errors() {
        let fixture = fixture();
        let unknown = fixture.dispatcher.call("missing_tool", json!({}));
        assert!(unknown.is_error);
        assert_eq!(result_json(unknown)["code"], "unknownDesktopCliTool");

        let invalid = fixture
            .dispatcher
            .call("development_task_create", json!({}));
        assert!(invalid.is_error);
        assert_eq!(result_json(invalid)["code"], "invalidToolArguments");
    }

    #[test]
    fn launch_validation_does_not_start_an_invalid_executable() {
        let fixture = fixture();
        let result = fixture.dispatcher.call(
            "game_instance_launch",
            json!({
                "taskId": "task",
                "name": "Invalid",
                "executablePath": fixture.root.join("missing.exe").to_string_lossy()
            }),
        );
        assert!(result.is_error);
        assert_eq!(result_json(result)["code"], "validation");
    }

    #[test]
    fn window_visibility_requires_a_running_managed_instance() {
        let fixture = fixture();
        let result = fixture.dispatcher.call(
            "game_instance_set_window_visibility",
            json!({
                "instanceId": "missing",
                "visibilityMode": "background"
            }),
        );
        assert!(result.is_error);
        assert_eq!(result_json(result)["code"], "notFound");
    }

    #[test]
    fn game_mcp_call_reports_missing_instance() {
        let fixture = fixture();
        let result = fixture.dispatcher.call(
            "game_runtime_call_tool",
            json!({
                "instanceId": "missing",
                "toolName": "runtime_log_server_get_state",
                "arguments": {}
            }),
        );
        assert!(result.is_error);
        assert_eq!(result_json(result)["code"], "notFound");
    }

    #[test]
    fn log_query_validates_limit_before_storage_access() {
        let fixture = fixture();
        let result = fixture.dispatcher.call(
            "game_log_query",
            json!({ "sessionId": "missing", "limit": 1001 }),
        );
        assert!(result.is_error);
        assert_eq!(result_json(result)["code"], "validation");
    }

    #[test]
    fn archive_transfer_tools_list_sources_and_validate_missing_targets() {
        let fixture = fixture();
        let sources = fixture
            .dispatcher
            .call("game_archive_transfer_source_list", json!({}));
        assert!(!sources.is_error);
        assert_eq!(result_json(sources), json!([]));

        let start = fixture.dispatcher.call(
            "game_archive_transfer_start",
            json!({
                "instanceId": "missing",
                "mainArchivePath": fixture.root.join("Main.PBArc").to_string_lossy()
            }),
        );
        assert!(start.is_error);
        assert_eq!(result_json(start)["code"], "validation");
    }

    #[test]
    fn binds_a_conversation_and_records_reported_and_automatic_activity() {
        let fixture = fixture();
        let task = fixture
            .tasks
            .create(TaskInput {
                title: "Observable".into(),
                description: String::new(),
            })
            .unwrap();
        let conversation = fixture
            .codex_terminal
            .create_conversation(&task.id, None)
            .unwrap();
        let session_id = "mcp-session";

        let bound = fixture.dispatcher.call_for_session(
            Some(session_id),
            "development_conversation_bind",
            json!({
                "taskId": task.id,
                "conversationId": conversation.id
            }),
        );
        assert!(!bound.is_error);
        assert_eq!(result_json(bound)["provider"], "codex");

        let reported = fixture.dispatcher.call_for_session(
            Some(session_id),
            "development_conversation_activity_report",
            json!({
                "status": "progress",
                "kind": "analysis",
                "summary": "Designing the game flow",
                "detail": "Reviewing the current archive structure"
            }),
        );
        assert!(!reported.is_error);

        let listed = fixture.dispatcher.call_for_session(
            Some(session_id),
            "game_instance_list",
            json!({ "taskId": task.id }),
        );
        assert!(!listed.is_error);

        let workflow = fixture
            .codex_terminal
            .workflow(&task.id, &conversation.id)
            .unwrap();
        let activities = &workflow.turns[0].activities;
        assert!(
            activities
                .iter()
                .any(|activity| activity.summary == "Designing the game flow")
        );
        let automatic = activities
            .iter()
            .find(|activity| activity.summary == "Using game_instance_list")
            .unwrap();
        assert_eq!(automatic.status, CodexActivityStatus::Completed);
        assert!(automatic.detail.contains("taskId"));
    }

    #[test]
    fn binds_and_records_activity_for_a_grok_conversation() {
        let fixture = fixture();
        let task = fixture
            .tasks
            .create(TaskInput {
                title: "Grok observable".into(),
                description: String::new(),
            })
            .unwrap();
        let conversation = fixture
            .grok_terminal
            .create_conversation(&task.id, None)
            .unwrap();
        let session_id = "grok-mcp-session";

        let bound = fixture.dispatcher.call_for_session(
            Some(session_id),
            "development_conversation_bind",
            json!({
                "provider": "grok",
                "taskId": task.id,
                "conversationId": conversation.id
            }),
        );
        assert!(!bound.is_error);
        assert_eq!(result_json(bound)["provider"], "grok");

        let listed = fixture.dispatcher.call_for_session(
            Some(session_id),
            "game_instance_list",
            json!({ "taskId": task.id }),
        );
        assert!(!listed.is_error);

        let workflow = fixture
            .grok_terminal
            .workflow(&task.id, &conversation.id)
            .unwrap();
        assert!(
            workflow.turns[0]
                .activities
                .iter()
                .any(|activity| activity.summary == "Using game_instance_list")
        );
    }

    #[test]
    fn tool_result_serialization_preserves_native_image_blocks() {
        let result = ToolCallResult {
            content: vec![
                json!({ "type": "text", "text": "{\"width\":1,\"height\":1}" }),
                json!({
                    "type": "image",
                    "mimeType": "image/png",
                    "data": "iVBORw0KGgo="
                }),
            ],
            is_error: false,
        };
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["content"][0]["type"], "text");
        assert_eq!(value["content"][1]["type"], "image");
        assert_eq!(value["content"][1]["mimeType"], "image/png");
    }
}
