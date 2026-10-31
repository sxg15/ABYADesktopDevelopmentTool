use super::*;
use crate::modules::instances::{ArchiveSelection, LaunchInstanceInput, LaunchMode};

fn implementation_check(tool: &str) -> bool {
    matches!(
        tool,
        "editor_gameplay_architecture_validate"
            | "editor_gameplay_architecture_lint"
            | "editor_lua_validate"
            | "editor_lua_scaffold"
            | "lua_run"
    )
}
fn prototype_tool(tool: &str) -> bool {
    implementation_check(tool) && tool != "lua_run"
        || matches!(
            tool,
            "custom_ui_authoring_open"
                | "custom_ui_authoring_select_node"
                | "custom_ui_authoring_create_node"
                | "custom_ui_authoring_duplicate_node"
                | "custom_ui_authoring_delete_node"
                | "custom_ui_authoring_move_node"
                | "custom_ui_authoring_set_properties"
                | "custom_ui_authoring_set_layout"
                | "custom_ui_authoring_set_parameters"
                | "custom_ui_authoring_set_custom_parameters"
                | "custom_ui_authoring_batch_edit"
                | "custom_ui_authoring_undo"
                | "custom_ui_authoring_redo"
                | "custom_ui_authoring_save"
        )
        || matches!(
            tool,
            "editor_logic_binding_set_actions"
                | "editor_logic_binding_remove"
                | "editor_logic_input_binding_create"
                | "editor_prefab_logic_binding_set_actions"
                | "editor_prefab_logic_binding_remove"
                | "editor_save_archive"
        )
}
impl RuntimeBridgeService {
    pub fn launch_scoped(
        &self,
        input: LaunchInstanceInput,
    ) -> AppResult<crate::modules::instances::GameInstance> {
        if input.profile.mode != LaunchMode::LanClient {
            crate::modules::instances::validate_executable(input.executable_path.trim())?;
            self.tasks.production_check_player_pin(
                &input.task_id,
                std::path::Path::new(input.executable_path.trim()),
            )?;
        }
        let path = input
            .profile
            .archive
            .as_ref()
            .map(|a| a.archive_path.clone())
            .unwrap_or_default();
        let permit = self
            .tasks
            .production_authoring_guard(&input.task_id, &path)?;
        if permit.is_none() {
            let data = self.tasks.production_authoring_data_root(&input.task_id)?;
            return self.instances.launch_with_data_root(input, data);
        }
        if input.profile.mode != LaunchMode::Editor {
            return Err(AppError::new(
                "stage_scope_denied",
                "预研仅允许编辑模式。",
                "not_executed",
            ));
        }
        let key = format!("prototype:{}", input.task_id);
        if !self.acceptance_busy.lock().insert(key.clone()) {
            return Err(AppError::validation("预研实例正在启动。"));
        }
        let result = (|| {
            if self.instances.list(Some(&input.task_id))?.iter().any(|i| {
                matches!(
                    i.process_state,
                    crate::modules::instances::ProcessState::Running
                        | crate::modules::instances::ProcessState::Launching
                        | crate::modules::instances::ProcessState::Stopping
                ) && i
                    .profile
                    .as_ref()
                    .and_then(|p| p.archive.as_ref())
                    .is_some_and(|a| a.archive_path == path)
            }) {
                return Err(AppError::validation(
                    "该预研副本已有实例，先核对并确认退出。",
                ));
            }
            self.instances.launch(input)
        })();
        self.acceptance_busy.lock().remove(&key);
        result
    }
    pub(super) fn check_authoring(&self, id: &str, tool: &str) -> AppResult<()> {
        let instance = self.instances.read(id)?;
        let task = instance
            .task_id
            .as_deref()
            .ok_or_else(|| AppError::validation("No task owner"))?;
        let archive = instance
            .profile
            .as_ref()
            .and_then(|p| p.archive.as_ref())
            .map(|a| a.archive_path.as_str())
            .unwrap_or("");
        let permit = self.tasks.production_authoring_guard(task, archive);
        if matches!(permit, Ok(None)) {
            return Ok(());
        }
        // Cleanup and contract discovery do not require gameplay-writing authority.
        if matches!(
            tool,
            "read_me_first" | "capability_search" | "capability_describe" | "runtime_input_reset"
        ) {
            return Ok(());
        }
        if !implementation_check(tool)
            && self.list_tools(id)?.as_array().is_some_and(|tools| {
                tools
                    .iter()
                    .any(|t| t["name"] == tool && t["riskLevel"] == "ReadOnly")
            })
        {
            return Ok(());
        }
        let grant = permit?.ok_or_else(|| AppError::validation("缺少预研授权。"))?;
        if !prototype_tool(tool) {
            return Err(AppError::new(
                "stage_scope_denied",
                "此工具不在隔离交互小样范围内；授权仅覆盖当前副本的 UI 与事件绑定。",
                "not_executed",
            ));
        }
        let live = self.acceptance_read(id, "editor_get_archive", json!({}))?;
        let actual = live["path"]
            .as_str()
            .map(std::path::Path::new)
            .and_then(|p| p.canonicalize().ok());
        let expected = grant["archivePath"].as_str().map(std::path::Path::new);
        if actual.as_deref() != expected || actual.is_none() {
            return Err(AppError::new(
                "stage_scope_denied",
                "当前实际存档不是已授权的隔离预研副本。",
                "not_executed",
            ));
        }
        Ok(())
    }
    /// User action: clone saved source, launch that copy and bind a short-lived grant.
    pub fn start_prototype(
        &self,
        task: &str,
        expected: u64,
        source: &str,
        note: &str,
    ) -> AppResult<crate::modules::tasks::ProductionView> {
        let key = format!("prototype:{task}");
        if !self.acceptance_busy.lock().insert(key.clone()) {
            return Err(AppError::validation("预研副本正在创建。"));
        }
        let result = self.start_prototype_inner(task, expected, source, note);
        self.acceptance_busy.lock().remove(&key);
        result
    }
    fn start_prototype_inner(
        &self,
        task: &str,
        expected: u64,
        source: &str,
        note: &str,
    ) -> AppResult<crate::modules::tasks::ProductionView> {
        let old = self.instances.read(source)?;
        if old.task_id.as_deref() != Some(task) {
            return Err(AppError::validation("源实例不属于当前任务。"));
        }
        if note.trim().is_empty() || note.len() > 2000 {
            return Err(AppError::validation("请填写预研范围。"));
        }
        let mut profile = old
            .profile
            .ok_or_else(|| AppError::validation("源实例缺少启动配置。"))?;
        let live = self.acceptance_read(source, "editor_get_archive", json!({}))?;
        let level = self.acceptance_read(source, "editor_get_level", json!({}))?;
        let path = live["path"]
            .as_str()
            .ok_or_else(|| AppError::validation("无法确认源存档。"))?;
        let copy =
            self.tasks
                .production_clone_prototype(task, expected, std::path::Path::new(path))?;
        let guid = live["guid"]
            .as_str()
            .ok_or_else(|| AppError::validation("缺少存档标识。"))?;
        let level_guid = level["levelGuid"]
            .as_str()
            .ok_or_else(|| AppError::validation("缺少关卡标识。"))?;
        profile.mode = LaunchMode::Editor;
        profile.host_instance_id = None;
        profile.archive = Some(ArchiveSelection {
            archive_path: copy.clone(),
            archive_guid: guid.into(),
            archive_name: "隔离交互小样".into(),
            level_guid: level_guid.into(),
            level_name: "预研关卡".into(),
        });
        let instance = self.instances.launch(LaunchInstanceInput {
            task_id: task.into(),
            name: "用户授权的隔离预研".into(),
            executable_path: old
                .executable_path
                .ok_or_else(|| AppError::validation("缺少运行包。"))?,
            profile,
        })?;
        let grant = (|| {
            self.wait_for_cli(&instance.id, Duration::from_secs(90))?;
            let started = Instant::now();
            loop {
                let report = self.instances.read_launch_report(&instance.id)?;
                if report.report["phase"] == "failed" {
                    return Err(AppError::validation(format!(
                        "隔离副本启动失败：{}",
                        report.report["error"]
                            .as_str()
                            .unwrap_or("请检查启动报告与 Player 版本。")
                    )));
                }
                if report.report["phase"] == "ready" && report.report["success"] == true {
                    break;
                }
                if started.elapsed() > Duration::from_secs(90) {
                    return Err(AppError::validation(
                        "隔离副本尚未就绪，未授权写入。请检查启动报告。",
                    ));
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            self.tasks
                .production_grant_prototype(task, expected, &instance.id, &copy, note)
        })();
        match grant {
            Ok(v) => Ok(v),
            Err(e) => {
                let _ = self.instances.stop(&instance.id);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn prototype_cannot_switch_archive_execute_general_lua_or_add_unknown_mutations() {
        for tool in [
            "editor_load_archive",
            "editor_create_archive",
            "lua_run",
            "future_unknown_write",
        ] {
            assert!(!super::prototype_tool(tool));
        }
        assert!(super::prototype_tool("custom_ui_authoring_set_layout"));
        assert!(super::prototype_tool("editor_logic_binding_set_actions"));
    }
}
