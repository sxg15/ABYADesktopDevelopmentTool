use super::*;
use serde_json::{Value, json};

impl CodexTerminalService {
    #[cfg(test)]
    pub(crate) fn with_session_store(mut self, root: PathBuf) -> Self {
        self.desktop_sessions_root = Some(root);
        self
    }

    pub fn desktop_thread_bound(&self, id: &str) -> AppResult<bool> {
        let file = self.tasks.desktop_context_path(id)?;
        if !file.exists() {
            return Ok(false);
        }
        let saved: Value = serde_json::from_slice(&std::fs::read(file)?)?;
        let cid = saved["conversationId"]
            .as_str()
            .ok_or_else(|| AppError::validation("任务对话记录缺失。"))?;
        Uuid::parse_str(cid).map_err(AppError::internal)?;
        let chat = self.conversation(id, cid)?;
        Ok(!chat.archived && chat.native_session_id.is_some())
    }

    pub fn desktop_chat(&self, id: &str, prompt: &str) -> AppResult<String> {
        let gate = self.gate(id);
        let _guard = gate.normal.lock();
        if crate::foundation::cli_sessions::task_active(id) {
            return Err(AppError::validation(
                "此任务仍有旧版执行正在运行。请先结束该次执行，再接入桌面对话。",
            ));
        }
        let task = self.tasks.get_readonly(id)?;
        self.tasks
            .install_desktop_skills(Path::new(&task.workspace_path))?;
        let file = self.tasks.desktop_context_path(id)?;
        let chat = if file.exists() {
            let saved: Value = serde_json::from_slice(&std::fs::read(file)?)?;
            let cid = saved["conversationId"]
                .as_str()
                .ok_or_else(|| AppError::validation("任务对话记录缺失。"))?;
            Uuid::parse_str(cid).map_err(AppError::internal)?;
            self.conversation(id, cid)?
        } else {
            let mut chats: Vec<_> = self
                .list_conversations(id)?
                .into_iter()
                .filter(|c| !c.archived && c.native_session_id.is_some())
                .collect();
            if chats.len() == 1 {
                chats.remove(0)
            } else {
                self.create_conversation(id, Some(task.title.chars().take(60).collect()))?
            }
        };
        if chat.archived {
            return Err(AppError::validation(
                "此任务绑定的 Codex 对话已归档，请先在 Codex 中恢复。",
            ));
        }
        self.tasks.save_desktop_context(id,&json!({"taskId":id,"provider":"codex","conversationId":chat.id,"cli":crate::foundation::cli_environment::desktop_cli_path()}))?;
        if let Some(native) = chat.native_session_id {
            let native = Uuid::parse_str(&native).map_err(AppError::internal)?;
            return Ok(format!("codex://threads/{native}"));
        }
        let mut url = reqwest::Url::parse("codex://new").map_err(AppError::internal)?;
        url.query_pairs_mut()
            .append_pair("path", &task.workspace_path)
            .append_pair("prompt", prompt);
        Ok(url.into())
    }

    pub fn check_desktop_thread(&self, id: &str, cid: &str, native: &str) -> AppResult<()> {
        Uuid::parse_str(native).map_err(AppError::internal)?;
        let gate = self.gate(id);
        let _guard = gate.normal.lock();
        let context: Value =
            serde_json::from_slice(&std::fs::read(self.tasks.desktop_context_path(id)?)?)?;
        if context["conversationId"] != cid {
            return Err(AppError::validation("此对话没有绑定当前任务。"));
        }
        let mut chat = self.conversation(id, cid)?;
        if chat.archived {
            return Err(AppError::validation("对话已归档。"));
        }
        if let Some(bound) = chat.native_session_id {
            return if bound == native {
                Ok(())
            } else {
                Err(AppError::validation("请回到本任务绑定的对话。"))
            };
        }
        self.desktop_rollout(id, native)?;
        let task = self.tasks.get_readonly(id)?;
        chat.native_session_id = Some(native.to_owned());
        write_conversation(
            &conversation_directory(Path::new(&task.workspace_path), cid),
            &chat,
        )
    }

    fn desktop_rollout(&self, id: &str, native: &str) -> AppResult<PathBuf> {
        let task = self.tasks.get_readonly(id)?;
        let root = self
            .desktop_sessions_root
            .clone()
            .ok_or_else(|| AppError::validation("找不到 Codex 会话目录。"))?;
        let mut files = Vec::new();
        collect_native_session_files(&root, 0, &mut files);
        files
            .into_iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|v| v.to_str())
                    .is_some_and(|n| n.ends_with(&format!("{native}.jsonl")))
            })
            .find(|p| {
                read_native_session_record(p).is_some_and(|r| {
                    r.id == native && paths_equal(&r.cwd, Path::new(&task.workspace_path))
                })
            })
            .ok_or_else(|| {
                AppError::validation("请从 APP 打开的任务工作区发送首条消息，再读取任务。")
            })
    }

    pub fn check_user_message(
        &self,
        id: &str,
        native: &str,
        quote: &str,
        since: &str,
    ) -> AppResult<()> {
        let path = self.desktop_rollout(id, native)?;
        check_latest_message(BufReader::new(File::open(path)?), quote, since)
    }
}

fn check_latest_message(reader: impl BufRead, quote: &str, since: &str) -> AppResult<()> {
    let mut last = None;
    for line in reader.lines() {
        let line = line?;
        if !line.contains("\"user\"") && !line.contains("\"user_message\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let payload = &event["payload"];
        let text = match event["type"].as_str() {
            Some("response_item") if payload["type"] == "message" && payload["role"] == "user" => {
                payload["content"].as_array().map(|items| {
                    items
                        .iter()
                        .filter(|i| i["type"] == "input_text")
                        .filter_map(|i| i["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            }
            Some("event_msg") if payload["type"] == "user_message" => {
                payload["message"].as_str().map(str::to_owned)
            }
            _ => continue,
        };
        last = Some((
            event["timestamp"]
                .as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
            text,
        ));
    }
    if let Some((Some(at), Some(message))) = last
        && !quote.trim().is_empty()
        && message.trim() == quote.trim()
        && DateTime::parse_from_rfc3339(since).is_ok_and(|submitted| at >= submitted)
    {
        return Ok(());
    }
    Err(AppError::validation(
        "请使用绑定对话中、对应当前版本的最新策划原话。",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_decision_uses_only_latest_user_message_after_submission() {
        let time = "2026-11-12T12:00:00Z";
        let message = |role: &str, text: &str| {
            json!({"type":"response_item","timestamp":time,
            "payload":{"type":"message","role":role,"content":[{"type":"input_text","text":text}]}})
            .to_string()
        };
        let log = [
            message("user", "旧版通过"),
            message("assistant", "新版通过"),
            message("user", "请改布局"),
        ]
        .join("\n");
        assert!(check_latest_message(log.as_bytes(), "请改布局", time).is_ok());
        for stale in ["旧版通过", "新版通过"] {
            assert!(check_latest_message(log.as_bytes(), stale, time).is_err());
        }
        assert!(check_latest_message(log.as_bytes(), "请改布局", "2026-11-12T12:00:01Z").is_err());
    }
}
