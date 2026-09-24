use abya_desktop_development_tool_lib::foundation::cli_transport::Connection;
use serde_json::{Value, json};
use std::io::Read;
use std::time::Duration;

fn command(group: &str, action: &str) -> Option<&'static str> {
    Some(match (group, action) {
        ("doctor", "") => "doctor",
        ("capabilities", "") => "capabilities",
        ("task", "list") => "development_task_list",
        ("task", "create") => "development_task_create",
        ("task", "update") => "development_task_update",
        ("task", "status") => "development_task_set_status",
        ("conversation", "bind") => "development_conversation_bind",
        ("conversation", "report") => "development_conversation_activity_report",
        ("instance", "list") => "game_instance_list",
        ("instance", "get") => "game_instance_get",
        ("instance", "launch") => "game_instance_launch",
        ("instance", "stop") => "game_instance_stop",
        ("instance", "window") => "game_instance_set_window_visibility",
        ("instance", "wait") => "game_instance_wait_for_state",
        ("instance", "launch-report") => "game_instance_get_launch_report",
        ("runtime", "cancel") => "game_runtime_cancel",
        ("runtime", "status") => "game_runtime_get_state",
        ("runtime", "wait") => "game_instance_wait_for_cli",
        ("runtime", "list") => "game_runtime_list_tools",
        ("runtime", "run" | "describe" | "search") => "game_runtime_call_tool",
        ("desktop", "info") => "desktop_get_capabilities",
        ("archive", "list") => "game_archive_list",
        ("archive", "targets") => "game_archive_transfer_target_list",
        ("archive", "sources") => "game_archive_transfer_source_list",
        ("archive", "transfer") => "game_archive_transfer_start",
        ("archive", "get") => "game_archive_transfer_get",
        ("archive", "cancel") => "game_archive_transfer_cancel",
        ("log", "start") => "game_log_collection_start",
        ("log", "stop") => "game_log_collection_stop",
        ("log", "sources") => "game_log_source_list",
        ("log", "sessions") => "game_log_session_list",
        ("log", "query") => "game_log_query",
        _ => return None,
    })
}

