use super::*;
use crate::modules::instances::{
    ArchiveSelection, LaunchInstanceInput, LaunchMode, LaunchProfile, WindowMode,
    WindowVisibilityMode,
};

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message, "preflight-report")
}

impl RuntimeBridgeService {
    /// 固定测试脚本仅能调用这些能力；不开放设计阶段的任意运行写入。
    fn preflight_call(
        &self,
        id: &str,
        tool: &str,
        input: Value,
        deadline: Instant,
    ) -> AppResult<Value> {
        if ![
            "runtime_get_game_state",
            "capability_describe",
            "editor_create_archive",
            "editor_load_archive",
            "editor_get_archive",
            "editor_save_archive",
            "editor_list_archives",
        ]
        .contains(&tool)
        {
            return Err(error("preflight_scope_denied", "不允许的预检操作。"));
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| error("preflight_timeout", "预检超过总时间预算，未自动重试。"))?;
        let response = self.run(
            id,
            &["capability", "run", tool],
            Some(input),
            "capability-preflight",
            &uuid::Uuid::new_v4().to_string(),
            remaining.min(Duration::from_secs(35)),
            Default::default(),
        )?;
        if response["success"] != true {
            return Err(error(
                "preflight_runtime_failed",
                &format!("{tool}: {}", response["data"]),
            ));
        }
        Ok(response["data"].clone())
    }

    fn preflight_progress(&self, p: &mut Value, phase: &str) -> AppResult<()> {
        p["phase"] = json!(phase);
        self.tasks
            .publish_capability_preflight(p["taskId"].as_str().unwrap(), p)
    }

