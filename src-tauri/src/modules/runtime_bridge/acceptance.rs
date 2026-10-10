use super::*;
use crate::modules::instances::{
    ArchiveSelection, GameInstance, LaunchInstanceInput, LaunchMode, LaunchProfile, ProcessState,
    WindowMode, WindowVisibilityMode,
};
use crate::modules::tasks::AcceptanceSpec;

struct LaunchLease {
    task: String,
    busy: Arc<Mutex<std::collections::HashSet<String>>>,
}
impl Drop for LaunchLease {
    fn drop(&mut self) {
        self.busy.lock().remove(&self.task);
    }
}

fn validate_report(report: &Value, spec: &AcceptanceSpec) -> AppResult<()> {
    if report["success"] != true
        || report["phase"] != "ready"
        || report["archiveGuid"] != spec.archive_guid
        || report["levelGuid"] != spec.level_guid
    {
        return Err(AppError::validation(
            "启动报告未就绪，或存档/关卡与验收版本不一致。",
        ));
    }
    Ok(())
}

fn validate_lua(value: &Value) -> AppResult<()> {
    let items = value["items"]
        .as_array()
        .ok_or_else(|| AppError::validation("无法读取 Lua 初始化结果。"))?;
    if let Some(failed) = items.first() {
        return Err(AppError::validation(format!(
            "玩法脚本初始化失败：{}。请检查任务运行包及实例日志。",
            failed["scriptName"].as_str().unwrap_or("Lua")
        )));
    }
    Ok(())
}

impl RuntimeBridgeService {
    pub fn acceptance_status(&self, task: &str) -> AppResult<Value> {
        self.tasks.get(task)?;
        let current = self.acceptance_progress.lock().get(task).cloned();
        let mut last = current
            .or(self.tasks.acceptance_last(task)?)
            .unwrap_or(json!({"status":"not-started"}));
        if last["status"] == "ready" && !self.session_alive(task, &last) {
            last["status"] = json!("stopped");
            last["message"] = json!("验收实例已退出，可重新打开当前版本。");
        }
        Ok(last)
    }

    fn progress(&self, task: &str, session: &mut Value, status: &str, message: &str) {
        session["status"] = json!(status);
        session["message"] = json!(message);
        session["updatedAt"] = json!(chrono::Utc::now().to_rfc3339());
        self.acceptance_progress
            .lock()
            .insert(task.into(), session.clone());
    }

    fn session_alive(&self, task: &str, session: &Value) -> bool {
        session["instanceIds"].as_array().is_some_and(|ids| {
            !ids.is_empty()
                && ids.iter().all(|id| {
                    id.as_str().is_some_and(|id| {
                        self.instances.get(id).is_ok_and(|info| {
                            info.process_alive && info.instance.task_id.as_deref() == Some(task)
                        })
                    })
                })
        })
    }

    fn acceptance_read(&self, id: &str, tool: &str, input: Value) -> AppResult<Value> {
        let value = self.run(
            id,
            &["capability", "run", tool],
            Some(input),
            "acceptance",
            &uuid::Uuid::new_v4().to_string(),
            Duration::from_secs(25),
            Default::default(),
        )?;
        if value["success"] != true {
            return Err(AppError::validation(format!(
                "验收检查 {tool} 失败，请查看实例诊断。"
            )));
        }
        Ok(value["data"].clone())
    }

