use super::{ProductionView, TaskService, files, mutation};
use crate::foundation::{AppError, AppResult};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct VersionSources {
    pub archive_path: PathBuf,
    pub player_path: PathBuf,
    pub archive_guid: String,
    pub level_guid: String,
    pub instance_id: String,
}
pub(super) fn archive_root(path: &Path) -> AppResult<PathBuf> {
    let root = if path.is_file() {
        path.parent().unwrap()
    } else {
        path
    };
    if !root.join("Main.PBArc").is_file() {
        return Err(AppError::validation("实际存档目录缺少 Main.PBArc。"));
    }
    Ok(root.canonicalize()?)
}
fn player_roots(path: &Path) -> AppResult<Vec<PathBuf>> {
    if !path.is_file() {
        return Err(AppError::validation("Player 可执行文件不可读。"));
    }
    let path = path.canonicalize()?;
    let parent = path.parent().unwrap();
    let data = parent.join(format!(
        "{}_Data",
        path.file_stem().unwrap().to_string_lossy()
    ));
    let mut roots = vec![path.clone()];
    if data.is_dir() {
        roots.push(data);
    }
    for name in [
        "UnityPlayer.dll",
        "GameAssembly.dll",
        "baselib.dll",
        "MonoBleedingEdge",
    ] {
        let item = parent.join(name);
        if item.exists() {
            roots.push(item);
        }
    }
    Ok(roots)
}
pub(super) fn player_fingerprint(path: &Path) -> AppResult<String> {
    digest(&player_roots(path)?)
}
pub(super) fn inventory(roots: &[PathBuf]) -> AppResult<Vec<(String, PathBuf, u64, u128)>> {
    let mut result = Vec::new();
    for (index, root) in roots.iter().enumerate() {
        let mut pending = vec![root.clone()];
        while let Some(path) = pending.pop() {
            let meta = std::fs::symlink_metadata(&path)?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err(AppError::validation("版本源包含链接，无法确定性核对。"));
                }
            }
            if meta.file_type().is_symlink() {
                return Err(AppError::validation("版本源包含符号链接。"));
            }
            if meta.is_dir() {
                for item in std::fs::read_dir(path)? {
                    pending.push(item?.path());
                }
            } else {
                let key = format!(
                    "{index}/{}",
                    path.strip_prefix(root.parent().unwrap())
                        .map_err(AppError::internal)?
                        .to_string_lossy()
                        .replace('\\', "/")
                );
                let modified = meta
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(AppError::internal)?
                    .as_nanos();
                result.push((key, path, meta.len(), modified));
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}
pub(super) fn digest(roots: &[PathBuf]) -> AppResult<String> {
    let before = inventory(roots)?;
    let mut h = Sha256::new();
    for (key, path, _, _) in &before {
        h.update(key);
        h.update([0]);
        h.update(files::hash_file(path)?.0);
    }
    if before != inventory(roots)? {
        return Err(AppError::validation("版本文件正在变化，请完成保存后重试。"));
    }
    Ok(format!("{:x}", h.finalize()))
}
pub(super) fn verify(details: &Value) -> AppResult<()> {
    let archive = details["archiveSource"]
        .as_str()
        .ok_or_else(|| AppError::validation("缺少实际存档版本源，请使用 production version。"))?;
    let player = details["playerSource"]
        .as_str()
        .ok_or_else(|| AppError::validation("缺少实际 Player 版本源。"))?;
    if digest(&[archive_root(Path::new(archive))?])? != details["archiveHash"]
        || digest(&player_roots(Path::new(player))?)? != details["playerBuildHash"]
    {
        return Err(AppError::validation(
            "实际存档或 Player 文件已改变，请登记新版本并复验，不能沿用旧通过记录。",
        ));
    }
    Ok(())
}

impl TaskService {
    /// Read current task pins without touching workspaces, Skills or version history.
    pub fn pinned_player_paths(&self) -> AppResult<Vec<String>> {
        self.database.with_connection(|db| {
            let mut statement = db.prepare("SELECT body FROM task_production")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            let mut paths = Vec::new();
            for row in rows {
                let record: super::ProductionRecord = serde_json::from_str(&row?)?;
                if let Some(path) = record.version_details["playerSource"].as_str()
                    && !path.is_empty()
                {
                    paths.push(path.to_owned());
                }
            }
            Ok(paths)
        })
    }

    #[cfg(test)]
    pub fn production_capture_version(
        &self,
        task_id: &str,
        expected: u64,
        id: &str,
        sources: VersionSources,
    ) -> AppResult<ProductionView> {
        self.production_capture_version_for_purpose(task_id, expected, id, sources, "candidate")
    }
    pub fn production_capture_version_for_purpose(
        &self,
        task_id: &str,
        expected: u64,
        id: &str,
        sources: VersionSources,
        purpose: &str,
    ) -> AppResult<ProductionView> {
        if !["candidate", "fixture"].contains(&purpose) {
            return Err(AppError::validation("版本用途须为 candidate 或 fixture。"));
        }
        let _files = crate::foundation::EXECUTABLE_FILES.read();
        let root = self.production_workspace(task_id)?;
        let archive = archive_root(&sources.archive_path)?;
        let record = self
            .production_get(task_id)?
            .record
            .ok_or_else(|| AppError::validation("缺少制作记录。"))?;
        if record.revision != expected {
            return Err(super::conflict());
        }
        super::validation::approved(&record, &root, "requirements")?;
        super::validation::approved(&record, &root, "plan")?;
        if purpose == "fixture" && record.current_version.as_deref() == Some(id) {
            return Err(AppError::validation("测试夹具不能覆盖当前候选版本。"));
        }
        if let Some(old) = record.workbench.execution.validation_snapshots.get(id) {
            if purpose != "fixture" {
                return Err(AppError::validation("夹具 ID 不能作为交付候选。"));
            }
            verify(old)?;
            if old["archiveHash"] == digest(std::slice::from_ref(&archive))?
                && old["playerBuildHash"] == digest(&player_roots(&sources.player_path)?)?
            {
                return self.production_get(task_id);
            }
            return Err(AppError::validation("夹具内容改变须使用新的版本 ID。"));
        }
        if record.current_version.as_deref() == Some(id) {
            if record.version_details["archiveHash"] == digest(std::slice::from_ref(&archive))?
                && record.version_details["playerBuildHash"]
                    == digest(&player_roots(&sources.player_path)?)?
                && record.version_details["archiveGuid"] == sources.archive_guid
                && record.version_details["levelGuid"] == sources.level_guid
            {
                verify(&record.version_details)?;
                return self.production_get(task_id);
            }
            return Err(AppError::validation("版本内容改变时必须使用新的版本 ID。"));
        }
        let freeze = record.policy["implementationExecution"]["mode"] == "autonomous";
        let editable = archive.clone();
        let archive = if freeze {
            super::version_snapshots::copy_verified(
                &archive,
                &root
                    .join("artifacts/version-snapshots")
                    .join(uuid::Uuid::new_v4().to_string()),
            )?
        } else {
            archive
        };
        let player = sources.player_path.canonicalize()?;
        let mut content = super::version_snapshots::manifest(&archive)?;
        if super::execution_loop::enabled(&record) {
            super::validation_impact::enrich(&archive, &mut content)?;
        }
        let mut details = json!({"id":id,"archiveGuid":sources.archive_guid,"levelGuid":sources.level_guid,"instanceId":sources.instance_id,
            "archiveSource":archive,"playerSource":player,"archiveHash":digest(&[archive])?,"playerBuildHash":digest(&player_roots(&player)?)?});
        details["frozenSnapshot"] = json!(freeze);
        details["purpose"] = json!(purpose);
        details["scopeHash"] = json!(super::autonomy::scope(&record));
        details["cycle"] = json!(record.cycle);
        details["editableArchiveSource"] = json!(editable);
        details["contentManifest"] = content.clone();
        let difference = if super::execution_loop::enabled(&record) {
            super::validation_impact::difference
        } else {
            super::version_snapshots::difference
        };
        details["validationImpact"] = difference(
            &record.version_details["contentManifest"],
            &content,
            record.version_details["playerBuildHash"] != details["playerBuildHash"],
        );
        self.production_commit(task_id, expected, |r| {
            if purpose == "fixture" {
                if !freeze || r.workbench.execution.validation_snapshots.len() >= 128 {
                    return Err(AppError::validation(
                        "夹具快照需启用自主流程且不能超过128份。",
                    ));
                }
                r.workbench
                    .execution
                    .validation_snapshots
                    .insert(id.into(), details.clone());
                Ok(())
            } else {
                mutation::set_version(r, &root, &details)
            }
        })?;
        self.production_export(task_id)?;
        self.production_get(task_id)
    }
    pub fn production_activate_version(
        &self,
        task: &str,
        expected: u64,
        revision: u64,
        id: &str,
    ) -> AppResult<ProductionView> {
        let _files = crate::foundation::EXECUTABLE_FILES.read();
        let root = self.production_workspace(task)?;
        let saved = self.production_snapshot(task, revision)?;
        let current = self
            .production_get(task)?
            .record
            .ok_or_else(super::conflict)?;
        if current.revision != expected {
            return Err(super::conflict());
        }
        if current.current_stage != "implementation"
            || saved.current_version.as_deref() != Some(id)
            || saved.version_details["frozenSnapshot"] != true
            || saved.version_details["purpose"] == "fixture"
            || saved.cycle != current.cycle
            || super::autonomy::scope(&saved) != super::autonomy::scope(&current)
        {
            return Err(AppError::validation(
                "只能恢复本任务同范围、同周期的已登记冻结候选。",
            ));
        }
        verify(&saved.version_details)?;
        self.production_commit(task, expected, |r| {
            mutation::set_version(r, &root, &saved.version_details)
        })?;
        self.production_export(task)?;
        self.production_get(task)
    }
}
