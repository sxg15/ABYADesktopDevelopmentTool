use super::*;
use serde_json::{Value, json};

pub(super) const DESKTOP_INSTRUCTIONS: &str = "这是 ABYA 开发 APP 创建的任务对话。开始制作前，读取本工作区 .codex/skills/abya-desktop-workflow/SKILL.md，通过 .codex/abya-desktop.json 中的 cli 核对任务 ID。文档确认、回退和修改意见由当前策划表达，按具体版本记录。";

pub(super) fn same_workspace(thread: &Value, expected: &Path) -> AppResult<bool> {
    let cwd = thread["cwd"]
        .as_str()
        .ok_or_else(|| AppError::validation("Codex 未报告对话工作目录。"))?;
    let actual = Path::new(cwd);
    if !actual.is_absolute() {
        return Err(AppError::validation("Codex 返回了无效的工作目录。"));
    }
    Ok(actual.canonicalize().is_ok_and(|p| {
        expected
            .canonicalize()
            .is_ok_and(|root| paths_equal(&p, &root))
    }))
}

impl CodexTerminalService {
    #[cfg(test)]
    pub(crate) fn with_session_store(mut self, root: PathBuf) -> Self {
        self.desktop_sessions_root = Some(root);
        self
    }

    pub fn prepared_desktop_link(&self, id: &str) -> AppResult<Option<String>> {
        let gate = self.gate(id);
        let _guard = gate.normal.lock();
        let file = self.tasks.desktop_context_path(id)?;
        if !file.exists() {
            return Ok(None);
        }
        let mut saved: Value = serde_json::from_slice(&std::fs::read(&file)?)?;
        let Some(native) = saved["preparedThread"].as_str() else {
            return Ok(None);
        };
        Uuid::parse_str(native).map_err(AppError::internal)?;
        let cid = saved["conversationId"]
            .as_str()
            .ok_or_else(|| AppError::validation("任务对话记录缺失。"))?;
        Uuid::parse_str(cid).map_err(AppError::internal)?;
        let chat = self.conversation(id, cid)?;
        if chat.archived || chat.native_session_id.as_deref() != Some(native) {
            return Ok(None);
        }
        let link = format!("codex://threads/{native}");
        let task = self.tasks.get_readonly(id)?;
        if saved["taskId"] != id {
            return Err(AppError::validation(
                "项目连接文件的任务 ID 不匹配。请重新打开任务对话。",
            ));
        }
        let cli = crate::foundation::cli_environment::desktop_cli_path();
        let current = saved["desktopLocator"].as_str().is_some_and(|locator| {
            crate::foundation::cli_environment::task_connection_is_current(
                locator,
                id,
                Path::new(&task.workspace_path),
            )
            .unwrap_or(false)
        });
        if !current || saved["cli"] != cli.to_string_lossy().as_ref() {
            saved["desktopLocator"] = json!(crate::foundation::cli_environment::task_locator(
                id,
                Path::new(&task.workspace_path)
            )?);
            saved["cli"] = json!(cli);
            self.tasks.save_desktop_context(id, &saved)?;
        }
        Ok(Some(link))
    }

    pub fn refresh_desktop_connections(&self) -> AppResult<()> {
        for task in self.tasks.list_readonly()? {
            if let Err(error) = self.prepared_desktop_link(&task.id) {
                eprintln!("Task connection refresh {}: {error}", task.id);
            }
        }
        Ok(())
    }

    pub fn desktop_chat(&self, id: &str) -> AppResult<String> {
        let gate = self.gate(id);
        let _guard = gate.normal.lock();
        let cli = (self.resolver)().ok_or_else(|| {
            AppError::validation("找不到 Codex CLI，无法建立任务对话。请检查 Codex 安装后重试。")
        })?;
        // A metadata server shared with legacy terminal sessions can retain a
        // native writer lease even after thread/unsubscribe. This short-lived
        // client never starts a model turn and exits before opening the link.
        let bootstrap = AppServerHandle::start(&cli, Arc::new(|_| {}))?;
        let prepared = self.prepare_desktop_chat(id, &bootstrap);
        let stopped = bootstrap.stop_confirmed();
        let link = prepared?;
        stopped?;
        Ok(link)
    }

