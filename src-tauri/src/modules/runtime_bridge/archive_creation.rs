use super::*;
use crate::modules::instances::{
    ArchiveSelection, LaunchInstanceInput, LaunchMode, LaunchProfile, WindowMode,
    WindowVisibilityMode,
};
use serde::Deserialize;
use std::{io::Write, path::Path};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateArchiveInput {
    pub task_id: String,
    pub creation_id: String,
    pub name: String,
    pub author: String,
    #[serde(default)]
    pub executable_path: String,
}

/// 文件名语义由桌面限定，具体存档格式和创建流程由当前 Player 决定。
fn validate_input(input: &CreateArchiveInput) -> AppResult<()> {
    uuid::Uuid::parse_str(&input.creation_id)
        .map_err(|_| AppError::validation("creationId 必须为 UUID。"))?;
    if [".", ".."].contains(&input.name.trim())
        || input.name.ends_with(['.', ' '])
        || input.name.chars().any(|c| "<>:\"/\\|?*".contains(c))
    {
        return Err(AppError::validation("name 必须是有效存档名称。"));
    }
    for (value, max) in [(&input.name, 80), (&input.author, 120)] {
        if value.trim().is_empty()
            || value.chars().count() > max
            || value.chars().any(|c| c.is_control())
        {
            return Err(AppError::validation(
                "name/author 不能为空、过长或包含文件路径字符。",
            ));
        }
    }
    Ok(())
}

fn save(path: &Path, value: &Value) -> AppResult<()> {
    let next = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&next, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(next, path)?;
    Ok(())
}

impl RuntimeBridgeService {
    fn bootstrap_call(
        &self,
        id: &str,
        tool: &str,
        input: Value,
        session: &str,
        operation: &str,
        cancel: Arc<AtomicBool>,
    ) -> AppResult<Value> {
        if matches!(tool, "editor_create_archive" | "editor_load_archive") {
            let instance = self.instances.read(id)?;
            let task = instance
                .task_id
                .as_deref()
                .ok_or_else(|| AppError::validation("初始化实例没有任务归属。"))?;
            self.tasks.production_bootstrap_root(task)?;
            let player = instance
                .executable_path
                .as_deref()
                .ok_or_else(|| AppError::validation("初始化实例没有运行包。"))?;
            self.tasks
                .production_check_player_pin(task, Path::new(player))?;
        }
        self.check_authoring(id, tool)?;
        let value = self.run(
            id,
            &["capability", "run", tool],
            Some(input),
            session,
            operation,
            Duration::from_secs(320),
            cancel,
        )?;
        if value["success"] != true {
            return Err(AppError::new(
                "archive_initialization_failed",
                format!("{tool}: {}", value["data"]),
                "inspect_creation_status",
            ));
        }
        Ok(value["data"].clone())
    }