    fn preflight_launch(
        &self,
        p: &mut Value,
        archive: Option<ArchiveSelection>,
    ) -> AppResult<String> {
        let input = LaunchInstanceInput {
            task_id: p["taskId"].as_str().unwrap().into(),
            name: format!("基础预检 · {}", p["runId"].as_str().unwrap()),
            executable_path: p["playerPath"].as_str().unwrap().into(),
            profile: LaunchProfile {
                mode: LaunchMode::Editor,
                window_mode: WindowMode::Windowed,
                visibility_mode: WindowVisibilityMode::Background,
                width: 1280,
                height: 720,
                archive,
                host_instance_id: None,
                exit_on_failure: false,
                language: String::new(),
            },
        };
        let data = PathBuf::from(p["dataRoot"].as_str().unwrap());
        let instance = if input.profile.archive.is_none() {
            self.instances.launch_bootstrap(input, data)?
        } else {
            self.instances.launch_with_data_root(input, Some(data))?
        };
        p["instances"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":instance.id,"pid":instance.pid}));
        self.tasks
            .publish_capability_preflight(p["taskId"].as_str().unwrap(), p)?;
        Ok(instance.id)
    }

    fn preflight_ready(&self, id: &str, deadline: Instant) -> AppResult<()> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| error("preflight_timeout", "预检等待超时。"))?;
        self.wait_for_cli(id, remaining.min(Duration::from_secs(40)))?;
        loop {
            let game = self.preflight_call(id, "runtime_get_game_state", json!({}), deadline)?;
            if game["context"]["rootInitialized"] == true
                && game["context"]["isTransitioning"] == false
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(error("preflight_timeout", "游戏初始化等待超时。"));
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    pub fn run_capability_preflight(
        &self,
        task: &str,
        reuse: bool,
        retry: bool,
    ) -> AppResult<Value> {
        let key = format!("capability-preflight:{task}");
        if !self.acceptance_busy.lock().insert(key.clone()) {
            return Err(error(
                "preflight_busy",
                "本任务预检正在执行，请读取当前记录。",
            ));
        }
        let result = (|| {
            let mut p = self.tasks.begin_capability_preflight(task, reuse, retry)?;
            if p["state"] != "running" {
                return Ok(p);
            }
            let outcome = if p["error"].is_null() {
                self.preflight_native(&mut p)
            } else {
                Err(error(
                    "preflight_setup_failed",
                    p["error"]["message"].as_str().unwrap_or("预检设置不可用。"),
                ))
            };
            match outcome {
                Ok(()) => {
                    p["state"] = json!(if reuse {
                        "workaround-verified"
                    } else {
                        "passed"
                    });
                    if reuse {
                        p["error"] = if p["routeIssue"].is_null() {
                            json!({"code":"preflight_existing_route","message":"本次验证使用已有任务存档替代新建；未证明原生新建能力恢复。"})
                        } else {
                            p["routeIssue"].clone()
                        };
                    }
                }
                Err(e) => {
                    p["state"] = json!("failed");
                    p["error"] = json!({"code":e.code,"message":e.message});
                }
            }
            p["finishedAt"] = json!(chrono::Utc::now().to_rfc3339());
            self.tasks.publish_capability_preflight(task, &p)?;
            self.cleanup_preflight(&mut p)?;
            Ok(p)
        })();
        self.acceptance_busy.lock().remove(&key);
        result
    }
}

impl RuntimeBridgeService {
    fn cleanup_preflight(&self, p: &mut Value) -> AppResult<()> {
        let task = p["taskId"].as_str().unwrap();
        let run = p["runId"].as_str().unwrap();
        let name = format!("基础预检 · {run}");
        let result = (|| -> AppResult<()> {
            let owned = self
                .instances
                .list(Some(task))?
                .into_iter()
                .filter(|i| i.name == name)
                .collect::<Vec<_>>();
            for i in &owned {
                if self.instances.get(&i.id)?.process_alive
                    && self.instances.stop(&i.id)?.process_alive
                {
                    return Err(error(
                        "preflight_cleanup_pending",
                        "测试进程尚未停止，未删除临时存档。",
                    ));
                }
            }
            // 重启后 live 表不代表 OS 状态；PID 仍存在时保守保留，不停止可能复用 PID 的进程。
            let pids = owned
                .iter()
                .filter_map(|i| i.pid)
                .map(sysinfo::Pid::from_u32)
                .collect::<Vec<_>>();
            let mut system = sysinfo::System::new();
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&pids),
                true,
                sysinfo::ProcessRefreshKind::nothing(),
            );
            if pids.iter().any(|pid| system.process(*pid).is_some()) {
                return Err(error(
                    "preflight_cleanup_pending",
                    "仍检测到测试 PID，等待退出后自动清理。",
                ));
            }
            self.tasks.cleanup_capability_preflight_files(task, run)
        })();
        p["cleanupError"] = result
            .as_ref()
            .err()
            .map(|e| json!(e.message))
            .unwrap_or(Value::Null);
        p["cleanup"] = match result {
            Ok(()) => json!({"state":"completed","at":chrono::Utc::now().to_rfc3339()}),
            Err(e) => {
                json!({"state":"pending","message":e.message,"at":chrono::Utc::now().to_rfc3339()})
            }
        };
        let attempt = p["cleanup"].clone();
        if !p["cleanupAttempts"].is_array() {
            p["cleanupAttempts"] = json!([]);
        }
        let attempts = p["cleanupAttempts"].as_array_mut().unwrap();
        if attempts.len() >= 32 {
            attempts.remove(0);
        }
        attempts.push(attempt);
        self.tasks
            .publish_capability_preflight(p["taskId"].as_str().unwrap(), p)
    }

    /// 周期核对中断或占用的预检；只清理，不重放任何游戏写入。
    pub fn reconcile_capability_preflights(&self) -> AppResult<()> {
        let mut first_error = None;
        for mut p in self.tasks.preflight_recovery_records()? {
            let Some(task) = p["taskId"].as_str() else {
                continue;
            };
            let key = format!("capability-preflight:{task}");
            if !self.acceptance_busy.lock().insert(key.clone()) {
                continue;
            }
            if p["state"] == "running" {
                p["state"] = json!("interrupted");
                p["error"] = json!({"code":"preflight_interrupted","message":"预检被中断，未把未知操作记为通过；测试进程退出后自动清理，之后可重新预检。"});
            }
            let result = self.cleanup_preflight(&mut p);
            self.acceptance_busy.lock().remove(&key);
            if let Err(e) = result {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl RuntimeBridgeService {
    fn preflight_native(&self, p: &mut Value) -> AppResult<()> {
        let task = p["taskId"].as_str().unwrap().to_owned();
        let run = p["runId"].as_str().unwrap().to_owned();
        let root = PathBuf::from(p["dataRoot"].as_str().unwrap());
        let deadline = Instant::now() + Duration::from_secs(180);
        let reused = p["reuseExisting"] == true;
        let selection = if reused {
            let copy = self.tasks.copy_preflight_existing_archive(&task, &run)?;
            let mut archives = crate::modules::game_archives::discover_archives(&copy)?;
            let a = archives
                .pop()
                .ok_or_else(|| error("preflight_archive_invalid", "替代存档不可解析。"))?;
            let level = a
                .levels
                .iter()
                .find(|l| l.is_start)
                .or(a.levels.first())
                .ok_or_else(|| error("preflight_archive_invalid", "替代存档没有关卡。"))?;
            Some(ArchiveSelection {
                archive_path: archive_creation::display_path(&copy),
                archive_guid: a.archive_guid,
                archive_name: a.archive_name,
                level_guid: level.level_guid.clone(),
                level_name: level.level_name.clone(),
            })
        } else {
            None
        };
        let id = self.preflight_launch(p, selection)?;
        self.preflight_progress(p, "runtime-connect")?;
        self.preflight_ready(&id, deadline)?;
        p["checks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"runtime-connect","status":"passed"}));
        if !reused {
            self.preflight_progress(p, "create-availability")?;
            let catalog = self.list_tools(&id)?;
            if !catalog.is_array() {
                return Err(error(
                    "preflight_runtime_failed",
                    "运行时未返回有效能力目录。",
                ));
            }
            for name in [
                "editor_create_archive",
                "editor_load_archive",
                "editor_get_archive",
                "editor_save_archive",
            ] {
                if !catalog
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["name"] == name || c["id"] == name)
                {
                    return Err(error(
                        "preflight_capability_missing",
                        &format!("当前包能力目录未提供 {name}，无法使用该自动初始化契约。"),
                    ));
                }
                let d = self.preflight_call(
                    &id,
                    "capability_describe",
                    json!({"capabilityId":name}),
                    deadline,
                )?;
                p["checks"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id":format!("describe:{name}"),"descriptor":d}));
                if d["available"] != true
                    && ["editor_create_archive", "editor_load_archive"].contains(&name)
                {
                    return Err(error(
                        "preflight_capability_unavailable",
                        &format!(
                            "当前 APP 指定包在主菜单未开放 {name}；仅确认该自动初始化入口不可用，未证明原生 UI 创建失败。"
                        ),
                    ));
                }
                if name == "editor_load_archive"
                    && d["inputSchema"]["properties"]["openStartLevel"]["type"] != "boolean"
                {
                    return Err(error(
                        "preflight_contract_missing",
                        "当前包缺少 openStartLevel 初始化契约。",
                    ));
                }
            }
            self.preflight_progress(p, "create-archive")?;
            let catalog = self.preflight_call(&id, "editor_list_archives", json!({}), deadline)?;
            let archives = catalog["archives"].as_array().ok_or_else(|| {
                error(
                    "preflight_isolation_unknown",
                    "无法读回存档目录，未派发创建。",
                )
            })?;
            if archives.iter().any(|a| {
                a["path"]
                    .as_str()
                    .and_then(|v| PathBuf::from(v).canonicalize().ok())
                    .is_none_or(|v| !v.starts_with(root.join("Archives")))
            }) {
                return Err(error(
                    "preflight_isolation_unknown",
                    "运行时枚举到了测试目录之外的存档，未派发创建或修改用户存档。",
                ));
            }
            let created = self.preflight_call(
                &id,
                "editor_create_archive",
                json!({"name":format!("预检-{run}"),"author":"ABYA 基础能力预检","introduce":run}),
                deadline,
            )?;
            let path = archive_creation::checked_archive(&root, &created["archive"])?;
            p["checks"].as_array_mut().unwrap().push(json!({"id":"create-archive","status":"passed","archive":created["archive"],"path":path}));
            self.preflight_call(&id,"editor_load_archive",json!({"archiveGuid":created["archive"]["guid"],"openStartLevel":true,"discardUnsavedChanges":false}),deadline)?;
        }
        self.preflight_progress(p, "save-archive")?;
        let loaded = self.preflight_call(&id, "editor_get_archive", json!({}), deadline)?;
        let archive = archive_creation::checked_archive(&root, &loaded)?;
        if loaded["archiveAndLevelLoaded"] != true {
            return Err(error("preflight_load_failed", "存档或初始关卡未真正加载。"));
        }
        let saved = self.preflight_call(&id, "editor_save_archive", json!({}), deadline)?;
        if saved["completed"] != true {
            return Err(error(
                "preflight_save_failed",
                "保存未返回 completed=true，不能视为成功。",
            ));
        }
        let disk: Value = serde_json::from_slice(&std::fs::read(archive.join("Main.PBArc"))?)?;
        if disk["Guid"] != loaded["guid"] {
            return Err(error(
                "preflight_save_failed",
                "保存文件与实际存档身份不一致。",
            ));
        }
        p["checks"].as_array_mut().unwrap().push(json!({"id":"save-archive","status":"passed","receipt":saved,"savedGuid":disk["Guid"],"savedName":disk["Name"]}));
        let selection = ArchiveSelection {
            archive_path: archive_creation::display_path(&archive),
            archive_guid: loaded["guid"].as_str().unwrap().into(),
            archive_name: disk["Name"].as_str().unwrap_or("").into(),
            level_guid: loaded["currentLevelGuid"]
                .as_str()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| error("preflight_load_failed", "没有初始关卡身份。"))?
                .into(),
            level_name: loaded["currentLevelName"].as_str().unwrap_or("").into(),
        };
        if self.instances.stop(&id)?.process_alive {
            return Err(error(
                "preflight_stop_failed",
                "测试实例尚未退出，未重复启动。",
            ));
        }
        self.preflight_progress(p, "cold-load")?;
        let cold = self.preflight_launch(p, Some(selection.clone()))?;
        self.preflight_ready(&cold, deadline)?;
        let actual = loop {
            let actual = self.preflight_call(&cold, "editor_get_archive", json!({}), deadline)?;
            if actual["archiveAndLevelLoaded"] == true {
                break actual;
            }
            if Instant::now() >= deadline {
                return Err(error(
                    "preflight_cold_load_failed",
                    "冷加载在预算内未就绪。",
                ));
            }
            std::thread::sleep(Duration::from_millis(300));
        };
        if actual["guid"] != selection.archive_guid
            || actual["currentLevelGuid"] != selection.level_guid
            || actual["archiveAndLevelLoaded"] != true
            || archive_creation::checked_archive(&root, &actual)? != archive
        {
            return Err(error(
                "preflight_cold_load_failed",
                "冷加载的存档、关卡或目录与保存结果不一致。",
            ));
        }
        let after: Value = serde_json::from_slice(&std::fs::read(archive.join("Main.PBArc"))?)?;
        if after["Name"] != disk["Name"] || after["Guid"] != disk["Guid"] {
            return Err(error(
                "preflight_cold_load_failed",
                "冷加载后保存内容不一致。",
            ));
        }
        p["checks"].as_array_mut().unwrap().push(json!({"id":"cold-load","status":"passed","archiveGuid":actual["guid"],"levelGuid":actual["currentLevelGuid"],"archivePath":archive}));
        let current = self.tasks.task_player_status(&task)?;
        if current["selectedHash"] != p["playerBuildHash"]
            || current["selectedPath"] != p["playerPath"]
        {
            return Err(error(
                "preflight_player_changed",
                "预检期间 APP 包发生变化，结果无效，请重新预检。",
            ));
        }
        Ok(())
    }
}
