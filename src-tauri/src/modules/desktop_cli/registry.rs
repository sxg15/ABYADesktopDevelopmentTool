use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliCommand {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
    annotations: ToolAnnotations,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolAnnotations {
    title: &'static str,
    read_only_hint: bool,
    destructive_hint: bool,
    idempotent_hint: bool,
    open_world_hint: bool,
}

pub(crate) fn tools() -> Vec<CliCommand> {
    vec![
        tool(
            "game_runtime_cancel",
            "Requests cancellation without implying rollback.",
            "Cancel runtime operation",
            object_schema(
                json!({"instanceId":{"type":"string"},"operationId":{"type":"string"}}),
                &["instanceId", "operationId"],
            ),
            reversible_write(),
        ),
        tool(
            "desktop_get_capabilities",
            "Returns the desktop CLI capability summary and registered tool names. Never returns authentication tokens.",
            "Get desktop capabilities",
            object_schema(json!({}), &[]),
            read_only(),
        ),
        tool(
            "development_task_list",
            "Lists persistent development tasks, optionally filtered by lifecycle status.",
            "List development tasks",
            object_schema(
                json!({
                    "status": {
                        "type": "string",
                        "enum": ["active", "completed", "archived"]
                    }
                }),
                &[],
            ),
            read_only(),
        ),
        tool(
            "development_task_create",
            "Creates an active development task. This is a reversible persistent write.",
            "Create development task",
            object_schema(
                json!({
                    "title": { "type": "string", "minLength": 1 },
                    "description": { "type": "string", "default": "" }
                }),
                &["title"],
            ),
            reversible_write(),
        ),
        tool(
            "development_task_update",
            "Updates a development task title and optionally its description. This is a reversible persistent write.",
            "Update development task",
            object_schema(
                json!({
                    "taskId": { "type": "string", "minLength": 1 },
                    "title": { "type": "string", "minLength": 1 },
                    "description": { "type": "string" }
                }),
                &["taskId", "title"],
            ),
            reversible_write(),
        ),
        tool(
            "development_task_set_status",
            "Sets a development task to active, completed, or archived. This is a reversible persistent write.",
            "Set development task status",
            object_schema(
                json!({
                    "taskId": { "type": "string", "minLength": 1 },
                    "status": {
                        "type": "string",
                        "enum": ["active", "completed", "archived"]
                    }
                }),
                &["taskId", "status"],
            ),
            reversible_write(),
        ),
        tool(
            "development_conversation_bind",
            "Binds this initialized CLI conversation context to one task-owned Codex or Grok conversation so subsequent desktop tool calls can be recorded in its active plan step.",
            "Bind development conversation",
            object_schema(
                json!({
                    "provider": {
                        "type": "string",
                        "enum": ["codex", "grok"],
                        "default": "codex"
                    },
                    "taskId": { "type": "string", "minLength": 1 },
                    "conversationId": { "type": "string", "minLength": 1 }
                }),
                &["taskId", "conversationId"],
            ),
            reversible_write(),
        ),
        tool(
            "development_conversation_activity_report",
            "Reports a sanitized semantic activity milestone for the conversation bound to this CLI context. Bind the conversation first.",
            "Report conversation activity",
            object_schema(
                json!({
                    "status": {
                        "type": "string",
                        "enum": ["started", "progress", "completed", "failed"]
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["analysis", "command", "fileChange", "mcp", "gameInstance", "test", "web", "agent", "other"]
                    },
                    "summary": { "type": "string", "minLength": 1, "maxLength": 240 },
                    "detail": { "type": "string", "maxLength": 2000 }
                }),
                &["status", "kind", "summary"],
            ),
            reversible_write(),
        ),
        tool(
            "game_instance_list",
            "Lists persisted game instances, optionally limited to one development task.",
            "List game instances",
            object_schema(
                json!({ "taskId": { "type": "string", "minLength": 1 } }),
                &[],
            ),
            read_only(),
        ),
        tool(
            "game_instance_get",
            "Returns persisted and live runtime details for one managed game instance.",
            "Get game instance",
            instance_id_schema(),
            read_only(),
        ),
        tool(
            "game_archive_list",
            "Discovers launchable ABYA archives and levels. Uses the configured default archive root when root is omitted.",
            "List game archives",
            object_schema(json!({ "root": { "type": "string", "minLength": 1 } }), &[]),
            read_only(),
        ),
        tool(
            "game_archive_transfer_target_list",
            "Lists connected managed and external game instances that advertise archive-transfer-v1.",
            "List archive transfer targets",
            object_schema(json!({}), &[]),
            read_only_open_world(),
        ),
        tool(
            "game_archive_transfer_source_list",
            "Lists valid Main.PBArc archive folders directly below the configured local save directory.",
            "List archive transfer sources",
            object_schema(json!({}), &[]),
            read_only(),
        ),
        tool(
            "game_archive_transfer_start",
            "Offers and transfers the complete folder containing Main.PBArc to a connected game. The game asks its user for confirmation before replacing an archive with the same GUID.",
            "Start archive transfer",
            object_schema(
                json!({
                    "instanceId": { "type": "string", "minLength": 1 },
                    "mainArchivePath": { "type": "string", "minLength": 1 }
                }),
                &["instanceId", "mainArchivePath"],
            ),
            destructive_open_world(),
        ),
        tool(
            "game_archive_transfer_get",
            "Returns persisted status, byte progress, package hash, and result details for one archive transfer.",
            "Get archive transfer",
            transfer_id_schema(),
            read_only(),
        ),
        tool(
            "game_archive_transfer_cancel",
            "Cancels one active archive transfer and asks the game to discard its staging data.",
            "Cancel archive transfer",
            transfer_id_schema(),
            destructive_open_world(),
        ),
        tool(
            "game_instance_launch",
            "Starts a managed game process with typed launch options. New launches default to a background window that remains rendered without activation. Omit executablePath to use desktop settings. Host requires archive; clientOnly requires a running Host in the same task. Raw command-line arguments are not accepted.",
            "Launch game instance",
            launch_schema(),
            process_create(),
        ),
        tool(
            "game_instance_set_window_visibility",
            "Shows a managed game window without activating it, or returns it to background mode. The instance must be running and windowed.",
            "Set game window visibility",
            window_visibility_schema(),
            reversible_write(),
        ),
        tool(
            "game_instance_stop",
            "Stops log collection, requests process-tree exit, force-terminates when needed, waits for termination, and reports the observed outcome.",
            "Stop game instance",
            instance_id_schema(),
            destructive(),
        ),
        tool(
            "game_instance_wait_for_state",
            "Waits until one game instance reaches the requested persisted process state or the bounded timeout expires.",
            "Wait for game process state",
            wait_for_state_schema(),
            read_only(),
        ),
        tool(
            "game_instance_wait_for_cli",
            "Waits until the selected managed instance Runtime CLI accepts initialization and returns non-secret server information.",
            "Wait for game Runtime CLI",
            wait_for_cli_schema(),
            read_only_open_world(),
        ),
        tool(
            "game_instance_get_launch_report",
            "Reads the bounded JSON startup report written by a desktop-managed game instance.",
            "Get game launch report",
            instance_id_schema(),
            read_only(),
        ),
        tool(
            "game_runtime_get_state",
            "Checks the selected managed instance loopback Runtime CLI and returns non-secret connection and server metadata.",
            "Get game CLI state",
            instance_id_schema(),
            read_only_open_world(),
        ),
        tool(
            "game_runtime_list_tools",
            "Lists tools advertised over the selected managed instance's authenticated loopback Runtime CLI.",
            "List game Runtime CLI tools",
            instance_id_schema(),
            read_only_open_world(),
        ),
        tool(
            "game_runtime_call_tool",
            "Calls one tool on the selected instance Runtime CLI and preserves all returned text and image file references. Inspect the tool schema first.",
            "Call game Runtime CLI tool",
            game_cli_call_schema(),
            unknown_game_mutation(),
        ),
        tool(
            "game_log_collection_start",
            "Resumes structured log batches from a connected managed or external game source.",
            "Start game log collection",
            instance_id_schema(),
            reversible_write_open_world(),
        ),
        tool(
            "game_log_collection_stop",
            "Pauses structured log batches from the selected connected source. Persisted events remain available.",
            "Stop game log collection",
            instance_id_schema(),
            reversible_write(),
        ),
        tool(
            "game_log_source_list",
            "Lists managed and external game log sources with current connection and latest-session state.",
            "List game log sources",
            object_schema(json!({}), &[]),
            read_only(),
        ),
        tool(
            "game_log_session_list",
            "Lists persisted runtime log sessions, optionally limited to one game instance.",
            "List game log sessions",
            object_schema(
                json!({ "instanceId": { "type": "string", "minLength": 1 } }),
                &[],
            ),
            read_only(),
        ),
        tool(
            "game_log_query",
            "Queries persisted structured log events for one log session, newest sequence first.",
            "Query game logs",
            object_schema(
                json!({
                    "sessionId": { "type": "string", "minLength": 1 },
                    "severity": { "type": "string" },
                    "provider": { "type": "string" },
                    "eventName": { "type": "string" },
                    "contains": { "type": "string" },
                    "beforeSequence": { "type": "integer", "minimum": 1 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 200 }
                }),
                &["sessionId"],
            ),
            read_only(),
        ),
    ]
}

pub(crate) fn tool_count() -> usize {
    tools().len()
}

pub(crate) fn tool_names() -> Vec<&'static str> {
    tools().into_iter().map(|tool| tool.name).collect()
}