    fn prepare_desktop_chat(&self, id: &str, server: &AppServerHandle) -> AppResult<String> {
        if crate::foundation::cli_sessions::task_active(id) {
            return Err(AppError::validation(
                "此任务仍有旧版执行正在运行，请先结束该次执行。",
            ));
        }
        let task = self.tasks.get_readonly(id)?;
        let workspace = Path::new(&task.workspace_path);
        self.tasks.install_desktop_skills(workspace)?;
        let file = self.tasks.desktop_context_path(id)?;
        let saved = if file.exists() {
            serde_json::from_slice(&std::fs::read(file)?)?
        } else {
            Value::Null
        };
        let mut chat = if let Some(cid) = saved["conversationId"].as_str() {
            Uuid::parse_str(cid).map_err(AppError::internal)?;
            self.conversation(id, cid)?
        } else {
            let mut candidates = self
                .stored_conversations(id)?
                .into_iter()
                .filter(|c| !c.archived && c.native_session_id.is_some());
            match (candidates.next(), candidates.next()) {
                (Some(chat), None) => chat,
                _ => self.create_conversation(id, Some(task.title.chars().take(60).collect()))?,
            }
        };
        let mut prepared = saved["preparedThread"].as_str();
        if let Some(native) = chat.native_session_id.as_deref() {
            Uuid::parse_str(native).map_err(AppError::internal)?;
            let reusable = match server.request(
                "thread/read",
                json!({"threadId":native,"includeTurns":false}),
            ) {
                Ok(metadata) => {
                    let thread = &metadata["thread"];
                    if thread["id"] != native {
                        return Err(AppError::validation("Codex 返回了其他对话，已停止打开。"));
                    }
                    !chat.archived
                        && same_workspace(thread, workspace)?
                        && !lifecycle::native_archived_state(thread)?
                }
                Err(error) if error.code == "codex_session_missing" => false,
                Err(error) => return Err(error),
            };
            if !reusable {
                chat = self.create_conversation(id, Some(task.title.chars().take(60).collect()))?;
                prepared = None;
            }
        }
        let project = server.ensure_project(&task)?;
        let locator = crate::foundation::cli_environment::task_locator(id, workspace)?;
        let context = |c: &CodexConversation, prepared: Option<&str>| {
            json!({
                "taskId":id,"provider":"codex","conversationId":c.id,
                "preparedThread":prepared,"desktopLocator":locator,"cli":crate::foundation::cli_environment::desktop_cli_path()
            })
        };
        if chat.native_session_id.is_none() {
            self.tasks.save_desktop_context(id, &context(&chat, None))?;
            let started = server.request(
                "thread/start",
                json!({"cwd":task.workspace_path,"projectId":project,"ephemeral":false,"model":"gpt-6-astra","config":{"model_reasoning_effort":"high"},"developerInstructions":DESKTOP_INSTRUCTIONS}),
            )?;
            let native = started["thread"]["id"]
                .as_str()
                .ok_or_else(|| AppError::validation("Codex 未返回新对话 ID。"))?;
            Uuid::parse_str(native).map_err(AppError::internal)?;
            let saved = (|| {
                if !same_workspace(&started["thread"], workspace)? {
                    return Err(AppError::validation(
                        "新对话工作目录与任务不同，已停止打开。",
                    ));
                }
                chat.native_session_id = Some(native.into());
                write_conversation(&conversation_directory(workspace, &chat.id), &chat)?;
                server.persist_developer_instructions(native, DESKTOP_INSTRUCTIONS)
            })();
            // thread/start subscribes the creating client as a writer. The desktop
            // can only resume once that subscription has been released.
            let released = server.request("thread/unsubscribe", json!({"threadId":native}));
            saved?;
            released?;
        }
        let native = chat.native_session_id.as_deref().unwrap();
        let mut meta = server.request(
            "thread/read",
            json!({"threadId":native,"includeTurns":false}),
        )?;
        if meta["thread"]["id"] != native || !same_workspace(&meta["thread"], workspace)? {
            return Err(AppError::validation(
                "Codex 返回的工作目录与任务不一致，请重新打开任务。",
            ));
        }
        if prepared != Some(native) || meta["thread"]["projectId"] != project {
            server.organize_thread(&project, &task, &chat, native)?;
            meta = server.request(
                "thread/read",
                json!({"threadId":native,"includeTurns":false}),
            )?;
            if meta["thread"]["id"] != native
                || !same_workspace(&meta["thread"], workspace)?
                || meta["thread"]["projectId"] != project
            {
                return Err(AppError::validation(
                    "Codex 未关联正确的任务项目，未打开对话。",
                ));
            }
        }
        self.tasks
            .save_desktop_context(id, &context(&chat, Some(native)))?;
        Ok(format!("codex://threads/{native}"))
    }

