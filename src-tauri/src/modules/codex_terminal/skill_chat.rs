use super::*;
use crate::foundation::Database;
use rusqlite::OptionalExtension;
use serde_json::json;

impl CodexTerminalService {
    pub fn skill_chat(&self, db: &Database, root: &Path, file: &str) -> AppResult<String> {
        let cwd = root
            .to_str()
            .ok_or_else(|| AppError::validation("无效的 Skill 工作目录。"))?;
        let key = format!("skill-maintenance/{cwd}/{file}");
        let gate = self.gate(&key);
        let _guard = gate.normal.lock();
        let cli = (self.resolver)().ok_or_else(|| AppError::validation("找不到 Codex CLI。"))?;
        let server = AppServerHandle::start(&cli, Arc::new(|_| {}))?;
        let result = (|| {
            let saved: Option<String> = db.with_connection(|c| {
                Ok(c.query_row(
                    "SELECT value_json FROM settings WHERE key=?1",
                    [&key],
                    |r| r.get(0),
                )
                .optional()?)
            })?;
            let project = server.ensure_named_project(
                &format!("skill-library/{cwd}"),
                "ABYA · Skill 维护",
                cwd,
            )?;
            if let Some(saved) = saved {
                let id: String = serde_json::from_str(&saved)?;
                Uuid::parse_str(&id).map_err(AppError::internal)?;
                match server.request("thread/read", json!({"threadId":id,"includeTurns":false})) {
                    Ok(v)
                        if v["thread"]["id"] == id
                            && desktop_chat::same_workspace(&v["thread"], root)?
                            && !lifecycle::native_archived_state(&v["thread"])? =>
                    {
                        if v["thread"]["projectId"] != project {
                            server.request(
                                "thread/metadata/update",
                                json!({"threadId":id,"projectId":project}),
                            )?;
                        }
                        return Ok(format!("codex://threads/{id}"));
                    }
                    Err(e) if e.code != "codex_session_missing" => return Err(e),
                    _ => {}
                }
            }
            let instructions = format!(
                "这是 ABYA Skill 的专属维护对话。工作区：{cwd}。维护目标：{file}。先读取目标 SKILL.md、相邻 library.json 和工作区开发约束。根据用户要求维护功能、适用场景、维护人和配套文件。保留已确认的项目规则，开始前核对源文件路径；不在此对话推进游戏任务。"
            );
            let thread=server.request("thread/start",json!({"cwd":cwd,"projectId":project,"ephemeral":false,"model":"gpt-6-astra","config":{"model_reasoning_effort":"high"},"developerInstructions":instructions}))?;
            let id = thread["thread"]["id"]
                .as_str()
                .ok_or_else(|| AppError::validation("Codex 未返回维护对话。"))?;
            Uuid::parse_str(id).map_err(AppError::internal)?;
            let prepared = (|| -> AppResult<()> {
                if !desktop_chat::same_workspace(&thread["thread"], root)? {
                    return Err(AppError::validation("维护对话工作目录不匹配。"));
                }
                server.persist_developer_instructions(id, &instructions)?;
                let name = Path::new(file)
                    .parent()
                    .and_then(Path::file_name)
                    .unwrap_or_default()
                    .to_string_lossy();
                server.request(
                    "thread/name/set",
                    json!({"threadId":id,"name":format!("Skill 维护 · {name}")}),
                )?;
                server.request(
                    "thread/settings/update",
                    json!({"threadId":id,"model":"gpt-6-astra","effort":"high"}),
                )?;
                db.with_connection(|c|{c.execute("INSERT OR REPLACE INTO settings(key,value_json,updated_at) VALUES(?1,?2,?3)",rusqlite::params![key,serde_json::to_string(id)?,chrono::Utc::now().to_rfc3339()])?;Ok(())})
            })();
            let release = server.request("thread/unsubscribe", json!({"threadId":id}));
            prepared?;
            release?;
            Ok(format!("codex://threads/{id}"))
        })();
        let stopped = server.stop_confirmed();
        let link = result?;
        stopped?;
        Ok(link)
    }
}