fn tool(
    name: &'static str,
    description: &'static str,
    title: &'static str,
    input_schema: Value,
    annotations: ToolAnnotations,
) -> CliCommand {
    CliCommand {
        name,
        description,
        input_schema,
        annotations: ToolAnnotations {
            title,
            ..annotations
        },
    }
}

fn object_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn instance_id_schema() -> Value {
    object_schema(
        json!({ "instanceId": { "type": "string", "minLength": 1 } }),
        &["instanceId"],
    )
}

fn transfer_id_schema() -> Value {
    object_schema(
        json!({ "transferId": { "type": "string", "minLength": 1 } }),
        &["transferId"],
    )
}

fn launch_schema() -> Value {
    object_schema(
        json!({
            "taskId": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 },
            "executablePath": {
                "type": "string",
                "description": "Optional .exe path. Uses desktop settings when omitted."
            },
            "mode": {
                "type": "string",
                "enum": ["editor", "offline", "lan-host", "lan-client", "igp-hosted", "normal", "host", "clientOnly"],
                "default": "offline",
                "description": "normal, host, and clientOnly are deprecated compatibility aliases."
            },
            "windowMode": {
                "type": "string",
                "enum": ["windowed", "borderless", "fullscreen"],
                "default": "windowed"
            },
            "visibilityMode": {
                "type": "string",
                "enum": ["background", "visible"],
                "default": "background",
                "description": "Background keeps the Unity window rendered outside the display area and non-activating."
            },
            "width": { "type": "integer", "minimum": 640, "maximum": 7680, "default": 1280 },
            "height": { "type": "integer", "minimum": 480, "maximum": 4320, "default": 720 },
            "archive": {
                "type": "object",
                "properties": {
                    "archivePath": { "type": "string", "minLength": 1 },
                    "archiveGuid": { "type": "string", "minLength": 1 },
                    "archiveName": { "type": "string" },
                    "levelGuid": { "type": "string", "minLength": 1 },
                    "levelName": { "type": "string" }
                },
                "required": ["archivePath", "archiveGuid", "archiveName", "levelGuid", "levelName"],
                "additionalProperties": false
            },
            "hostInstanceId": { "type": "string", "minLength": 1 },
            "exitOnFailure": { "type": "boolean", "default": true },
            "language": { "type": "string", "maxLength": 32, "default": "" }
        }),
        &["taskId", "name"],
    )
}

