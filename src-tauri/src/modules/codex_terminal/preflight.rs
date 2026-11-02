use super::*;

// MCP 在模型宿主内执行，不再通过工作区 Shell 启动 ABYA 自检进程。
#[cfg_attr(test, allow(dead_code))]
pub(super) fn check(
    _codex: &ResolvedCodex,
    cwd: &Path,
    task: &str,
    conversation: &str,
) -> AppResult<Option<AppError>> {
    if !cwd.is_dir() {
        return Err(AppError::validation("任务工作目录不存在。"));
    }
    let env = crate::foundation::mcp_sessions::environment("codex", task, conversation);
    let get = |key: &str| {
        env.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| AppError::validation("APP MCP 服务或会话授权未就绪。"))
    };
    let url = get("ABYA_MCP_URL")?;
    let token = get("ABYA_MCP_SESSION_TOKEN")?;
    let client = crate::foundation::mcp_http::blocking_client(Duration::from_secs(10))?;
    let response=client.post(&url).bearer_auth(&token).json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"ABYA preflight","version":"1"}}})).send().map_err(|_|AppError::validation("APP MCP 服务无法响应。"))?;
    if !response.status().is_success() {
        return Err(AppError::validation("APP MCP 连接授权未通过。"));
    }
    let session = response
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::validation("MCP 未返回会话身份。"))?
        .to_owned();
    let body: serde_json::Value = response.json().map_err(AppError::internal)?;
    if body["result"]["protocolVersion"] != "2025-11-25" {
        return Err(AppError::validation("MCP 协议版本不兼容。"));
    }
    let response = client
        .post(url)
        .bearer_auth(token)
        .header("Mcp-Session-Id", session)
        .header("MCP-Protocol-Version", "2025-11-25")
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .map_err(|_| AppError::validation("MCP 初始化确认失败。"))?;
    if !response.status().is_success() {
        return Err(AppError::validation("MCP 初始化确认被拒绝。"));
    }
    Ok(None)
}
