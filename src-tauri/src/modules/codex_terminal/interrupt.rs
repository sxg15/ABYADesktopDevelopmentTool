use super::*;

impl CodexTerminalService {
    /// Interrupt execution without detaching the TUI or revoking the next input's access.
    pub fn interrupt_turn(&self, task_id: &str, conversation_id: &str) -> AppResult<()> {
        self.conversation(task_id, conversation_id)?;
        self.cancelled.lock().insert(conversation_id.into());
        let _guard = self.lifecycle_lock.lock();
        let conversation = self.conversation(task_id, conversation_id)?;
        let native = conversation
            .native_session_id
            .as_deref()
            .filter(|_| !conversation.archived)
            .ok_or_else(|| AppError::validation("原会话未连接或已归档，未执行中断。"))?;
        let binding = self.thread_conversations.lock().get(native).cloned();
        if binding
            .as_ref()
            .is_none_or(|(task, id)| task != task_id || id != conversation_id)
        {
            return Err(AppError::validation("原会话关联已变化，请重新核对终端。"));
        }
        let server = self
            .runtime_servers
            .lock()
            .get(conversation_id)
            .cloned()
            .filter(|server| server.is_alive())
            .ok_or_else(|| AppError::validation("原会话未连接，无法确认当前执行是否已中断。"))?;
        let task = self.tasks.get(task_id)?;
        let workspace = Path::new(&task.workspace_path);
        self.queue_tasks
            .lock()
            .insert(conversation_id.into(), task_id.into());
        self.pause_queued_locked(conversation_id)?;
        diagnostics::record(
            workspace,
            conversation_id,
            "turn-interrupt-requested",
            Some(native),
        );
        if let Err(error) = server.interrupt_and_wait(native) {
            diagnostics::record(
                workspace,
                conversation_id,
                "turn-interrupt-failed",
                Some(native),
            );
            return Err(error);
        }
        let directory = conversation_directory(workspace, conversation_id);
        if let Some(mut request) = continuation_queue::read(&directory)?
            && request.status == "paused"
        {
            request.status = "interrupted".into();
            continuation_queue::save(&directory, &mut request)?;
        }
        diagnostics::record(
            workspace,
            conversation_id,
            "turn-interrupt-confirmed",
            Some(native),
        );
        Ok(())
    }
}
