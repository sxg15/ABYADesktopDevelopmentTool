use super::*;
static ASSET_GATE: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

impl TaskService {
    /// 任务绑定只记录来源，不授予绕过 APP 当前设置的权限。
    pub fn task_player_status(&self, task: &str) -> AppResult<Value> {
        let root = self.production_workspace(task)?;
        let path = files::safe_path(&root, "artifacts/player-selection.json")?;
        let saved: Value = if path.exists() {
            serde_json::from_slice(&std::fs::read(path)?)?
        } else {
            Value::Null
        };
        let selected = crate::foundation::selected_game_executable(&self.database, "")?;
        let hash = versions::player_fingerprint(&selected)?;
        let legacy = self.database.with_connection(|db| load(db, task))?;
        let previous = if saved.is_null() {
            legacy.map(|r| json!({"path":r.version_details["playerSource"],"hash":r.version_details["playerBuildHash"]})).unwrap_or(Value::Null)
        } else {
            saved
        };
        let matches = previous["path"]
            .as_str()
            .is_none_or(|p| Path::new(p).canonicalize().ok().as_ref() == Some(&selected))
            && previous["hash"].as_str().is_none_or(|h| h == hash);
        Ok(
            json!({"selectedPath":selected,"selectedHash":hash,"previous":previous,"matches":matches}),
        )
    }

    /// APP 显式确认切换；预期指纹防止确认期间设置或文件发生变化。
    pub fn adopt_task_player(&self, task: &str, expected_hash: &str) -> AppResult<Value> {
        let _gate = ASSET_GATE.lock();
        let status = self.task_player_status(task)?;
        if status["selectedHash"] != expected_hash {
            return Err(conflict());
        }
        self.write_task_player(task, &status)?;
        Ok(status)
    }

    pub(super) fn check_task_player(&self, task: &str, player: &Path) -> AppResult<()> {
        let _gate = ASSET_GATE.lock();
        crate::foundation::selected_game_executable(&self.database, &player.to_string_lossy())?;
        let status = self.task_player_status(task)?;
        if status["matches"] != true {
            return Err(AppError::new(
                "player_pin_mismatch",
                "APP 构建包与任务记录不同，请在启动窗口确认使用当前包并重新验证。",
                "not_executed",
            ));
        }
        if self.production_get(task)?.record.is_some_and(|r| {
            r.policy["capabilityPreflightVersion"] == 1
                && matches!(r.current_stage.as_str(), "implementation" | "review")
        }) {
            self.validate_current_preflight(task)?;
        }
        self.write_task_player(task, &status)
    }

    fn write_task_player(&self, task: &str, status: &Value) -> AppResult<()> {
        let root = self.production_workspace(task)?;
        if status["previous"]["path"] == status["selectedPath"]
            && status["previous"]["hash"] == status["selectedHash"]
            && files::safe_path(&root, "artifacts/player-selection.json")?.exists()
        {
            return Ok(());
        }
        let value = json!({"taskId":task,"path":status["selectedPath"],"hash":status["selectedHash"],"recordedAt":now()});
        files::write_file(
            &root,
            &format!("artifacts/player-selections/{}.json", uuid::Uuid::new_v4()),
            &serde_json::to_vec_pretty(&json!({"previous":status["previous"],"selected":value}))?,
        )?;
        files::write_file(
            &root,
            "artifacts/player-selection.json",
            &serde_json::to_vec_pretty(&value)?,
        )
    }

    /// 只返回本任务归属目录；不会扫描其他任务或把冻结候选当成编辑源。
    pub fn task_archive_sources(&self, task: &str) -> AppResult<Value> {
        let root = self.production_workspace(task)?;
        let data = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            &format!(".abya-authoring/{task}"),
        )?;
        let mut roots = Vec::new();
        if data.exists() {
            let owner: Value = serde_json::from_slice(&std::fs::read(data.join("owner.json"))?)?;
            if owner["taskId"] != task {
                return Err(AppError::validation("任务存档目录归属不匹配。"));
            }
            roots.push(data.join("Archives"));
        }
        let record = self.database.with_connection(|db| load(db, task))?;
        let mut candidate = Value::Null;
        let mut preview_version = Value::Null;
        if let Some(r) = record {
            preview_version = r.workbench.execution.verification_package["version"].clone();
            if let Some(editable) = r.version_details["editableArchiveSource"].as_str() {
                let path = versions::archive_root(Path::new(editable))?;
                if !roots.iter().any(|p| path.starts_with(p)) {
                    roots.push(path);
                }
            }
            if r.version_details["frozenSnapshot"] == true {
                candidate =
                    json!({"version":r.current_version,"path":r.version_details["archiveSource"]});
            }
        }
        Ok(json!({"editableRoots":roots,"candidate":candidate,"previewVersion":preview_version}))
    }

    pub fn task_export_source(&self, task: &str, version: &str) -> AppResult<PathBuf> {
        let r = self
            .database
            .with_connection(|db| load(db, task))?
            .ok_or_else(conflict)?;
        if r.current_version.as_deref() != Some(version)
            || r.version_details["frozenSnapshot"] != true
        {
            return Err(conflict());
        }
        versions::verify(&r.version_details)?;
        versions::archive_root(Path::new(mutation::text(
            &r.version_details,
            "archiveSource",
        )?))
    }
}