    fn create_archive_once(
        &self,
        input: &CreateArchiveInput,
        data: &Path,
        path: &Path,
        identity: Value,
        session: &str,
        request: &str,
    ) -> AppResult<Value> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let active_key = format!("bootstrap-{}-{}", input.task_id, input.creation_id);
        let mut state = json!({"taskId":input.task_id,"creationId":input.creation_id,"requestId":request,"operationId":active_key,
            "input":identity,"status":"starting","dataRoot":data,"startedAt":chrono::Utc::now().to_rfc3339()});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&serde_json::to_vec_pretty(&state)?)?;
        file.sync_all()?;
        drop(file);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut owned = None;
        let mut create_dispatched = false;
        let result = (|| -> AppResult<()> {
            let witness = super::native_archive_ui::RootWitness::capture(data)?;
            let instance = self.instances.launch_bootstrap(
                LaunchInstanceInput {
                    task_id: input.task_id.clone(),
                    name: format!("初始化 · {}", input.name),
                    executable_path: input.executable_path.clone(),
                    profile: LaunchProfile {
                        mode: LaunchMode::Editor,
                        window_mode: WindowMode::Windowed,
                        visibility_mode: WindowVisibilityMode::Background,
                        width: 1280,
                        height: 720,
                        archive: None,
                        host_instance_id: None,
                        exit_on_failure: false,
                        language: String::new(),
                    },
                },
                data.to_path_buf(),
            )?;
            let id = &instance.id;
            owned = Some(id.clone());
            state["instanceId"] = json!(id);
            state["status"] = json!("initializing-runtime");
            save(path, &state)?;
            self.active.lock().insert(
                active_key.clone(),
                (id.clone(), session.into(), cancel.clone()),
            );
            self.wait_for_cli(id, Duration::from_secs(60)).map_err(|e| AppError::new(
                "bootstrap_runtime_unavailable",
                format!("托管初始化未就绪：{}。请核对 Player 是否支持免按键启动和主菜单存档入口。",e.message),
                "not_executed",
            ))?;
            self.wait_for_game_root(id, session, &cancel)?;
            let tools = self.list_tools(id)?;
            for name in [
                "editor_create_archive",
                "editor_load_archive",
                "editor_get_archive",
            ] {
                if !tools
                    .as_array()
                    .is_some_and(|list| list.iter().any(|v| v["name"] == name || v["id"] == name))
                {
                    return Err(AppError::validation(format!(
                        "当前 Player 缺少 {name}，未创建存档。"
                    )));
                }
            }
            state["status"] = json!("creating");
            let mut native_ui = false;
            for name in ["editor_create_archive", "editor_load_archive"] {
                let descriptor = self.bootstrap_call(
                    id,
                    "capability_describe",
                    json!({"capabilityId":name}),
                    session,
                    &uuid::Uuid::new_v4().to_string(),
                    cancel.clone(),
                )?;
                if descriptor["available"] != true {
                    if descriptor["available"] == false
                        && descriptor["currentContext"]["mode"] == "MainMenu"
                    {
                        native_ui = true;
                        continue;
                    }
                    return Err(AppError::new(
                        "bootstrap_capability_unavailable",
                        format!(
                            "当前 Player 尚未开放主菜单存档入口 {name}，请更新支持通用初始化的运行包。"
                        ),
                        "not_executed",
                    ));
                }
                if !native_ui
                    && name == "editor_load_archive"
                    && descriptor["inputSchema"]["properties"]["openStartLevel"]["type"]
                        != "boolean"
                {
                    return Err(AppError::new(
                        "bootstrap_capability_unavailable",
                        "当前 Player 缺少显式打开开始关卡的初始化契约。",
                        "not_executed",
                    ));
                }
            }
            state["createOperationId"] = json!(format!("{}-create", input.creation_id));
            state["initializationRoute"] = json!(if native_ui {
                "native-creator-ui"
            } else {
                "editor_create_archive"
            });
            let created = if native_ui {
                self.create_from_native_form(
                    id,
                    &witness,
                    super::native_archive_ui::ArchiveFormInput {
                        name: &input.name,
                        author: &input.author,
                        description: "Task-owned blank archive",
                    },
                    session,
                    &cancel,
                    || {
                        self.tasks.production_bootstrap_root(&input.task_id)?;
                        self.tasks.production_check_player_pin(
                            &input.task_id,
                            Path::new(&input.executable_path),
                        )?;
                        save(path, &state)?;
                        create_dispatched = true;
                        Ok(())
                    },
                )?
            } else {
                save(path, &state)?;
                create_dispatched = true;
                self.bootstrap_call(id,"editor_create_archive",json!({"name":input.name,"author":input.author,"introduce":"Task-owned blank archive"}),session,state["createOperationId"].as_str().unwrap(),cancel.clone())?
            };
            state["createResult"] = created.clone();
            save(path, &state)?;
            let archive = checked_archive(data, &created["archive"])?;
            state["created"] = json!(true);
            state["archivePath"] = json!(archive);
            state["archiveGuid"] = created["archive"]["guid"].clone();
            state["status"] = json!("loading");
            state["loadOperationId"] = json!(format!("{}-load", input.creation_id));
            save(path, &state)?;
            let editing_id = if native_ui {
                let selection = super::native_archive_ui::ui_archive_selection(&created)?;
                if self.instances.stop(id)?.process_alive {
                    return Err(AppError::validation(
                        "建档实例尚未退出，未启动第二个编辑器。",
                    ));
                }
                let editor = self.instances.launch_with_data_root(
                    LaunchInstanceInput {
                        task_id: input.task_id.clone(),
                        name: format!("编辑 · {}", input.name),
                        executable_path: input.executable_path.clone(),
                        profile: LaunchProfile {
                            mode: LaunchMode::Editor,
                            window_mode: WindowMode::Windowed,
                            visibility_mode: WindowVisibilityMode::Background,
                            width: 1280,
                            height: 720,
                            archive: Some(selection),
                            host_instance_id: None,
                            exit_on_failure: false,
                            language: String::new(),
                        },
                    },
                    Some(data.to_path_buf()),
                )?;
                owned = Some(editor.id.clone());
                state["bootstrapInstanceId"] = json!(id);
                state["instanceId"] = json!(editor.id);
                save(path, &state)?;
                self.active.lock().insert(
                    active_key.clone(),
                    (editor.id.clone(), session.into(), cancel.clone()),
                );
                self.wait_for_cli(&editor.id, Duration::from_secs(60))?;
                self.wait_for_game_root(&editor.id, session, &cancel)?;
                editor.id
            } else {
                self.bootstrap_call(
                id,
                "editor_load_archive",
                json!({"archiveGuid":state["archiveGuid"],"discardUnsavedChanges":false,"openStartLevel":true}),
                session,
                state["loadOperationId"].as_str().unwrap(),
                cancel.clone(),
            )?;
                id.clone()
            };
            let id = &editing_id;
            let loaded = self.wait_for_loaded_archive(id, session, &cancel)?;
            if loaded["guid"] != state["archiveGuid"]
                || loaded["archiveAndLevelLoaded"] != true
                || checked_archive(data, &loaded)? != archive
            {
                return Err(AppError::validation("实际加载的存档与创建结果不一致。"));
            }
            let level = loaded["currentLevelGuid"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::validation("空白存档没有已加载的初始关卡。"))?;
            let selection = ArchiveSelection {
                archive_path: super::archive_creation::display_path(&archive),
                archive_guid: loaded["guid"].as_str().unwrap().into(),
                archive_name: input.name.clone(),
                level_guid: level.into(),
                level_name: loaded["currentLevelName"].as_str().unwrap_or("").into(),
            };
            if native_ui {
                // The relaunched editor is already bound by its launch profile.
                if loaded["currentLevelGuid"] != created["archive"]["currentLevelGuid"] {
                    return Err(AppError::validation(
                        "原生新建存档的起始关卡与实际打开的关卡不同。",
                    ));
                }
            } else {
                self.instances
                    .bind_created_archive(id, &input.task_id, selection.clone())?;
            }
            state["archive"] = serde_json::to_value(selection)?;
            state["status"] = json!("ready");
            state["created"] = json!(true);
            state["loaded"] = json!(true);
            Ok(())
        })();
        self.active.lock().remove(&active_key);
        if let Err(error) = result {
            state["failedPhase"] = state["status"].clone();
            state["status"] = json!(if create_dispatched {
                "outcome-unknown"
            } else {
                "failed-before-create"
            });
            state["error"] = json!({"code":error.code,"message":error.message});
            if !create_dispatched && let Some(id) = owned {
                state["cleanup"] =
                    json!({"stopped":self.instances.stop(&id).is_ok_and(|r|!r.process_alive)});
            }
        }
        state["finishedAt"] = json!(chrono::Utc::now().to_rfc3339());
        save(path, &state)?;
        self.archive_creation_status(&input.task_id, &input.creation_id)
    }
    pub fn archive_creation_status(&self, task: &str, creation: &str) -> AppResult<Value> {
        let path = self
            .tasks
            .production_creation_receipt_path(task, creation)?;
        if !path.is_file() {
            return Ok(json!({"status":"not-started","creationId":creation}));
        }
        let mut value: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        if value["taskId"] != task {
            return Err(AppError::validation("初始化回执归属不匹配。"));
        }
        value["instanceRunning"] = json!(
            value["instanceId"]
                .as_str()
                .is_some_and(|id| self.instances.get(id).is_ok_and(|v| v.process_alive))
        );
        Ok(value)
    }

    /// 同一任务串行创建；已存在的 creationId 只返回原结果，永不重放写入。
    pub fn create_archive(
        &self,
        input: CreateArchiveInput,
        session: &str,
        operation: &str,
    ) -> AppResult<Value> {
        validate_input(&input)?;
        let data = self.tasks.production_bootstrap_root(&input.task_id)?;
        self.tasks
            .production_check_player_pin(&input.task_id, Path::new(&input.executable_path))?;
        crate::modules::instances::validate_executable(&input.executable_path)?;
        let path = self
            .tasks
            .production_creation_receipt_path(&input.task_id, &input.creation_id)?;
        let identity = json!({"name":input.name,"author":input.author,"player":std::fs::canonicalize(&input.executable_path)?});
        if path.exists() {
            let result = self.archive_creation_status(&input.task_id, &input.creation_id)?;
            if result["input"] != identity {
                return Err(AppError::validation("相同 creationId 的参数不能改变。"));
            }
            return Ok(result);
        }
        let key = format!("archive-create:{}", input.task_id);
        if !self.acceptance_busy.lock().insert(key.clone()) {
            return Err(AppError::validation("本任务另一个存档初始化仍在执行。"));
        }
        let result = self.create_archive_once(&input, &data, &path, identity, session, operation);
        self.acceptance_busy.lock().remove(&key);
        result
    }
}

