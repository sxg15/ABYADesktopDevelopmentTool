use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionControl {
    pub request_id: String,
    pub operation: String,
    pub status: String,
    pub native_id: Option<String>,
    pub backend_pid: Option<u32>,
    pub terminal_pid: Option<u32>,
    pub workflow_turn_id: Option<String>,
    pub continuation_turn_id: Option<String>,
    pub turn_id: Option<String>,
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub native_code: Option<i64>,
    pub method: Option<String>,
    pub updated_at: String,
    pub owner: String,
    pub elapsed_ms: u64,
}

pub(super) fn read(directory: &Path) -> AppResult<Option<ExecutionControl>> {
    let path = directory.join("execution-control.json");
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&std::fs::read(path)?)?))
}

fn save(
    directory: &Path,
    task: &str,
    conversation: &str,
    record: &mut ExecutionControl,
    event: &str,
) -> AppResult<()> {
    record.updated_at = Utc::now().to_rfc3339();
    let temp = directory.join(format!("execution-control-{}.tmp", Uuid::new_v4()));
    std::fs::write(&temp, serde_json::to_vec(record)?)?;
    std::fs::rename(temp, directory.join("execution-control.json"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("connection-events.jsonl"))?;
    writeln!(
        file,
        "{}",
        json!({
            "at":record.updated_at, "event":event, "requestId":record.request_id,
            "taskId":task, "conversationId":conversation, "nativeSessionId":record.native_id,
            "backendPid":record.backend_pid, "turnId":record.turn_id, "operation":record.operation,
            "terminalPid":record.terminal_pid,"workflowTurnId":record.workflow_turn_id,
            "continuationTurnId":record.continuation_turn_id,
            "status":record.status, "errorCode":record.error_code, "nativeCode":record.native_code,
            "elapsedMs":record.elapsed_ms,
            "method":record.method, "appVersion":env!("CARGO_PKG_VERSION"),
            "releaseLabel":env!("ABYA_RELEASE_ID")
        })
    )?;
    Ok(())
}

impl CodexTerminalService {
    pub fn force_stop(&self, task: &str, conversation: &str) -> AppResult<()> {
        self.control_execution(task, conversation, "forceStop")
    }

    pub(super) fn control_execution(
        &self,
        task_id: &str,
        conversation_id: &str,
        operation: &str,
    ) -> AppResult<()> {
        self.conversation(task_id, conversation_id)?;
        if !self.stopping.lock().insert(conversation_id.into()) {
            return Err(AppError::validation("此会话正在停止，请等待结果。"));
        }
        self.cancelled.lock().insert(conversation_id.into());
        let result = self.control_owned(task_id, conversation_id, operation);
        self.stopping.lock().remove(conversation_id);
        result
    }

    fn control_owned(
        &self,
        task_id: &str,
        conversation_id: &str,
        operation: &str,
    ) -> AppResult<()> {
        let task = self.tasks.get(task_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let conversation = self.conversation(task_id, conversation_id)?;
        let mut record = ExecutionControl {
            request_id: Uuid::new_v4().to_string(),
            operation: operation.into(),
            status: "stopping".into(),
            native_id: conversation.native_session_id.clone(),
            backend_pid: self
                .runtime_servers
                .lock()
                .get(conversation_id)
                .and_then(AppServerHandle::pid),
            terminal_pid: self
                .sessions
                .lock()
                .get(conversation_id)
                .and_then(|s| s.state.lock().pid),
            workflow_turn_id: self
                .workflow(task_id, conversation_id)
                .ok()
                .and_then(|w| w.current_turn_id),
            continuation_turn_id: continuation_queue::read(&directory)?.and_then(|q| q.turn_id),
            turn_id: None,
            error_code: None,
            error: None,
            native_code: None,
            method: None,
            updated_at: String::new(),
            owner: self.process_epoch.clone(),
            elapsed_ms: 0,
        };
        save(
            &directory,
            task_id,
            conversation_id,
            &mut record,
            "execution-stop-requested",
        )?;
        let started = Instant::now();
        let result = (|| {
            let _guard = self
                .lifecycle_lock
                .try_lock_for(Duration::from_secs(2))
                .ok_or_else(|| {
                    AppError::new("codex_control_busy", "会话正在切换，尚未停止；请重试。", "")
                })?;
            self.control_locked(task_id, conversation_id, operation, &directory, &mut record)
        })();
        record.status = if result.is_err() {
            "failed"
        } else if operation == "interrupt" {
            "interrupted"
        } else {
            "stopped"
        }
        .into();
        if let Err(error) = &result {
            record.error_code = Some(error.code.clone());
            record.error = Some(format!(
                "自动续接已禁止，但当前执行尚未确认停止。{}",
                if error.message.contains("会话准备") {
                    "原生服务拒绝了停止请求，请重试或强制停止此会话。"
                } else {
                    &error.message
                }
            ));
            let detail: serde_json::Value = serde_json::from_str(&error.detail).unwrap_or_default();
            record.native_code = detail["nativeCode"].as_i64();
            record.method = detail["method"].as_str().map(str::to_owned);
        }
        record.elapsed_ms = started.elapsed().as_millis() as u64;
        save(
            &directory,
            task_id,
            conversation_id,
            &mut record,
            "execution-stop-finished",
        )?;
        result.map_err(|error| {
            AppError::new(
                error.code,
                record.error.clone().unwrap_or(error.message),
                format!(
                    "诊断编号：{}；方法：{}；原生错误码：{}",
                    record.request_id,
                    record.method.as_deref().unwrap_or("—"),
                    record
                        .native_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "—".into())
                ),
            )
        })
    }

    fn control_locked(
        &self,
        task: &str,
        conversation: &str,
        operation: &str,
        directory: &Path,
        record: &mut ExecutionControl,
    ) -> AppResult<()> {
        self.conversation(task, conversation)?;
        let server = self.runtime_servers.lock().get(conversation).cloned();
        record.backend_pid = server.as_ref().and_then(AppServerHandle::pid);
        if let Some(server) = &server {
            let native = record
                .native_id
                .as_deref()
                .ok_or_else(|| AppError::validation("会话缺少原生关联。"))?;
            if self
                .thread_conversations
                .lock()
                .get(native)
                .is_none_or(|(t, c)| t != task || c != conversation)
            {
                return Err(AppError::validation("原会话关联已变化，未执行停止。"));
            }
            if operation == "forceStop" {
                crate::foundation::cli_sessions::revoke("codex", conversation);
            } else if server.is_alive() {
                self.queue_tasks
                    .lock()
                    .insert(conversation.into(), task.into());
                self.pause_queued_locked(conversation)?;
                let native = native.to_string();
                server.interrupt_observed(&native, |event, turn| {
                    record.turn_id = Some(turn.into());
                    record.method = Some("turn/interrupt".into());
                    let _ = save(directory, task, conversation, record, event);
                })?;
            } else if operation == "interrupt" {
                return Err(AppError::validation(
                    "执行器已断开，无法确认轮次已中断；请暂停或强制停止此会话。",
                ));
            }
        } else if operation == "interrupt" {
            return Err(AppError::validation(
                "原会话未连接，无法确认当前执行已中断。",
            ));
        }
        self.queue_tasks
            .lock()
            .insert(conversation.into(), task.into());
        self.pause_queued_locked(conversation)?;
        if operation == "interrupt" {
            if let Some(mut q) = continuation_queue::read(directory)?
                && q.status == "paused"
            {
                q.status = "interrupted".into();
                continuation_queue::save(directory, &mut q)?;
            }
        } else {
            self.close_owned_executor(conversation)?;
        }
        Ok(())
    }

    pub(super) fn close_owned_executor(&self, conversation: &str) -> AppResult<()> {
        let server = self.runtime_servers.lock().get(conversation).cloned();
        if let Some(server) = &server {
            server.stop_confirmed()?;
        }
        let session = self.sessions.lock().remove(conversation);
        if let Some(session) = session
            && let Err(error) = disconnect_and_terminate(session.clone())
        {
            self.sessions.lock().insert(conversation.into(), session);
            return Err(error);
        }
        self.runtime_servers.lock().remove(conversation);
        crate::foundation::cli_sessions::revoke("codex", conversation);
        Ok(())
    }
}
