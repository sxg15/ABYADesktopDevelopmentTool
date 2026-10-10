use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryCandidate {
    pub(super) id: String,
    started_at: String,
}

impl CodexTerminalService {
    pub fn recovery_candidates(&self, task_id: &str) -> AppResult<Vec<RecoveryCandidate>> {
        let task = self.tasks.get(task_id)?;
        let root = codex_sessions_root()
            .ok_or_else(|| AppError::validation("无法定位 Codex 会话目录。"))?;
        let mut found = native_session_records_in(&root, Path::new(&task.workspace_path));
        found.sort_by_key(|r| r.started_at);
        Ok(found
            .into_iter()
            .map(|r| RecoveryCandidate {
                id: r.id,
                started_at: r.started_at.to_rfc3339(),
            })
            .collect())
    }

    pub fn repair_binding(
        &self,
        task_id: &str,
        conversation_id: &str,
        native_id: &str,
    ) -> AppResult<CodexConversation> {
        let _guard = self.lifecycle_lock.lock();
        if self.sessions.lock().contains_key(conversation_id)
            || self.runtime_servers.lock().contains_key(conversation_id)
        {
            return Err(AppError::validation("请先暂停这个对话的 AI，再修复关联。"));
        }
        if !self
            .recovery_candidates(task_id)?
            .iter()
            .any(|c| c.id == native_id)
        {
            return Err(AppError::validation("候选会话不属于当前任务工作目录。"));
        }
        let task = self.tasks.get(task_id)?;
        let mut conversation = self.conversation(task_id, conversation_id)?;
        if conversation.archived {
            return Err(AppError::validation("请先恢复已归档对话。"));
        }
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        for entry in std::fs::read_dir(conversations_root(Path::new(&task.workspace_path)))? {
            let path = entry?.path().join(CONVERSATION_METADATA_FILE);
            if let Ok(bytes) = std::fs::read(path) {
                let other: CodexConversation = serde_json::from_slice(&bytes)?;
                if other.id != conversation_id
                    && other.native_session_id.as_deref() == Some(native_id)
                {
                    return Err(AppError::validation("该原生会话已关联另一条 APP 对话。"));
                }
            }
        }
        let codex = (self.resolver)().ok_or_else(|| AppError::validation("Codex 不可用。"))?;
        let server = self.ensure_app_server(&codex)?;
        if server.is_archived(native_id)? {
            return Err(AppError::validation("候选原生会话已归档。"));
        }
        server.ensure_idle(native_id)?;
        std::fs::copy(
            directory.join(CONVERSATION_METADATA_FILE),
            directory.join(format!(
                "conversation-before-repair-{}.json",
                Uuid::new_v4()
            )),
        )?;
        let old = conversation.native_session_id.clone();
        if old.as_deref() != Some(native_id) {
            // Preserve old projections, but never label their timings as belonging to the new ID.
            let backup_id = Uuid::new_v4();
            for file in [
                "workflow.json",
                "workflow-events.jsonl",
                "connection-events.jsonl",
            ] {
                let source = directory.join(file);
                if source.exists() {
                    std::fs::rename(
                        &source,
                        directory.join(format!("before-repair-{backup_id}-{file}")),
                    )?;
                }
            }
            let fresh = CodexWorkflowSnapshot::empty(
                task_id,
                conversation_id,
                CodexObservabilityStatus::Native,
                "关联已修复；旧活动投影已备份，统计从新关联开始记录。",
            );
            save_workflow(&directory, &fresh, "bindingRepaired")?;
        }
        conversation.native_session_id = Some(native_id.into());
        conversation.binding_version = 2;
        conversation.updated_at = Utc::now().to_rfc3339();
        conversation.native_sync_error = None;
        write_conversation(&directory, &conversation)?;
        self.thread_conversations
            .lock()
            .retain(|_, (_, id)| id != conversation_id);
        std::fs::write(
            directory.join(format!("recovery-{}.json", Uuid::new_v4())),
            serde_json::to_vec_pretty(&json!({
                "at":Utc::now().to_rfc3339(),"oldNativeSessionId":old,"newNativeSessionId":native_id,
                "note":"只修复 APP 关联；两条原生历史未合并、未删除。最新用户决定仍需参照恢复说明。"
            }))?,
        )?;
        Ok(conversation)
    }
}
