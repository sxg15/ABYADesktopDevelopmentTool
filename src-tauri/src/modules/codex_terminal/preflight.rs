use super::*;
use crate::foundation::{cli_environment, cli_sessions, cli_transport::Connection};
use serde_json::Value;
use std::process::Output;

// APP connectivity and a workspace's standalone sandbox probe are distinct.
// A failed sandbox probe must not lock the user out of native permission controls.
pub(super) fn check(
    codex: &ResolvedCodex,
    cwd: &Path,
    task: &str,
    conversation: &str,
) -> AppResult<Option<AppError>> {
    let environment = cli_sessions::environment("codex", task, conversation);
    let get = |key: &str| {
        environment
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    let connection = Connection::Pipe {
        name: get("ABYA_DESKTOP_PIPE")
            .ok_or_else(|| AppError::validation("APP 的 CLI 服务未启动，请到设置中启动服务。"))?,
        credential: get("ABYA_DESKTOP_SESSION_TOKEN")
            .ok_or_else(|| AppError::validation("当前对话连接授权未生成，请重新连接。"))?,
        server_pid: std::process::id(),
    };
    let response = connection.request(&json!({"version":1,"requestId":Uuid::new_v4().to_string(),
        "command":"doctor","arguments":{},"context":{"provider":"codex","taskId":task,"conversationId":conversation}}), Duration::from_secs(5))
        .map_err(|_| AppError::new("desktop_cli_unavailable", "APP 的 CLI 服务无法响应，请在设置中重启 CLI 服务后重新连接。", ""))?;
    if response["success"] != true {
        return Err(AppError::new(
            "desktop_cli_unhealthy",
            "APP 的 CLI 自检未通过，请在设置中检查 CLI 服务及运行包完整性。",
            "",
        ));
    }
    if !cli_environment::desktop_cli_path().is_file() {
        return Err(AppError::new(
            "desktop_cli_missing",
            "运行包缺少 abya-desktop.exe，请重新拉取完整 Publish 目录。",
            "",
        ));
    }
    if !cwd.is_dir() {
        return Err(AppError::new(
            "workspace_missing",
            "任务工作目录不存在，请恢复原目录后重新连接。",
            "",
        ));
    }
    let mut command = app_server::codex_process_command(codex);
    command
        .arg("sandbox")
        .arg(cli_environment::desktop_cli_path())
        .args(["doctor", "--json"])
        .current_dir(cwd)
        .envs(environment)
        .env("ABYA_DEVELOPMENT_TASK_ID", task)
        .env("ABYA_DEVELOPMENT_CONVERSATION_ID", conversation)
        .env("ABYA_DEVELOPMENT_PROVIDER", "codex")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(AppError::internal)?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(30) {
            let mut kill = Command::new("taskkill");
            kill.args(["/PID", &child.id().to_string(), "/T", "/F"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(windows)]
            std::os::windows::process::CommandExt::creation_flags(&mut kill, CREATE_NO_WINDOW);
            let _ = kill.status();
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Some(failure("sandbox_timeout", None, None)));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(classify(&child.wait_with_output()?))
}

fn failure(kind: &str, exit: Option<i32>, os: Option<i32>) -> AppError {
    let reason = match kind {
        "session_rejected" => "工作区沙箱中的连接授权或会话标识不匹配",
        "pipe_unavailable" => "工作区沙箱无法访问 APP 的连接通道",
        "workspace_bootstrap" => "工作区沙箱无法启动命令，请核对目录访问权限与 Codex 沙箱配置",
        "config_invalid" => "工作区或用户的 Codex 配置无法加载",
        "sandbox_timeout" => "工作区沙箱启动检查超时",
        "runtime_missing" => "工作区沙箱未找到完整的工具运行环境",
        _ => "工作区沙箱未返回有效的工具自检结果",
    };
    AppError::new(
        "workspace_preflight_warning",
        format!(
            "启动检查提示：{reason}。APP 服务自检已通过，终端可连接；请在终端检查原生提示与执行设置。工具能否执行仍由实际沙箱与审批决定。检查分类：{kind}；退出码：{}；系统错误码：{}。",
            exit.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
            os.map(|v| v.to_string()).unwrap_or_else(|| "—".into())
        ),
        json!({"method":"sandbox/doctor","probeCode":kind,"exitCode":exit,"osCode":os}).to_string(),
    )
}

fn classify(output: &Output) -> Option<AppError> {
    let value = serde_json::from_slice::<Value>(&output.stdout).unwrap_or_default();
    if output.status.success() && value["success"] == true {
        return None;
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    let os = [267, 5, 2].into_iter().find(|code| {
        stderr.contains(&format!("windows error {code})"))
            || stderr.contains(&format!("failed: {code} ("))
    });
    let kind = match value["error"]["code"].as_str() {
        Some("session_unauthorized" | "desktop_identity_mismatch" | "validation") => {
            "session_rejected"
        }
        Some("desktop_connection_unavailable" | "outcome_unknown") => "pipe_unavailable",
        _ if os.is_some() => "workspace_bootstrap",
        _ if stderr.contains("config") => "config_invalid",
        _ if value["data"]["runtimeCliInstalled"] == false => "runtime_missing",
        _ => "invalid_probe_response",
    };
    Some(failure(kind, output.status.code(), os))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::ExitStatusExt;
    fn output(exit: u32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(exit),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }
    #[test]
    fn workspace_failures_are_actionable_warnings_without_sensitive_output() {
        for (result, kind) in [
            (
                output(
                    4,
                    r#"{"success":false,"error":{"code":"session_unauthorized","message":"secret-canary"}}"#,
                    "",
                ),
                "session_rejected",
            ),
            (
                output(
                    1,
                    "",
                    "CreateProcessAsUserW failed: 267 (cwd secret-canary)",
                ),
                "workspace_bootstrap",
            ),
            (
                output(1, "", "Error parsing config secret-canary"),
                "config_invalid",
            ),
            (
                output(
                    6,
                    r#"{"success":false,"error":{"code":"desktop_connection_unavailable"}}"#,
                    "secret-canary",
                ),
                "pipe_unavailable",
            ),
            (
                output(1, "secret-canary", "unknown native failure"),
                "invalid_probe_response",
            ),
        ] {
            let warning = classify(&result).unwrap();
            assert_eq!(warning.code, "workspace_preflight_warning");
            assert!(warning.detail.contains(kind));
            assert!(!warning.to_string().contains("secret-canary"));
            assert!(warning.message.contains("沙箱与审批决定"));
        }
        assert!(classify(&output(0, r#"{"success":true}"#, "")).is_none());
    }
}