    fn check_instance(&self, id: &str, spec: &AcceptanceSpec, host: bool) -> AppResult<()> {
        self.wait_for_cli(id, Duration::from_secs(35))
            .map_err(|error| {
                self.instances
                    .read_launch_report(id)
                    .ok()
                    .filter(|r| r.report["phase"] == "failed")
                    .map(|r| {
                        AppError::validation(format!(
                            "游戏启动失败：{}",
                            r.report["error"].as_str().unwrap_or("请查看启动报告。")
                        ))
                    })
                    .unwrap_or(error)
            })?;
        let tools = self.list_tools(id)?;
        let mut required = vec!["runtime_get_game_state", "lua_runtime_list"];
        if spec.require_ui {
            required.push("custom_ui_runtime_list_roots");
        }
        required.extend(spec.required_tools.iter().map(String::as_str));
        for name in required {
            if !tools
                .as_array()
                .is_some_and(|items| items.iter().any(|v| v["name"] == name || v["id"] == name))
            {
                return Err(AppError::validation(format!(
                    "当前运行包缺少验收能力：{name}。"
                )));
            }
        }
        let start = Instant::now();
        loop {
            let report = self.instances.read_launch_report(id)?;
            let game = self.acceptance_read(id, "runtime_get_game_state", json!({}))?;
            let c = &game["context"];
            let networking = spec.seats == 1
                || (c["clientConnected"] == true
                    && c["serverActive"] == host
                    && c["networkGameplayReady"] == true);
            let ui = !spec.require_ui
                || self.acceptance_read(id, "custom_ui_runtime_list_roots", json!({}))?["roots"]
                    .as_array()
                    .is_some_and(|roots| !roots.is_empty());
            validate_lua(&self.acceptance_read(
                id,
                "lua_runtime_list",
                json!({"state":"Failed","limit":32}),
            )?)?;
            if report.exists
                && validate_report(&report.report, spec).is_ok()
                && c["rootInitialized"] == true
                && c["isTransitioning"] == false
                && networking
                && ui
            {
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(35) {
                return Err(AppError::validation(
                    "网络或玩法入口未就绪，请查看启动报告和 Lua 日志。",
                ));
            }
            std::thread::sleep(Duration::from_millis(350));
        }
    }

    fn launch_seat(
        &self,
        task: &str,
        spec: &AcceptanceSpec,
        seat: u8,
        host_id: Option<String>,
    ) -> AppResult<GameInstance> {
        self.instances.launch(LaunchInstanceInput {
            task_id: task.into(),
            name: format!(
                "验收 {} · {}",
                spec.version,
                if seat == 0 {
                    "Host / 单人".into()
                } else {
                    format!("Client {seat}")
                }
            ),
            executable_path: spec.executable_path.clone(),
            profile: LaunchProfile {
                mode: if spec.seats == 1 {
                    LaunchMode::Offline
                } else if seat == 0 {
                    LaunchMode::LanHost
                } else {
                    LaunchMode::LanClient
                },
                window_mode: WindowMode::Windowed,
                visibility_mode: WindowVisibilityMode::Visible,
                width: 1280,
                height: 720,
                exit_on_failure: true,
                language: String::new(),
                host_instance_id: host_id,
                archive: Some(ArchiveSelection {
                    archive_path: spec.archive_path.clone(),
                    archive_guid: spec.archive_guid.clone(),
                    archive_name: "任务验收存档".into(),
                    level_guid: spec.level_guid.clone(),
                    level_name: "验收关卡".into(),
                }),
            },
        })
    }

    pub fn start_acceptance(&self, task: &str) -> AppResult<Value> {
        if !self.acceptance_busy.lock().insert(task.into()) {
            return Err(AppError::validation(
                "验收窗口正在启动，请等待当前启动完成。",
            ));
        }
        let _lease = LaunchLease {
            task: task.into(),
            busy: self.acceptance_busy.clone(),
        };
        let mut session = json!({"id":uuid::Uuid::new_v4().to_string(),"taskId":task,
            "instanceIds":[],"startedAt":chrono::Utc::now().to_rfc3339()});
        self.progress(
            task,
            &mut session,
            "checking-version",
            "正在核对当前存档与运行包版本。",
        );
        let mut created = Vec::<String>::new();
        let result = (|| -> AppResult<()> {
            let spec = self.tasks.acceptance_spec(task)?;
            session["version"] = json!(spec.version);
            let last = self.tasks.acceptance_last(task)?;
            let reusable = last.as_ref().filter(|last| {
                last["version"] == spec.version
                    && last["instanceIds"]
                        .as_array()
                        .is_some_and(|ids| ids.len() == usize::from(spec.seats))
                    && self.session_alive(task, last)
            });
            let ids = if let Some(last) = reusable {
                let ids: Vec<String> = serde_json::from_value(last["instanceIds"].clone())?;
                for (seat, id) in ids.iter().enumerate() {
                    let current = self.instances.read(id)?;
                    if current.process_state != ProcessState::Running
                        || current.executable_path.as_deref().is_none_or(|p| {
                            std::fs::canonicalize(p).ok()
                                != std::fs::canonicalize(&spec.executable_path).ok()
                        })
                    {
                        return Err(AppError::validation(
                            "已有实例的运行包不匹配，请关闭该组验收实例后重试。",
                        ));
                    }
                    self.check_instance(id, &spec, seat == 0)?;
                    self.instances
                        .set_window_visibility(id, WindowVisibilityMode::Visible)?;
                }
                ids
            } else {
                if let Some(last) = last.as_ref()
                    && last["instanceIds"].as_array().is_some_and(|ids| {
                        ids.iter().any(|id| {
                            id.as_str().is_some_and(|id| {
                                self.instances.get(id).is_ok_and(|v| v.process_alive)
                            })
                        })
                    })
                {
                    return Err(AppError::validation(
                        "上一组验收仍有运行中的窗口，请先关闭该组再打开新版本。",
                    ));
                }
                for seat in 0..spec.seats {
                    self.progress(
                        task,
                        &mut session,
                        if seat == 0 {
                            "starting-host"
                        } else {
                            "joining-clients"
                        },
                        if seat == 0 {
                            "正在启动当前版本并检查玩法。"
                        } else {
                            "正在加入同一房间并检查玩法。"
                        },
                    );
                    let instance = self.launch_seat(task, &spec, seat, created.first().cloned())?;
                    created.push(instance.id.clone());
                    session["instanceIds"] = json!(created);
                    self.check_instance(&instance.id, &spec, seat == 0)?;
                }
                created.clone()
            };
            // Recheck every seat after all clients join; connection alone is not gameplay readiness.
            self.progress(
                task,
                &mut session,
                "checking-gameplay",
                "正在复核各端玩法初始化。",
            );
            for (seat, id) in ids.iter().enumerate() {
                self.check_instance(id, &spec, seat == 0)?;
            }
            session["instanceIds"] = json!(ids);
            session["status"] = json!("ready");
            session["message"] =
                json!("玩法入口与 Lua 初始化检查通过，可以人工试玩；尚不代表验收通过。");
            session["updatedAt"] = json!(chrono::Utc::now().to_rfc3339());
            self.tasks
                .acceptance_record(task, &spec.version, session.clone())?;
            Ok(())
        })();
        if let Err(error) = result {
            let mut cleanup_failures = Vec::new();
            for id in &created {
                if self.instances.stop(id).map_or(true, |s| s.process_alive) {
                    cleanup_failures.push(id.clone());
                }
            }
            session["cleanupFailures"] = json!(cleanup_failures);
            self.progress(task, &mut session, "failed", &error.message);
            if let Some(version) = session["version"].as_str().map(str::to_owned) {
                let _ = self
                    .tasks
                    .acceptance_record(task, &version, session.clone());
            }
            return Err(error);
        }
        self.acceptance_progress
            .lock()
            .insert(task.into(), session.clone());
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lua_failure_is_not_ready_even_if_the_player_connected() {
        assert!(
            validate_lua(&json!({"items":[{"scriptName":"cat-match-clock","state":"Failed"}]}))
                .is_err()
        );
        assert!(validate_lua(&json!({"items":[]})).is_ok());
        assert!(validate_lua(&json!({})).is_err());
    }
    #[test]
    fn launch_report_must_match_the_saved_archive_and_level() {
        let spec = AcceptanceSpec {
            version: "v1".into(),
            executable_path: String::new(),
            archive_path: String::new(),
            archive_guid: "a".into(),
            level_guid: "l".into(),
            seats: 2,
            required_tools: vec![],
            require_ui: true,
        };
        let report = json!({"success":true,"phase":"ready","archiveGuid":"a","levelGuid":"l"});
        assert!(validate_report(&report, &spec).is_ok());
        assert!(
            validate_report(
                &json!({"success":true,"phase":"ready","archiveGuid":"wrong","levelGuid":"l"}),
                &spec
            )
            .is_err()
        );
    }
}