fn window_visibility_schema() -> Value {
    object_schema(
        json!({
            "instanceId": { "type": "string", "minLength": 1 },
            "visibilityMode": {
                "type": "string",
                "enum": ["background", "visible"]
            }
        }),
        &["instanceId", "visibilityMode"],
    )
}

fn wait_for_state_schema() -> Value {
    object_schema(
        json!({
            "instanceId": { "type": "string", "minLength": 1 },
            "targetState": {
                "type": "string",
                "enum": ["launching", "running", "stopping", "exited", "failed", "interrupted", "unmanaged"]
            },
            "timeoutMs": {
                "type": "integer",
                "minimum": 100,
                "maximum": 600000,
                "default": 30000
            }
        }),
        &["instanceId", "targetState"],
    )
}

fn wait_for_cli_schema() -> Value {
    object_schema(
        json!({
            "instanceId": { "type": "string", "minLength": 1 },
            "timeoutMs": {
                "type": "integer",
                "minimum": 100,
                "maximum": 600000,
                "default": 30000
            }
        }),
        &["instanceId"],
    )
}

fn game_cli_call_schema() -> Value {
    object_schema(
        json!({
            "instanceId": { "type": "string", "minLength": 1 },
            "toolName": { "type": "string", "minLength": 1 },
            "arguments": { "type": "object", "default": {} }
        }),
        &["instanceId", "toolName"],
    )
}

fn hints(
    read_only_hint: bool,
    destructive_hint: bool,
    idempotent_hint: bool,
    open_world_hint: bool,
) -> ToolAnnotations {
    ToolAnnotations {
        title: "",
        read_only_hint,
        destructive_hint,
        idempotent_hint,
        open_world_hint,
    }
}

fn read_only() -> ToolAnnotations {
    hints(true, false, true, false)
}

fn read_only_open_world() -> ToolAnnotations {
    hints(true, false, true, true)
}

fn reversible_write() -> ToolAnnotations {
    hints(false, false, false, false)
}

fn reversible_write_open_world() -> ToolAnnotations {
    hints(false, false, false, true)
}

fn process_create() -> ToolAnnotations {
    hints(false, false, false, true)
}

fn destructive() -> ToolAnnotations {
    hints(false, true, false, false)
}

fn destructive_open_world() -> ToolAnnotations {
    hints(false, true, false, true)
}

fn unknown_game_mutation() -> ToolAnnotations {
    hints(false, true, false, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn tool_names_are_unique_and_schemas_are_objects() {
        let tools = tools();
        let names = tools.iter().map(|tool| tool.name).collect::<HashSet<_>>();
        assert_eq!(names.len(), tools.len());
        assert!(tools.len() >= 30);
        assert!(tools.iter().all(|tool| {
            tool.input_schema.get("type").and_then(Value::as_str) == Some("object")
                && tool.input_schema.get("properties").is_some()
        }));
    }
}
