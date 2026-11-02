use super::*;

impl TaskService {
    /// 普通任务重开由本工具创建的存档时，也沿用相同的私有数据根。
    pub fn production_owned_archive_root(
        &self,
        task: &str,
        archive: &str,
    ) -> AppResult<Option<PathBuf>> {
        if archive.is_empty() {
            return Ok(None);
        }
        let root = self.production_workspace(task)?;
        let data = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            &format!(".abya-authoring/{task}"),
        )?;
        if !data.is_dir() {
            return Ok(None);
        }
        let selected = Path::new(archive).canonicalize()?;
        if !selected.starts_with(data.canonicalize()?.join("Archives")) {
            return Ok(None);
        }
        self.owned_authoring_data_root(task).map(Some)
    }

    /// 完整流程须已确认计划；未启用制作流程的普通任务沿用任务级授权。
    pub fn production_bootstrap_root(&self, task: &str) -> AppResult<PathBuf> {
        let t = self.get_readonly(task)?;
        if t.status != TaskStatus::Active {
            return Err(conflict());
        }
        if let Some(r) = self.production_get(task)?.record {
            if !["implementation", "review"].contains(&r.current_stage.as_str()) {
                return Err(AppError::new(
                    "stage_scope_denied",
                    "请先确认执行计划，再创建任务存档。",
                    "not_executed",
                ));
            }
            let root = Path::new(&t.workspace_path);
            validation::approved(&r, root, "requirements")?;
            validation::approved(&r, root, "plan")?;
        }
        self.owned_authoring_data_root(task)
    }

    /// 创建结果独立于阶段状态，停止或归档后仍可查询原收据。
    pub fn production_creation_receipt_path(
        &self,
        task: &str,
        creation: &str,
    ) -> AppResult<PathBuf> {
        let t = self.get_readonly(task)?;
        let id = uuid::Uuid::parse_str(creation)
            .map_err(|_| AppError::validation("creationId 必须为 UUID。"))?;
        files::safe_path(
            Path::new(&t.workspace_path),
            &format!("artifacts/archive-creation/{id}.json"),
        )
    }
}
