use super::*;
use crate::modules::instances::{
    ArchiveSelection, LaunchInstanceInput, LaunchMode, LaunchProfile, WindowMode,
    WindowVisibilityMode,
};

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message, "preflight-report")
}

fn preflight_response(tool: &str, response: Value) -> AppResult<Value> {
    if response["success"] == true {
        return Ok(response["data"].clone());
    }
    let data = &response["data"];
    if tool == "runtime_get_game_state"
        && response["success"] == false
        && data["errorCode"] == "runtime_not_ready"
        && data["executionState"] == "not_executed"
        && data["status"] == "initializing"
        && data["ready"] == false
    {
        return Err(error(
            "preflight_root_initializing",
            "游戏运行时正在初始化。",
        ));
    }
    Err(error(
        "preflight_runtime_failed",
        &format!("{tool}: {data}"),
    ))
}

fn root_ready(response: Value) -> AppResult<bool> {
    match preflight_response("runtime_get_game_state", response) {
        Ok(game) => Ok(game["context"]["rootInitialized"] == true
            && game["context"]["isTransitioning"] == false),
        Err(e) if e.code == "preflight_root_initializing" => Ok(false),
        Err(e) => Err(e),
    }
}

impl RuntimeBridgeService {
    /// 固定测试脚本仅能调用这些能力；不开放设计阶段的任意运行写入。
    fn preflight_call(&self, id: &str, tool: &str, input: Value) -> AppResult<Value> {
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
        let response = self.run(
            id,
            &["capability", "run", tool],
            Some(input),
            "capability-preflight",
            &uuid::Uuid::new_v4().to_string(),
            Duration::from_secs(35),
            Default::default(),
        )?;
        preflight_response(tool, response)
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

    fn preflight_ready(&self, id: &str) -> AppResult<()> {
        self.wait_for_cli(id, Duration::from_secs(40))?;
        self.wait_for_game_root(id, "capability-preflight", &Default::default())
    }
    pub(super) fn wait_for_game_root(
        &self,
        id: &str,
        session: &str,
        cancel: &Arc<AtomicBool>,
    ) -> AppResult<()> {
        let start = Instant::now();
        loop {
            if start.elapsed() >= Duration::from_secs(60) {
                return Err(error(
                    "preflight_root_timeout",
                    "CLI 已连接，但游戏根未在限时内就绪。",
                ));
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::validation("初始化已取消。"));
            }
            // Loading can briefly occupy the CLI after its first successful status.
            // Keep the established initialization deadline instead of failing a
            // separate five-second reconnect window inside that deadline.
            self.wait_for_cli(id, Duration::from_secs(60).saturating_sub(start.elapsed()))?;
            let response = self.run(
                id,
                &["capability", "run", "runtime_get_game_state"],
                Some(json!({})),
                session,
                &uuid::Uuid::new_v4().to_string(),
                Duration::from_secs(35),
                cancel.clone(),
            )?;
            if root_ready(response)? {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    #[cfg(test)]
    pub(crate) fn probe_archive_ui(
        &self,
        input: LaunchInstanceInput,
        data: PathBuf,
    ) -> AppResult<Value> {
        let witness = super::native_archive_ui::RootWitness::capture(&data)?;
        let instance = self
            .instances
            .launch_bootstrap(input.clone(), data.clone())?;
        let result = (|| {
            self.preflight_ready(&instance.id)?;
            self.create_from_native_form(
                &instance.id,
                &witness,
                super::native_archive_ui::ArchiveFormInput {
                    name: "原生界面空白建档验收",
                    author: "ABYA QA",
                    description: "隔离建档测试",
                },
                "native-menu-test",
                &Default::default(),
                || Ok(()),
            )
        })();
        self.instances.stop(&instance.id)?;
        let mut created = result?;
        let selection = super::native_archive_ui::ui_archive_selection(&created)?;
        let mut receipts = Vec::new();
        for save in [true, false] {
            let mut launch = input.clone();
            launch.profile.archive = Some(selection.clone());
            let instance = self
                .instances
                .launch_with_data_root(launch, Some(data.clone()))?;
            let checked = (|| {
                self.preflight_ready(&instance.id)?;
                let loaded = self.preflight_loaded_archive(&instance.id)?;
                archive_creation::checked_archive(&data, &loaded)?;
                if loaded["guid"] != selection.archive_guid
                    || loaded["currentLevelGuid"] != selection.level_guid
                {
                    return Err(error(
                        "preflight_load_failed",
                        "Cold load identity mismatch",
                    ));
                }
                if save
                    && self.preflight_call(&instance.id, "editor_save_archive", json!({}))?["completed"]
                        != true
                {
                    return Err(error(
                        "preflight_save_failed",
                        "Player did not complete save",
                    ));
                }
                Ok(json!({"guid":loaded["guid"],"level":loaded["currentLevelGuid"],"saved":save}))
            })();
            self.instances.stop(&instance.id)?;
            receipts.push(checked?);
        }
        created["launches"] = json!(receipts);
        Ok(created)
    }
    pub fn run_capability_preflight(
        &self,
        task: &str,
        reuse: bool,
        retry: bool,
        user_initiated: bool,
    ) -> AppResult<Value> {
        let key = format!("capability-preflight:{task}");
        if !self.acceptance_busy.lock().insert(key.clone()) {
            return Err(error(
                "preflight_busy",
                "本任务预检正在执行，请读取当前记录。",
            ));
        }
        let result = (|| {
            let mut p =
                self.tasks
                    .begin_capability_preflight_from(task, reuse, retry, user_initiated)?;
            if p["state"] != "running" {
                return Ok(p);
            }
            let reuse = p["reuseExisting"] == true;
            let maintenance = reuse && p["purpose"] == "maintenance";
            let outcome = if p["error"].is_null() {
                self.preflight_native(&mut p)
            } else {
                Err(error(
                    p["error"]["code"]
                        .as_str()
                        .unwrap_or("preflight_setup_failed"),
                    p["error"]["message"].as_str().unwrap_or("预检设置不可用。"),
                ))
            };
            match outcome {
                Ok(()) => {
                    p["state"] = json!(if maintenance {
                        "existing-verified"
                    } else if reuse {
                        "workaround-verified"
                    } else {
                        "passed"
                    });
                    if reuse && !maintenance {
                        p["error"] = if p["routeIssue"].is_null() {
                            json!({"code":"preflight_existing_route","message":"本次验证使用已有任务存档替代新建；未证明原生新建能力恢复。"})
                        } else {
                            p["routeIssue"].clone()
                        };
                    }
                }
                Err(e) => {
                    p["state"] = json!("failed");
                    p["error"] = json!({"code":e.code,"message":e.message,"detail":e.detail});
                    if let Some(id) = p["instances"]
                        .as_array()
                        .and_then(|list| list.last())
                        .and_then(|v| v["id"].as_str())
                        && let Ok(info) = self.instances.get(id)
                    {
                        p["instanceAtFailure"] = json!({"id":id,"pid":info.instance.pid,"state":info.instance.process_state,"alive":info.process_alive,"exitCode":info.instance.exit_code});
                    }
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
    /// 场景就绪不代表关卡加载已完成，保存和冷加载均核对实际存档加载标志。
    fn preflight_loaded_archive(&self, id: &str) -> AppResult<Value> {
        self.wait_for_loaded_archive(id, "capability-preflight", &Default::default())
    }
    pub(super) fn wait_for_loaded_archive(
        &self,
        id: &str,
        session: &str,
        cancel: &Arc<AtomicBool>,
    ) -> AppResult<Value> {
        let start = Instant::now();
        loop {
            if start.elapsed() >= Duration::from_secs(60) {
                return Err(error(
                    "preflight_archive_timeout",
                    "CLI 已连接，但存档和关卡未在限时内就绪。",
                ));
            }
            let response = self.run(
                id,
                &["capability", "run", "editor_get_archive"],
                Some(json!({})),
                session,
                &uuid::Uuid::new_v4().to_string(),
                Duration::from_secs(35),
                cancel.clone(),
            )?;
            let actual = preflight_response("editor_get_archive", response)?;
            if actual["archiveAndLevelLoaded"] == true
                && actual["isLoadingArchive"] != true
                && actual["isLoadingLevel"] != true
            {
                return Ok(actual);
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    fn preflight_native(&self, p: &mut Value) -> AppResult<()> {
        let task = p["taskId"].as_str().unwrap().to_owned();
        let run = p["runId"].as_str().unwrap().to_owned();
        let root = PathBuf::from(p["dataRoot"].as_str().unwrap());
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
        let witness = super::native_archive_ui::RootWitness::capture(&root)?;
        let mut id = self.preflight_launch(p, selection)?;
        self.preflight_progress(p, "runtime-connect")?;
        self.preflight_ready(&id)?;
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
            let mut native_ui = false;
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
                let d =
                    self.preflight_call(&id, "capability_describe", json!({"capabilityId":name}))?;
                p["checks"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id":format!("describe:{name}"),"descriptor":d}));
                if d["available"] != true
                    && ["editor_create_archive", "editor_load_archive"].contains(&name)
                {
                    if d["available"] == false && d["currentContext"]["mode"] == "MainMenu" {
                        native_ui = true;
                        continue;
                    }
                    return Err(error(
                        "preflight_capability_unavailable",
                        &format!(
                            "当前 APP 指定包在主菜单未开放 {name}；仅确认该自动初始化入口不可用，未证明原生 UI 创建失败。"
                        ),
                    ));
                }
                if !native_ui
                    && name == "editor_load_archive"
                    && d["inputSchema"]["properties"]["openStartLevel"]["type"] != "boolean"
                {
                    return Err(error(
                        "preflight_contract_missing",
                        "当前包缺少 openStartLevel 初始化契约。",
                    ));
                }
            }
            self.preflight_progress(p, "create-archive")?;
            if native_ui {
                p["initializationRoute"] = json!("native-creator-ui");
                self.preflight_progress(p, "native-create-form")?;
                let name = format!("预检-{run}");
                let created = self.create_from_native_form(
                    &id,
                    &witness,
                    super::native_archive_ui::ArchiveFormInput {
                        name: &name,
                        author: "ABYA 基础能力预检",
                        description: &run,
                    },
                    "capability-preflight",
                    &Default::default(),
                    || Ok(()),
                )?;
                let path = archive_creation::checked_archive(&root, &created["archive"])?;
                p["checks"].as_array_mut().unwrap().push(json!({"id":"create-archive","status":"passed","route":"native-creator-ui","archive":created["archive"],"path":path}));
                let selection = super::native_archive_ui::ui_archive_selection(&created)?;
                if self.instances.stop(&id)?.process_alive {
                    return Err(error(
                        "preflight_stop_failed",
                        "建档实例仍在运行，未启动第二个编辑器。",
                    ));
                }
                id = self.preflight_launch(p, Some(selection))?;
                self.preflight_ready(&id)?;
            } else {
                let catalog = self.preflight_call(&id, "editor_list_archives", json!({}))?;
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
            )?;
                let path = archive_creation::checked_archive(&root, &created["archive"])?;
                p["checks"].as_array_mut().unwrap().push(json!({"id":"create-archive","status":"passed","archive":created["archive"],"path":path}));
                self.preflight_call(&id,"editor_load_archive",json!({"archiveGuid":created["archive"]["guid"],"openStartLevel":true,"discardUnsavedChanges":false}))?;
            }
        }
        self.preflight_progress(p, "save-archive")?;
        let loaded = self.preflight_loaded_archive(&id)?;
        if !reused
            && p["checks"]
                .as_array()
                .and_then(|checks| checks.iter().find(|c| c["id"] == "create-archive"))
                .is_none_or(|c| c["archive"]["guid"] != loaded["guid"])
        {
            return Err(error(
                "preflight_load_failed",
                "打开的存档不是这次创建的存档。",
            ));
        }
        let archive = archive_creation::checked_archive(&root, &loaded)?;
        if loaded["archiveAndLevelLoaded"] != true {
            return Err(error("preflight_load_failed", "存档或初始关卡未真正加载。"));
        }
        let saved = self.preflight_call(&id, "editor_save_archive", json!({}))?;
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
        self.preflight_ready(&cold)?;
        let actual = self.preflight_loaded_archive(&cold)?;
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

#[cfg(test)]
mod readiness_tests {
    use super::*;
    #[test]
    fn only_the_unexecuted_readiness_probe_is_retryable_on_startup() {
        let mut response = json!({"success":false,"data":{"errorCode":"runtime_not_ready","executionState":"not_executed","status":"initializing","ready":false}});
        assert_eq!(
            preflight_response("runtime_get_game_state", response.clone())
                .unwrap_err()
                .code,
            "preflight_root_initializing"
        );
        assert_eq!(
            preflight_response("editor_create_archive", response.clone())
                .unwrap_err()
                .code,
            "preflight_runtime_failed"
        );
        response["data"]["executionState"] = json!("outcome_unknown");
        assert_eq!(
            preflight_response("runtime_get_game_state", response)
                .unwrap_err()
                .code,
            "preflight_runtime_failed"
        );
    }
}

#[cfg(test)]
mod native_startup_tests {
    use super::*;

    #[test]
    fn native_startup_rejections_wait_until_root_is_ready() {
        for phase in ["starting_runtime", "resource_label"] {
            let pending = json!({"success":false,"data":{
                "errorCode":"runtime_not_ready","executionState":"not_executed",
                "status":"initializing","ready":false,"phase":phase}});
            assert!(!root_ready(pending).unwrap());
        }
        assert!(
            !root_ready(json!({"success":true,"data":{"context":{
            "rootInitialized":true,"isTransitioning":true}}}))
            .unwrap()
        );
        assert!(
            root_ready(json!({"success":true,"data":{"context":{
            "rootInitialized":true,"isTransitioning":false}}}))
            .unwrap()
        );
    }

    #[test]
    fn real_failures_and_unknown_outcomes_are_not_startup_waits() {
        for (code, execution, status) in [
            ("runtime_not_ready", "outcome_unknown", "initializing"),
            ("runtime_not_ready", "not_executed", "failed"),
            (
                "runtime_initialization_failed",
                "not_executed",
                "initializing",
            ),
        ] {
            assert!(
                root_ready(json!({"success":false,"data":{
                "errorCode":code,"executionState":execution,"status":status,"ready":false}}))
                .is_err()
            );
        }
        assert!(root_ready(json!({"success":false,"data":{}})).is_err());
        assert!(!root_ready(json!({"success":true,"data":{}})).unwrap());
    }
}