    #[cfg(test)]
    pub(crate) fn verify_desktop_resume(&self, native: &str, workspace: &Path) -> AppResult<()> {
        let cli = (self.resolver)().ok_or_else(|| AppError::validation("Missing test CLI"))?;
        let desktop = AppServerHandle::start(&cli, Arc::new(|_| {}))?;
        let result = (|| {
            let resumed = desktop.request(
                "thread/resume",
                json!({"threadId":native,"cwd":workspace,"excludeTurns":true}),
            )?;
            if resumed["thread"]["id"] != native || !same_workspace(&resumed["thread"], workspace)?
            {
                return Err(AppError::validation(
                    "Independent desktop client resumed another thread or directory.",
                ));
            }
            desktop.request("thread/unsubscribe", json!({"threadId":native}))?;
            Ok(())
        })();
        desktop.stop();
        result
    }

    #[cfg(test)]
    pub(crate) fn archive_desktop_fixture(
        &self,
        task: &crate::modules::tasks::DevelopmentTask,
        native: &str,
    ) {
        let server = self.ensure_app_server(&(self.resolver)().unwrap()).unwrap();
        let project = server.ensure_project(task).unwrap();
        server
            .request("thread/archive", json!({"threadId":native}))
            .unwrap();
        server
            .request("project/delete", json!({"projectId":project}))
            .unwrap();
        self.stop_all();
    }

    #[cfg(test)]
    pub(crate) fn fixture_desktop_chat(&self, id: &str, native: &str) -> AppResult<String> {
        let task = self.tasks.get_readonly(id)?;
        let native = native.to_owned();
        let server = AppServerHandle::for_rpc_test(move |method, params| {
            Ok(match method {
                "project/list" => {
                    json!({"data":[{"id":"test-project","roots":[{"path":task.workspace_path}]}]})
                }
                "thread/read" if params["threadId"] != native => {
                    json!({"thread":{"id":params["threadId"],"cwd":std::env::temp_dir(),"projectId":"previous-project","archived":false}})
                }
                "thread/start" | "thread/read" => {
                    json!({"thread":{"id":native,"cwd":task.workspace_path,"projectId":"test-project","archived":false}})
                }
                "thread/unsubscribe" => json!({"status":"unsubscribed"}),
                "thread/inject_items" | "thread/name/set" => json!({}),
                _ => return Err(format!("Unexpected RPC {method}")),
            })
        });
        self.prepare_desktop_chat(id, &server)
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
        let chat = self.conversation(id, cid)?;
        if chat.archived
            || chat.native_session_id.as_deref() != Some(native)
            || context["preparedThread"] != native
        {
            return Err(AppError::validation(
                "当前 Codex 对话未绑定这项任务。请在开发 APP 点击「打开任务对话」后继续。",
            ));
        }
        Ok(())
    }

    pub(super) fn desktop_rollout(&self, id: &str, native: &str) -> AppResult<PathBuf> {
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
                let kinds = payload
                    .pointer("/internal_chat_message_metadata_passthrough/content_item_kinds")
                    .and_then(Value::as_array);
                if kinds.is_some_and(|items| {
                    !items.is_empty()
                        && items
                            .iter()
                            .all(|k| k == "environments.environment_context")
                        && payload["content"]
                            .as_array()
                            .is_some_and(|content| content.len() == items.len())
                }) {
                    continue;
                }
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
        let mut system_date: Value = serde_json::from_str(&message(
            "user",
            "<environment_context>日期变更</environment_context>",
        ))
        .unwrap();
        system_date["payload"]["internal_chat_message_metadata_passthrough"] =
            json!({"content_item_kinds":["environments.environment_context"]});
        let dated = [message("user", "确认本版"), system_date.to_string()].join("\n");
        assert!(check_latest_message(dated.as_bytes(), "确认本版", time).is_ok());
        system_date["payload"]["internal_chat_message_metadata_passthrough"] =
            json!({"content_item_kinds":["user.text"]});
        let authored = [message("user", "确认本版"), system_date.to_string()].join("\n");
        assert!(check_latest_message(authored.as_bytes(), "确认本版", time).is_err());
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