pub(super) fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(p) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{p}")
    } else {
        value.strip_prefix(r"\\?\").unwrap_or(&value).into()
    }
}

pub(super) fn checked_archive(root: &Path, value: &Value) -> AppResult<PathBuf> {
    let guid = value["guid"]
        .as_str()
        .ok_or_else(|| AppError::validation("游戏未返回存档 GUID。"))?;
    uuid::Uuid::parse_str(guid).map_err(|_| AppError::validation("游戏返回的存档 GUID 无效。"))?;
    let path = PathBuf::from(
        value["path"]
            .as_str()
            .ok_or_else(|| AppError::validation("游戏未返回存档路径。"))?,
    )
    .canonicalize()?;
    let archives = root.canonicalize()?.join("Archives");
    if path.parent() != Some(archives.as_path())
        || !path.join("Main.PBArc").is_file()
        || path.join("Main.PBArc").canonicalize()?.parent() != Some(path.as_path())
    {
        return Err(AppError::validation(
            "创建结果不在当前任务的独立存档目录内。",
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creation_input_accepts_generic_unicode_names_and_rejects_paths() {
        let mut input = CreateArchiveInput {
            task_id: "task".into(),
            creation_id: uuid::Uuid::new_v4().to_string(),
            name: "独立开发项目".into(),
            author: "Team / Creator".into(),
            executable_path: "player.exe".into(),
        };
        assert!(validate_input(&input).is_ok());
        for name in ["../other", "C:\\shared", "..", "bad/name", "empty.", ""] {
            input.name = name.into();
            assert!(validate_input(&input).is_err());
        }
        input.name = "New archive".into();
        input.creation_id = "../id".into();
        assert!(validate_input(&input).is_err());
    }
    #[test]
    fn runtime_archive_must_be_a_real_direct_child_of_owned_root() {
        let root = std::env::temp_dir().join(format!("abya-create-test-{}", uuid::Uuid::new_v4()));
        let inside = root.join("Archives/created");
        let foreign = root.join("foreign");
        for p in [&inside, &foreign] {
            std::fs::create_dir_all(p).unwrap();
            std::fs::write(p.join("Main.PBArc"), "{}").unwrap();
        }
        let id = uuid::Uuid::new_v4().to_string();
        assert!(checked_archive(&root, &json!({"guid":id,"path":inside})).is_ok());
        assert!(checked_archive(&root, &json!({"guid":id,"path":foreign})).is_err());
        assert!(checked_archive(&root, &json!({"guid":"invalid","path":inside})).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
