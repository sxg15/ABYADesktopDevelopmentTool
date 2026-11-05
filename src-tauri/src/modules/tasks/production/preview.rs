use super::*;

impl TaskService {
    /// 原生输入仅用于已批准实现的测试实例，不操作用户正在试玩的副本。
    pub fn production_native_input_guard(&self, task: &str, instance: &str) -> AppResult<()> {
        if self.get_readonly(task)?.status != TaskStatus::Active {
            return Err(conflict());
        }
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if !["implementation", "review"].contains(&r.current_stage.as_str()) {
            return Err(AppError::validation("原生输入仅用于实现或修改阶段的验证。"));
        }
        let root = self.production_workspace(task)?;
        validation::approved(&r, &root, "requirements")?;
        validation::approved(&r, &root, "plan")?;
        if r.acceptance_sessions
            .iter()
            .chain(r.workbench.execution.preview_sessions.iter())
            .any(|s| {
                s["instanceIds"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id == instance))
            })
        {
            return Err(AppError::validation(
                "这是人工试玩实例，AI 不能向该窗口发送输入。请使用独立测试实例。",
            ));
        }
        Ok(())
    }
    /// 从包生成时的记录读取最后可玩版本，不切换当前候选，也不修改验收状态。
    pub(super) fn preview_record(&self, task: &str) -> AppResult<ProductionRecord> {
        if self.get_readonly(task)?.status != TaskStatus::Active {
            return Err(conflict());
        }
        let current = self.production_get(task)?.record.ok_or_else(conflict)?;
        let root = self.production_workspace(task)?;
        validation::approved(&current, &root, "requirements")?;
        validation::approved(&current, &root, "plan")?;
        // 反馈复验始终对应当前冻结候选；旧检查点不能代替本轮修改。
        if current.current_stage == "review" {
            validation::version(&current)?;
            versions::verify(&current.version_details)?;
            return Ok(current);
        }
        let pack = &current.workbench.execution.verification_package;
        if pack["version"].as_str() == current.current_version.as_deref() {
            verification_package::core_ready(&current, &root)?;
            // 旧任务的包没有历史来源时，只允许完整校验当前候选后预览。
            if pack["sourceRevision"].as_u64().is_none()
                && verification_package::valid(&current, &root)
            {
                return Ok(current);
            }
        }
        let revision = pack["sourceRevision"]
            .as_u64()
            .ok_or_else(|| AppError::validation("请先生成包含预览来源的验证包。"))?;
        let body: String = self.database.with_connection(|c| {
            Ok(c.query_row(
                "SELECT body FROM task_production_history WHERE task_id=?1 AND revision=?2",
                rusqlite::params![task, revision as i64],
                |row| row.get(0),
            )?)
        })?;
        let mut source: ProductionRecord = serde_json::from_str(&body)?;
        if autonomy::scope(&source) != autonomy::scope(&current) || source.cycle != current.cycle {
            return Err(AppError::validation("预览包属于已改变的需求范围。"));
        }
        source.workbench.execution.verification_package = pack.clone();
        if !verification_package::valid(&source, &root) {
            return Err(AppError::validation("预览包或其原始证据已经改变。"));
        }
        Ok(source)
    }

    pub fn preview_spec(&self, task: &str) -> AppResult<AcceptanceSpec> {
        let r = self.preview_record(task)?;
        if !["single", "multiplayer"].contains(&r.player_mode.as_str()) {
            return Err(AppError::validation("预览须沿用已确认的单人或多人模式。"));
        }
        let text = |key| mutation::text(&r.version_details, key).map(str::to_owned);
        Ok(AcceptanceSpec {
            version: validation::version(&r)?.into(),
            executable_path: acceptance::launch_path(text("playerSource")?),
            archive_path: acceptance::launch_path(text("archiveSource")?),
            archive_guid: text("archiveGuid")?,
            level_guid: text("levelGuid")?,
            seats: if r.player_mode == "multiplayer" { 2 } else { 1 },
            required_tools: vec![],
            require_ui: true,
        })
    }

    pub fn preview_archive(&self, task: &str, version: &str) -> AppResult<String> {
        let r = self.preview_record(task)?;
        if r.current_version.as_deref() != Some(version) {
            return Err(conflict());
        }
        let root = self.production_workspace(task)?;
        let runs = files::safe_path(root.parent().ok_or_else(conflict)?, ".abya-play-runs")?;
        let data = runs.join(uuid::Uuid::new_v4().simple().to_string());
        let parent = data.join("Archives");
        std::fs::create_dir_all(&parent)?;
        std::fs::write(
            data.join("owner.json"),
            serde_json::to_vec(&json!({"taskId":task,"version":version,"purpose":"preview"}))?,
        )?;
        let source = Path::new(mutation::text(&r.version_details, "archiveSource")?);
        Ok(acceptance::launch_path(
            version_snapshots::copy_verified(source, &parent)?
                .to_string_lossy()
                .into_owned(),
        ))
    }

    pub fn record_preview(&self, task: &str, version: &str, session: Value) -> AppResult<()> {
        let source = self.preview_record(task)?;
        if source.current_version.as_deref() != Some(version) {
            return Err(conflict());
        }
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        self.production_commit(task, r.revision, |r| {
            let list = &mut r.workbench.execution.preview_sessions;
            list.push(session);
            if list.len() > 20 {
                list.remove(0);
            }
            Ok(())
        })
    }
}