fn execute() -> Result<Value, Box<dyn std::error::Error>> {
    let mut words = Vec::new();
    let mut input = None;
    let mut supplied_id = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => {}
            "--request-id" if supplied_id.is_none() => {
                supplied_id = Some(args.next().ok_or("Missing request ID")?);
            }
            "--input-file" if input.is_none() => {
                input = Some(args.next().ok_or("Missing input file")?)
            }
            "--help" => {
                return Ok(json!({"success":true,"data":{
                "usage":"abya-desktop <group> <action> --input-file FILE|- --json",
                "groups":["doctor","capabilities","task","conversation","instance","runtime","archive","log"],
                "context":"ABYA_DEVELOPMENT_TASK_ID, ABYA_DEVELOPMENT_CONVERSATION_ID, ABYA_DEVELOPMENT_PROVIDER",
                "schemas":"abya-desktop capabilities --json"}}));
            }
            "--version" => {
                return Ok(json!({"success":true,"version":"0.2.0","protocolVersion":1}));
            }
            value if value.starts_with('-') => return Err("Unknown or repeated option".into()),
            _ => words.push(arg),
        }
    }
    let first = words.first().map(String::as_str).unwrap_or("doctor");
    let second = words.get(1).map(String::as_str).unwrap_or("");
    if words.len() > 2 {
        return Err("Too many positional arguments".into());
    }
    let name = command(first, second).ok_or("Unknown command; use --help")?;
    let mut arguments: Value = if let Some(file) = input {
        let mut raw = String::new();
        if file == "-" {
            std::io::stdin()
                .take(4 * 1024 * 1024 + 1)
                .read_to_string(&mut raw)?;
        } else {
            std::fs::File::open(file)?
                .take(4 * 1024 * 1024 + 1)
                .read_to_string(&mut raw)?;
        }
        if raw.len() > 4 * 1024 * 1024 {
            return Err("Input exceeds 4 MiB".into());
        }
        serde_json::from_str(raw.trim_start_matches('\u{feff}'))?
    } else {
        json!({})
    };
    if !arguments.is_object() {
        return Err("Input must be a JSON object".into());
    }
    if first == "runtime" && matches!(second, "describe" | "search") {
        let key = if second == "describe" {
            "capabilityId"
        } else {
            "query"
        };
        let value = arguments
            .get(key)
            .cloned()
            .ok_or("Missing capabilityId or query")?;
        arguments = json!({"instanceId":arguments["instanceId"],
            "toolName":if second=="describe" {"capability_describe"} else {"capability_search"},
            "arguments":{key:value}});
    }
    let context = match (
        std::env::var("ABYA_DEVELOPMENT_TASK_ID"),
        std::env::var("ABYA_DEVELOPMENT_CONVERSATION_ID"),
    ) {
        (Ok(task), Ok(conversation)) => Some(json!({"taskId":task,"conversationId":conversation,
            "provider":std::env::var("ABYA_DEVELOPMENT_PROVIDER").unwrap_or("codex".into())})),
        _ => None,
    };
    let connection = Connection::discover()?;
    let request_id = supplied_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    uuid::Uuid::parse_str(&request_id)?;
    let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = interrupted.clone();
    let cancel_connection = connection.clone();
    let cancel_context = context.clone();
    let cancel_operation = request_id.clone();
    let instance = arguments["instanceId"].clone();
    ctrlc::set_handler(move || {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        if name == "game_runtime_call_tool" {
            let _ = cancel_connection.request(&json!({
                    "version":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":"game_runtime_cancel",
                    "context":cancel_context,"arguments":{"instanceId":instance,"operationId":cancel_operation}
                }), Duration::from_secs(8));
        }
    })?;
    if interrupted.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(
            json!({"schemaVersion":1,"requestId":request_id,"success":false,"error":{"code":"outcome_unknown"}}),
        );
    }
    let response = connection.request(&json!({
        "version":1,"requestId":request_id,"command":name,"arguments":arguments,"context":context
    }), Duration::from_secs(730))?;
    if response["success"] == true && response["schemaVersion"] != 1 {
        return Err(
            abya_desktop_development_tool_lib::foundation::AppError::new(
                "outcome_unknown",
                "Protocol version mismatch; verify state before retrying.",
                "",
            )
            .into(),
        );
    }
    if response
        .get("requestId")
        .is_some_and(|v| v.as_str() != Some(&request_id))
    {
        return Err(
            abya_desktop_development_tool_lib::foundation::AppError::new(
                "outcome_unknown",
                "Response identity mismatch; verify state before retrying.",
                "",
            )
            .into(),
        );
    }
    Ok(response)
}
fn main() {
    let result = execute().unwrap_or_else(|e| {
        let code = e
            .downcast_ref::<abya_desktop_development_tool_lib::foundation::AppError>()
            .map(|error| error.code.as_str())
            .unwrap_or("invalid_request");
        json!({"schemaVersion":1,"success":false,
        "error":{"code":code,"message":e.to_string()}})
    });
    println!("{result}");
    if result["success"] != true {
        let code = result
            .pointer("/error/code")
            .or_else(|| result.pointer("/data/errorCode"))
            .or_else(|| result.pointer("/data/code"))
            .and_then(Value::as_str)
            .unwrap_or("");
        std::process::exit(match code {
            "invalid_request" | "validation" | "invalidToolArguments" => 2,
            "notFound" | "instance_not_found" => 3,
            "unauthorized"
            | "session_unauthorized"
            | "session_scope_denied"
            | "desktop_identity_mismatch" => 4,
            "capability_unavailable" => 5,
            "outcome_unknown" | "timeout" => 7,
            _ => 6,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_commands() {
        assert_eq!(command("runtime", "mcp"), None);
    }
    #[test]
    fn routes_runtime_without_protocol_aliases() {
        assert_eq!(command("runtime", "run"), Some("game_runtime_call_tool"));
    }
}
