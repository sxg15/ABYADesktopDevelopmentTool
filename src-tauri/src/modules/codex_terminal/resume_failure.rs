use super::*;

// Parse untrusted native errors once; expose only fixed messages and numeric codes.
// Error data can contain environment values, commands and local paths.
pub(super) fn rpc_error(method: &str, raw: &str) -> AppError {
    let value = serde_json::from_str::<serde_json::Value>(raw).unwrap_or_default();
    let message = value["message"].as_str().unwrap_or(raw).to_lowercase();
    let (code, hint) = if message.contains("active writer") {
        (
            "codex_session_in_use",
            "原对话正被另一个 Codex 客户端占用。请先在该客户端停止当前回复并释放会话连接；若关闭面板后仍占用，请退出占用它的客户端，再点击“接回原对话”。",
        )
    } else if message.contains("timed out") || message.contains("timeout") {
        (
            "codex_request_timeout",
            "等待 Codex 响应超时。请检查 Codex 是否可用，再接回原对话。",
        )
    } else if message.contains("not found") || message.contains("no rollout found") {
        (
            "codex_session_missing",
            "Codex 未找到原会话。请核对当前系统用户与原会话历史，在会话信息中检查关联。",
        )
    } else if message.contains("archived") {
        (
            "codex_session_archived",
            "原会话已归档。请刷新会话列表并明确恢复后再连接。",
        )
    } else if message.contains("connection") || message.contains("channel") {
        (
            "codex_transport_failed",
            "Codex 后端连接已中断。请接回原对话；若再次失败，请检查 Codex 安装状态。",
        )
    } else {
        (
            "codex_native_request_failed",
            "Codex 拒绝了会话准备请求。请根据诊断信息检查原生客户端状态和执行环境。",
        )
    };
    AppError::new(
        code,
        hint,
        json!({"method":method,"nativeCode":value["code"].as_i64()}).to_string(),
    )
}

pub(super) struct OpenAttempt {
    id: String,
    started: Instant,
    pub stage: &'static str,
    pub server: Option<AppServerHandle>,
    pub warning: Option<AppError>,
}
impl OpenAttempt {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            started: Instant::now(),
            stage: "preflight",
            server: None,
            warning: None,
        }
    }

    pub fn finish(
        &self,
        task: &str,
        conversation: &CodexConversation,
        workspace: &Path,
        error: Option<&AppError>,
    ) -> Option<AppError> {
        let rpc = error
            .or(self.warning.as_ref())
            .and_then(|e| serde_json::from_str::<serde_json::Value>(&e.detail).ok())
            .unwrap_or_default();
        let entry = json!({"at":Utc::now().to_rfc3339(),"event":if error.is_some(){"open-failed"}else{"open-succeeded"},
            "requestId":self.id,"taskId":task,"conversationId":conversation.id,"nativeSessionId":conversation.native_session_id,
            "stage":self.stage,"elapsedMs":self.started.elapsed().as_millis(),"backendPid":self.server.as_ref().and_then(AppServerHandle::pid),
            "errorCode":error.map(|e|e.code.as_str()),"method":rpc["method"].as_str(),"nativeCode":rpc["nativeCode"].as_i64(),
            "warningCode":self.warning.as_ref().map(|e|e.code.as_str()),"probeCode":rpc["probeCode"].as_str(),"exitCode":rpc["exitCode"].as_i64(),"osCode":rpc["osCode"].as_i64(),
            "appVersion":env!("CARGO_PKG_VERSION")});
        let path =
            conversation_directory(workspace, &conversation.id).join("connection-events.jsonl");
        if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(path) {
            let _ = writeln!(file, "{entry}");
        }
        error.map(|error| {
            let message = if error.code == "internal" {
                "原生会话准备失败，请检查 Codex 执行环境。"
            } else {
                &error.message
            };
            AppError::new(
                &error.code,
                format!("{message} 历史与原会话关联已保留，未自动发送任务。"),
                format!(
                    "诊断编号：{}；阶段：{}；方法：{}；原生错误码：{}",
                    self.id,
                    self.stage,
                    rpc["method"].as_str().unwrap_or("—"),
                    rpc["nativeCode"]
                        .as_i64()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "—".into())
                ),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_errors_keep_category_method_code_but_drop_sensitive_text() {
        for (raw, code) in [
            (json!({"code":-32600,"message":"thread abc already has an active writer","data":{"token":"secret-canary"}}).to_string(), "codex_session_in_use"),
            ("Timed out waiting for Codex app-server.".into(), "codex_request_timeout"),
            (json!({"code":-32600,"message":"thread not found at C:/private/secret-canary"}).to_string(), "codex_session_missing"),
            (json!({"code":-32600,"message":"unknown failure secret-canary"}).to_string(), "codex_native_request_failed"),
        ] {
            let error = rpc_error("thread/resume", &raw);
            assert_eq!(error.code, code);
            assert!(error.detail.contains("thread/resume"));
            assert!(!error.to_string().contains("secret-canary"));
        }
    }

    #[test]
    fn conflicting_writer_never_creates_a_replacement_and_explicit_retry_keeps_identity() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let busy = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let seen = calls.clone();
        let occupied = busy.clone();
        let native = Uuid::new_v4().to_string();
        let expected = native.clone();
        let server = AppServerHandle::for_rpc_test(move |method, params| {
            seen.lock().push(method.to_string());
            assert_eq!(params["threadId"], expected);
            match method {
                // notLoaded is local observation, not evidence of global ownership.
                "thread/read" => Ok(json!({"thread":{"id":expected,"status":{"type":"notLoaded"},"turns":[]}})),
                "thread/resume" if occupied.load(std::sync::atomic::Ordering::SeqCst) => Err(json!({"code":-32600,"message":"thread already has an active writer","data":"secret-canary"}).to_string()),
                "thread/resume" => Ok(json!({"thread":{"id":expected}})),
                _ => panic!("Unexpected mutating RPC: {method}"),
            }
        });
        let root = std::env::temp_dir().join(format!("abya-resume-{}", Uuid::new_v4()));
        let cwd = root.to_str().unwrap();
        let error = server
            .resume_thread(&native, cwd, "instructions", "task", "conversation")
            .unwrap_err();
        assert_eq!(error.code, "codex_session_in_use");
        assert!(error.detail.contains("-32600"));
        assert_eq!(
            calls
                .lock()
                .iter()
                .filter(|m| *m == "thread/resume")
                .count(),
            1
        );
        busy.store(false, std::sync::atomic::Ordering::SeqCst);
        server
            .resume_thread(&native, cwd, "instructions", "task", "conversation")
            .unwrap();
        assert!(
            !calls
                .lock()
                .iter()
                .any(|m| matches!(m.as_str(), "thread/start" | "turn/start" | "thread/archive"))
        );
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
