use super::*;
use crate::foundation::Database;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};

fn save(db: &Database, key: &str, value: &Value) -> AppResult<()> {
    db.with_connection(|c| {
        c.execute(
            "INSERT OR REPLACE INTO settings(key,value_json,updated_at) VALUES(?1,?2,?3)",
            rusqlite::params![key, value.to_string(), chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    })
}
fn discussion(path: &Path) -> AppResult<Vec<Value>> {
    let mut items = std::collections::VecDeque::new();
    for line in BufReader::new(std::fs::File::open(path)?).lines() {
        let line = line?;
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let p = &v["payload"];
        if v["type"] != "response_item" || p["type"] != "message" {
            continue;
        }
        let role = p["role"].as_str().unwrap_or("");
        if !["user", "assistant"].contains(&role) {
            continue;
        }
        let text = p["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| x["text"].as_str())
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        if text.starts_with("<environment_context>") || text.is_empty() {
            continue;
        }
        items.push_back(json!({"role":role,"text":text.chars().take(12000).collect::<String>()}));
        if items.len() > 40 {
            items.pop_front();
        }
    }
    Ok(items.into())
}

impl CodexTerminalService {
    pub fn migrate_chat(
        &self,
        db: &Database,
        id: &str,
        request: &str,
        open: impl Fn(&str) -> AppResult<()>,
    ) -> AppResult<String> {
        Uuid::parse_str(request).map_err(AppError::internal)?;
        let gate = self.gate(id);
        let _lock = gate.normal.lock();
        let task = self.tasks.get_readonly(id)?;
        let root = Path::new(&task.workspace_path);
        let context_file = self.tasks.desktop_context_path(id)?;
        let current: Value = serde_json::from_slice(&std::fs::read(&context_file)?)?;
        let key = format!("chat-migration/{id}/{request}");
        let saved: Option<String> = db.with_connection(|c| {
            Ok(c.query_row(
                "SELECT value_json FROM settings WHERE key=?1",
                [&key],
                |r| r.get(0),
            )
            .optional()?)
        })?;
        let mut job: Value = if let Some(s) = saved {
            serde_json::from_str(&s)?
        } else {
            let cid = current["conversationId"]
                .as_str()
                .ok_or_else(|| AppError::validation("请先打开本任务已有的 Codex 对话。"))?;
            let old = self.conversation(id, cid)?;
            let thread = old
                .native_session_id
                .as_deref()
                .ok_or_else(|| AppError::validation("原对话还未完成绑定。"))?;
            let history = discussion(&self.desktop_rollout(id, thread)?)?;
            let production = self.tasks.production_get(id)?;
            let file=self.tasks.save_chat_handoff(id,request,&json!({"taskId":id,"title":task.title,"description":task.description,"workspace":task.workspace_path,"previousThread":thread,"production":production,"recentDiscussion":history,"note":"历史讨论用于恢复上下文；阶段、批准和权限以当前任务服务与用户本轮指令为准。完整历史保留在旧对话。"}))?;
            let next = self.create_conversation(
                id,
                Some(format!(
                    "{} · 接续",
                    task.title.chars().take(40).collect::<String>()
                )),
            )?;
            let value = json!({"old":old.id,"oldThread":thread,"next":next.id,"handoff":file,"phase":"prepared"});
            save(db, &key, &value)?;
            value
        };
        if current["conversationId"] != job["old"] && current["conversationId"] != job["next"] {
            return Err(AppError::validation(
                "任务已绑定另一对话，请关闭迁移窗口后核对。",
            ));
        }
        let cli = (self.resolver)().ok_or_else(|| AppError::validation("找不到 Codex CLI。"))?;
        let server = AppServerHandle::start(&cli, Arc::new(|_| {}))?;
        let result = (|| {
            let old_id = job["oldThread"]
                .as_str()
                .ok_or_else(|| AppError::validation("缺少原对话 ID。"))?
                .to_owned();
            Uuid::parse_str(&old_id).map_err(AppError::internal)?;
            // Broken history can reject thread/read. The local rollout above still binds ownership.
            if let Ok(v) = server.request(
                "thread/read",
                json!({"threadId":old_id,"includeTurns":false}),
            ) && v["thread"]["status"]["type"] == "active"
            {
                return Err(AppError::validation(
                    "旧对话仍在执行，请在 Codex 停止后再迁移。",
                ));
            }
            let project = server.ensure_project(&task)?;
            let mut next = self.conversation(
                id,
                job["next"]
                    .as_str()
                    .ok_or_else(|| AppError::validation("缺少接续记录。"))?,
            )?;
            let instructions = format!(
                "{} 本对话接续旧任务。首先读取 {}，其中包含制作状态、已确认文档、问题与最近讨论，再通过 production get 核对当前进度。历史原话只作背景，不能重放为新授权。用户继续后沿用当前任务执行。",
                desktop_chat::DESKTOP_INSTRUCTIONS,
                job["handoff"].as_str().unwrap_or("")
            );
            if next.native_session_id.is_none() {
                let recent = discussion(&self.desktop_rollout(id, &old_id)?)?;
                self.tasks.save_chat_handoff(id,request,&json!({"taskId":id,"title":task.title,"description":task.description,"workspace":task.workspace_path,"previousThread":old_id,"production":self.tasks.production_get(id)?,"recentDiscussion":recent,"note":"历史讨论只作上下文，不代表本轮授权；继续前回读 production get。"}))?;
                if job["phase"] == "starting" {
                    return Err(AppError::validation(
                        "上次创建对话的结果未知，已保留迁移记录，请核对 Codex 后恢复。",
                    ));
                }
                job["phase"] = json!("starting");
                save(db, &key, &job)?;
                let started=server.request("thread/start",json!({"cwd":task.workspace_path,"projectId":project,"ephemeral":false,"model":"gpt-6-astra","config":{"model_reasoning_effort":"high"},"developerInstructions":instructions}))?;
                let native = started["thread"]["id"]
                    .as_str()
                    .ok_or_else(|| AppError::validation("Codex 未返回新对话。"))?;
                Uuid::parse_str(native).map_err(AppError::internal)?;
                let stored = (|| -> AppResult<()> {
                    if !desktop_chat::same_workspace(&started["thread"], root)? {
                        return Err(AppError::validation("新对话工作区不匹配。"));
                    }
                    next.native_session_id = Some(native.into());
                    write_conversation(&conversation_directory(root, &next.id), &next)?;
                    server.persist_developer_instructions(native, &instructions)?;
                    server.request(
                        "thread/settings/update",
                        json!({"threadId":native,"model":"gpt-6-astra","effort":"high"}),
                    )?;
                    job["phase"] = json!("created");
                    save(db, &key, &job)
                })();
                let released = server.request("thread/unsubscribe", json!({"threadId":native}));
                stored?;
                released?;
            }
            let native = next.native_session_id.as_deref().unwrap();
            server.organize_thread(&project, &task, &next, native)?;
            let verify = server.request(
                "thread/read",
                json!({"threadId":native,"includeTurns":false}),
            )?;
            if verify["thread"]["id"] != native
                || !desktop_chat::same_workspace(&verify["thread"], root)?
                || lifecycle::native_archived_state(&verify["thread"])?
            {
                return Err(AppError::validation("新对话尚未就绪，原对话已保留。"));
            }
            self.tasks.save_desktop_context(id,&json!({"taskId":id,"provider":"codex","conversationId":next.id,"preparedThread":native,"desktopLocator":crate::foundation::cli_environment::task_locator(id,root)?,"cli":crate::foundation::cli_environment::desktop_cli_path()}))?;
            if job["phase"] != "complete" {
                job["phase"] = json!("bound");
                save(db, &key, &job)?;
                open(&format!("codex://threads/{native}"))?;
                let mut archived = false;
                for attempt in 0..3 {
                    match server.request("thread/archive", json!({"threadId":old_id})) {
                        Ok(_) => {
                            archived = true;
                            break;
                        }
                        Err(e) if attempt < 2 && e.code == "codex_session_in_use" => {
                            std::thread::sleep(Duration::from_millis(500))
                        }
                        Err(e) if e.code == "codex_session_in_use" => {
                            return Ok(format!("codex://threads/{native}"));
                        }
                        Err(e) => return Err(e),
                    }
                }
                if !archived {
                    return Err(AppError::validation("新对话已就绪，旧对话归档尚未完成。"));
                }

                let mut old = self.conversation(id, job["old"].as_str().unwrap())?;
                old.archived = true;
                old.updated_at = chrono::Utc::now().to_rfc3339();
                write_conversation(&conversation_directory(root, &old.id), &old)?;
                job["phase"] = json!("complete");
                save(db, &key, &job)?;
            }
            Ok(format!("codex://threads/{native}"))
        })();
        let stopped = server.stop_confirmed();
        let link = result?;
        stopped?;
        Ok(link)
    }
}

impl CodexTerminalService {
    pub fn finish_pending_chat_archives(&self, db: &Database) -> AppResult<()> {
        let pending:Vec<(String,String)>=db.with_connection(|c|{
            let mut q=c.prepare("SELECT key,value_json FROM settings WHERE key LIKE 'chat-migration/%' AND json_extract(value_json,'$.phase')='bound' LIMIT 10")?;
            Ok(q.query_map([],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<Result<Vec<_>,_>>()?)
        })?;
        if pending.is_empty() {
            return Ok(());
        }
        let cli = (self.resolver)().ok_or_else(|| AppError::validation("Codex unavailable"))?;
        let server = AppServerHandle::start(&cli, Arc::new(|_| {}))?;
        let result = (|| -> AppResult<()> {
            for (key, raw) in pending {
                let Some(task) = key.split('/').nth(1) else {
                    continue;
                };
                let gate = self.gate(task);
                let Some(_guard) = gate.normal.try_lock() else {
                    continue;
                };
                let mut job: Value = serde_json::from_str(&raw)?;
                let old_id = job["oldThread"]
                    .as_str()
                    .ok_or_else(|| AppError::validation("迁移缺少原对话"))?;
                Uuid::parse_str(old_id).map_err(AppError::internal)?;
                let current = server.request(
                    "thread/read",
                    json!({"threadId":old_id,"includeTurns":false}),
                );
                if current
                    .as_ref()
                    .is_ok_and(|v| v["thread"]["status"]["type"] == "active")
                {
                    continue;
                }
                let archived = current
                    .as_ref()
                    .is_ok_and(|v| lifecycle::native_archived_state(&v["thread"]).unwrap_or(false));
                if !archived
                    && server
                        .request("thread/archive", json!({"threadId":old_id}))
                        .is_err()
                {
                    continue;
                }
                let mut old = self.conversation(task, job["old"].as_str().unwrap_or(""))?;
                old.archived = true;
                old.updated_at = chrono::Utc::now().to_rfc3339();
                let t = self.tasks.get_readonly(task)?;
                write_conversation(
                    &conversation_directory(Path::new(&t.workspace_path), &old.id),
                    &old,
                )?;
                job["phase"] = json!("complete");
                save(db, &key, &job)?;
            }
            Ok(())
        })();
        let stopped = server.stop_confirmed();
        result?;
        stopped
    }
}
