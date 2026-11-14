use super::no_questions_events::observe_native_turn;
use super::*;
use crate::foundation::Database;
use serde_json::Value;
use tauri_plugin_opener::OpenerExt;
static UI_GATE: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

fn load_runtime(db: &Database, task: &str) -> AppResult<Value> {
    use rusqlite::OptionalExtension;
    db.with_connection(|c| {
        let raw: Option<String> = c
            .query_row(
                "SELECT value_json FROM settings WHERE key=?1",
                [format!("no-questions-runtime/{task}")],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or(Value::Null))
    })
}
fn save_runtime(db: &Database, task: &str, value: &Value) -> AppResult<()> {
    db.with_connection(|c| {
        c.execute(
            "INSERT OR REPLACE INTO settings(key,value_json,updated_at) VALUES(?1,?2,?3)",
            rusqlite::params![
                format!("no-questions-runtime/{task}"),
                value.to_string(),
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    })
}
impl CodexTerminalService {
    pub fn set_no_questions(
        &self,
        db: &Database,
        app: &tauri::AppHandle,
        task: &str,
        enabled: bool,
        stop: bool,
    ) -> AppResult<crate::modules::tasks::ProductionView> {
        let _ui = UI_GATE.lock();
        let view = self.tasks.no_questions_mode(task, enabled)?;
        if !enabled || stop {
            if let Some(link) = self.prepared_desktop_link(task)? {
                let r = load_runtime(db, task)?;
                if r["phase"] == "sent" || r["phase"] == "running" || stop {
                    app.opener()
                        .open_url(link, None::<&str>)
                        .map_err(AppError::internal)?;
                    let _ =
                        self.no_questions_ui(task, "stop", "", r["heading"].as_str().unwrap_or(""));
                }
            }
            return Ok(view);
        }
        let current = load_runtime(db, task)?;
        if current["runId"] != view.record.as_ref().unwrap().no_questions["runId"] {
            save_runtime(
                db,
                task,
                &json!({"runId":view.record.as_ref().unwrap().no_questions["runId"],"phase":"new"}),
            )?;
        }
        Ok(view)
    }
    pub fn drive_no_questions(&self, db: &Database, app: &tauri::AppHandle) -> AppResult<()> {
        for task in self.tasks.no_questions_tasks()? {
            let r = self
                .tasks
                .production_get(&task)?
                .record
                .ok_or_else(|| AppError::validation("制作记录缺失"))?;
            let run = r.no_questions["runId"].as_str().unwrap_or("").to_owned();
            if let Err(e) = self.no_questions_step(db, app, &task, &run) {
                let _ = self.tasks.no_questions_status(
                    &task,
                    &run,
                    "blocked",
                    &format!("自动接续暂缓：{}", e.message),
                );
            }
        }
        Ok(())
    }
    fn no_questions_step(
        &self,
        db: &Database,
        app: &tauri::AppHandle,
        task: &str,
        run: &str,
    ) -> AppResult<()> {
        let r = self
            .tasks
            .production_get(task)?
            .record
            .ok_or_else(|| AppError::validation("制作记录缺失"))?;
        if r.no_questions["enabled"] != true || r.no_questions["runId"] != run {
            return Ok(());
        }
        if r.no_questions["status"] == "blocked"
            && r.no_questions["blockedRevision"].as_u64() == Some(r.revision)
        {
            return Ok(());
        }
        let mut runtime = load_runtime(db, task)?;
        if runtime["runId"] != run {
            runtime = json!({"runId":run,"phase":"new"});
        }
        if runtime["phase"] == "blocked"
            && runtime["retryAt"]
                .as_i64()
                .is_some_and(|t| Utc::now().timestamp() < t)
        {
            return Ok(());
        }
        let bound = self.prepared_desktop_link(task)?;
        let link = if let Some(link) = bound {
            link
        } else {
            self.desktop_chat(task)?
        };
        let thread = link.rsplit('/').next().unwrap();
        if runtime["thread"].as_str().is_some_and(|v| v != thread) {
            runtime = json!({"runId":run,"phase":"new"});
        }
        let cli = (self.resolver)().ok_or_else(|| AppError::validation("Codex 不可用"))?;
        let server = AppServerHandle::start(&cli, Arc::new(|_| {}))?;
        let result = (|| {
            let metadata = server.request(
                "thread/read",
                json!({"threadId":thread,"includeTurns":false}),
            )?;
            let turns = server.request(
                "thread/turns/list",
                json!({"threadId":thread,"limit":1,"itemsView":"notLoaded","sortDirection":"desc"}),
            )?;
            Ok::<_, AppError>((metadata, turns))
        })();
        let stopped = server.stop_confirmed();
        let (metadata, turns) = result?;
        stopped?;
        let heading = metadata["thread"]["name"].as_str().unwrap_or("").to_owned();
        let mut last = turns["data"]
            .as_array()
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or(Value::Null);
        if let Ok(path) = self.desktop_rollout(task, thread) {
            observe_native_turn(&path, &mut runtime)?;
            if runtime["nativeTurn"].is_string() {
                last["id"] = runtime["nativeTurn"].clone();
                last["status"] = runtime["nativeStatus"].clone();
            }
        }
        save_runtime(db, task, &runtime)?;
        let turn = last["id"].as_str().unwrap_or("");
        if metadata["thread"]["status"]["activeFlags"]
            .as_array()
            .is_some_and(|f| !f.is_empty())
        {
            self.tasks.no_questions_status(
                task,
                run,
                "blocked",
                "Codex 正在等待平台权限或用户处理，请在任务对话查看具体状态。",
            )?;
            return Ok(());
        }
        if metadata["thread"]["status"]["type"] == "active" || last["status"] == "inProgress" {
            runtime["heading"] = json!(heading);
            runtime["thread"] = json!(thread);
            runtime["phase"] = json!("running");
            runtime["turn"] = json!(turn);
            save_runtime(db, task, &runtime)?;
            self.tasks.no_questions_status(
                task,
                run,
                "running",
                "Codex 正在执行本项目，本轮将持续到新试玩版。",
            )?;
            return Ok(());
        }
        if ["sent", "dispatching"].contains(&runtime["phase"].as_str().unwrap_or(""))
            && runtime["previousTurn"] == turn
        {
            self.tasks.no_questions_status(
                task,
                run,
                "waiting",
                "指令已提交，正在等待 Codex 开始本轮；未重复发送。",
            )?;
            return Ok(());
        }
        if runtime["phase"] == "running" && last["status"] == "interrupted" {
            self.tasks.no_questions_mode(task, false)?;
            return Ok(());
        }
        if self.tasks.no_questions_finish_if_ready(task)? {
            return Ok(());
        }
        let task_record = self.tasks.get_readonly(task)?;
        let prompt=format!("[ABYA 不提问模式 {run} / {}]
APP 已记录本项目的自主执行授权。读取 .codex/skills/abya-desktop-workflow/SKILL.md 和 .codex/skills/abya-game-development-task/references/no-questions.md，回读 production get 核对 noQuestions.enabled 与 runId。使用已有输入自行补足合理假设，依次整理、审核需求/美术/方案并用 production respond 的 auto-review 登记 AI 自审。已授权本项目隔离 Player 预检与制作、测试、修复、打包。保持当前指定 Player 和用户需求范围，直到产出本轮新的试玩包并真实验证输入、结算和重开，正常关闭测试实例并恢复编辑档后调用 auto-finish。遇普通失败自行核对修复，产品假设写入文档。实际外部权限或缺少工作目标才用 auto-block 记录明确恢复条件，绝不伪造运行结果。用户关闭模式或停止后即停止自动推进。",Uuid::new_v4());
        let prompt = format!(
            "{prompt}\n任务名称：{}\n任务说明：{}",
            task_record.title, task_record.description
        );
        let prompt = if runtime["phase"] == "blocked" && runtime["previousTurn"] == turn {
            runtime["prompt"].as_str().unwrap_or(&prompt).to_owned()
        } else {
            prompt
        };
        let _ui = UI_GATE.lock();
        if self
            .tasks
            .production_get(task)?
            .record
            .is_none_or(|r| r.no_questions["enabled"] != true || r.no_questions["runId"] != run)
        {
            return Ok(());
        }
        // Persist before dispatch; an uncertain send must never be blindly repeated.
        runtime = json!({"runId":run,"thread":thread,"phase":"dispatching","previousTurn":turn,"heading":heading,"prompt":prompt,"at":Utc::now().to_rfc3339()});
        save_runtime(db, task, &runtime)?;
        app.opener()
            .open_url(&link, None::<&str>)
            .map_err(AppError::internal)?;
        match self.no_questions_ui(task, "send", &prompt, &heading) {
            Ok(()) => {
                runtime["phase"] = json!("sent");
                save_runtime(db, task, &runtime)?;
            }
            Err(e) => {
                runtime["phase"] = json!("blocked");
                runtime["retryAt"] = json!(Utc::now().timestamp() + 60);
                save_runtime(db, task, &runtime)?;
                return Err(e);
            }
        }
        self.tasks
            .no_questions_status(task, run, "running", "已发送本轮自动制作指令。")?;
        Ok(())
    }
    fn no_questions_ui(
        &self,
        task: &str,
        action: &str,
        prompt: &str,
        heading: &str,
    ) -> AppResult<()> {
        use std::os::windows::process::CommandExt;
        let task = self.tasks.get_readonly(task)?;
        let context: Value =
            serde_json::from_slice(&std::fs::read(self.tasks.desktop_context_path(&task.id)?)?)?;
        let conversation = self.conversation(
            &task.id,
            context["conversationId"]
                .as_str()
                .ok_or_else(|| AppError::validation("对话尚未绑定"))?,
        )?;
        let dir = std::env::temp_dir().join(format!("abya-auto-{}", Uuid::new_v4()));
        std::fs::create_dir(&dir)?;
        let result = (|| {
            let script = dir.join("dispatch.ps1");
            let payload = dir.join("request.json");
            std::fs::write(
                &script,
                format!("﻿{}", include_str!("no_questions_driver.ps1")),
            )?;
            std::fs::write(
                &payload,
                serde_json::to_vec(
                    &json!({"heading":if heading.is_empty(){format!("ABYA · {} · {}",task.title,conversation.title)}else{heading.into()},"action":action,"prompt":prompt}),
                )?,
            )?;
            let shell = PathBuf::from(
                std::env::var_os("SystemRoot")
                    .ok_or_else(|| AppError::validation("Windows 环境不可用"))?,
            )
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            let out = std::process::Command::new(shell)
                .creation_flags(CREATE_NO_WINDOW)
                .args(["-NoProfile", "-STA", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(script)
                .arg("-Payload")
                .arg(payload)
                .output()?;
            if !out.status.success() {
                return Err(AppError::validation(
                    String::from_utf8_lossy(&out.stderr)
                        .chars()
                        .take(400)
                        .collect::<String>(),
                ));
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(dir);
        result
    }
}
