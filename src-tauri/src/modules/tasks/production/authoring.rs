use super::*;
/// Return an isolated archive grant, or null when a current approved plan authorizes writes.
impl TaskService {
    pub fn production_authoring_guard(
        &self,
        task: &str,
        archive: &str,
    ) -> AppResult<Option<Value>> {
        let t = self.get_readonly(task)?;
        if t.status != TaskStatus::Active {
            return Err(AppError::validation("任务已不在执行状态。"));
        }
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(None);
        };
        if !super::stage_contracts::enabled(&r) {
            return Ok(None);
        }
        let root = Path::new(&t.workspace_path);
        if matches!(r.current_stage.as_str(), "implementation" | "review") {
            validation::approved(&r, root, "requirements")?;
            validation::approved(&r, root, "plan")?;
            return Ok(None);
        }
        let scope = autonomy::scope(&r);
        validation::approved(&r, root, "requirements")?;
        let selected = Path::new(archive).canonicalize().ok();
        let grant = r.workbench.prototype_grants.iter().rev().find(|g| {
            selected
                .as_ref()
                .is_some_and(|p| g["archivePath"].as_str().is_some_and(|s| Path::new(s) == p))
                && g["status"] == "active"
                && g["cycle"] == r.cycle
                && g["scopeHash"] == scope
                && g["stage"] == r.current_stage
                && g["expiresAt"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .is_some_and(|t| t > Utc::now())
        });
        grant.cloned().map(Some).ok_or_else(||AppError::new("stage_scope_denied",
            "当前阶段只需设计或方案产物。游戏写入须有已批准执行计划，或用户明确授权的隔离预研实例。","not_executed"))
    }
    pub fn production_clone_prototype(
        &self,
        task: &str,
        expected: u64,
        source: &Path,
    ) -> AppResult<String> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if r.revision != expected {
            return Err(conflict());
        }
        if !stage_contracts::enabled(&r) || !stage_contracts::design_stage(&r.current_stage) {
            return Err(AppError::validation("仅资源或方案阶段需要隔离预研授权。"));
        }
        let root = self.production_workspace(task)?;
        validation::approved(&r, &root, "requirements")?;
        let copy = copy_saved_prototype(
            &versions::archive_root(source)?,
            &files::safe_path(&root, "artifacts/prototypes")?
                .join(uuid::Uuid::new_v4().to_string()),
        )?;
        Ok(copy.to_string_lossy().into())
    }
    /// APP-only orchestration provides verified instance/path values. Never exposed by production update.
    pub fn production_grant_prototype(
        &self,
        task: &str,
        expected: u64,
        instance: &str,
        path: &str,
        note: &str,
    ) -> AppResult<ProductionView> {
        if note.trim().is_empty() || note.len() > 2000 {
            return Err(AppError::validation("请说明这次预研目标。"));
        }
        let root = self.production_workspace(task)?;
        let folder = Path::new(path).canonicalize()?;
        let allowed = files::safe_path(&root, "artifacts/prototypes")?.canonicalize()?;
        if !folder.starts_with(&allowed) || folder == allowed {
            return Err(AppError::validation("预研必须使用任务内独立副本。"));
        }
        self.production_commit(task,expected,|r|{
            validation::approved(r,&root,"requirements")?;
            if !stage_contracts::enabled(r){return Err(conflict());}
            if !stage_contracts::design_stage(&r.current_stage){return Err(conflict());}
            if r.workbench.prototype_grants.len()>=64{return Err(AppError::validation("预研记录已达容量。"));}
            let scope=autonomy::scope(r);
            r.workbench.prototype_grants.push(json!({"id":uuid::Uuid::new_v4().to_string(),"instanceId":instance,
                "archivePath":folder,"note":note,"stage":r.current_stage,"cycle":r.cycle,"scopeHash":scope,"status":"active",
                "expiresAt":(Utc::now()+chrono::Duration::minutes(30)).to_rfc3339(),"createdAt":now()}));Ok(())
        })?;
        self.production_get(task)
    }
}
/// The live editor rewrites APA workspace metadata with identical bytes. Compare
/// bytes and the complete file set rather than treating that timestamp as a save.
fn copy_saved_prototype(source: &Path, parent: &Path) -> AppResult<PathBuf> {
    let source = versions::archive_root(source)?;
    let entries = versions::inventory(std::slice::from_ref(&source))?;
    if entries.len() > 20000
        || entries.iter().map(|(_, _, bytes, _)| bytes).sum::<u64>() > 1024 * 1024 * 1024
    {
        return Err(AppError::validation("预研副本超过 20000 文件或 1 GiB。"));
    }
    let destination = parent.join(source.file_name().ok_or_else(conflict)?);
    if destination.exists() {
        return Err(conflict());
    }
    let mut hashes = BTreeMap::new();
    for (_, file, _, _) in &entries {
        let relative = file.strip_prefix(&source).map_err(AppError::internal)?;
        let hash = files::hash_file(file)?.0;
        let target = destination.join(relative);
        std::fs::create_dir_all(target.parent().unwrap())?;
        std::fs::copy(file, &target)?;
        if files::hash_file(&target)?.0 != hash || files::hash_file(file)?.0 != hash {
            return Err(AppError::validation(
                "复制期间存档内容变化，未授权该副本，请保存后重试。",
            ));
        }
        hashes.insert(file.clone(), hash);
    }
    let after = versions::inventory(std::slice::from_ref(&source))?;
    if after.len() != hashes.len() || after.iter().any(|(_, p, _, _)| !hashes.contains_key(p)) {
        return Err(AppError::validation(
            "复制期间存档文件集合变化，未授权该副本。",
        ));
    }
    for (file, hash) in hashes {
        if files::hash_file(&file)?.0 != hash {
            return Err(AppError::validation("复制期间存档内容变化，未授权该副本。"));
        }
    }
    Ok(destination)
}
pub(super) fn has_prototype(r: &ProductionRecord) -> bool {
    let scope = autonomy::scope(r);
    r.workbench.prototype_grants.iter().any(|g| {
        g["status"] == "active"
            && g["scopeHash"] == scope
            && g["cycle"] == r.cycle
            && g["stage"] == r.current_stage
            && g["expiresAt"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_some_and(|t| t > Utc::now())
    })
}
pub(super) fn project_grants(r: &mut ProductionRecord) {
    let scope = autonomy::scope(r);
    for g in &mut r.workbench.prototype_grants {
        g["effective"] = json!(
            g["status"] == "active"
                && g["scopeHash"] == scope
                && g["stage"] == r.current_stage
                && g["cycle"] == r.cycle
                && g["expiresAt"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .is_some_and(|t| t > Utc::now())
        );
    }
}
