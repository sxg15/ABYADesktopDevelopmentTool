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
fn archive_root(path: &Path) -> AppResult<PathBuf> {
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
fn inventory(roots: &[PathBuf]) -> AppResult<Vec<(String, PathBuf, u64, u128)>> {
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
fn digest(roots: &[PathBuf]) -> AppResult<String> {
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
    pub fn production_capture_version(
        &self,
        task_id: &str,
        expected: u64,
        id: &str,
        sources: VersionSources,
    ) -> AppResult<ProductionView> {
        let root = self.production_workspace(task_id)?;
        let archive = archive_root(&sources.archive_path)?;
        let player = sources.player_path.canonicalize()?;
        let details = json!({"id":id,"archiveGuid":sources.archive_guid,"levelGuid":sources.level_guid,"instanceId":sources.instance_id,
            "archiveSource":archive,"playerSource":player,"archiveHash":digest(&[archive])?,"playerBuildHash":digest(&player_roots(&player)?)?});
        self.production_commit(task_id, expected, |r| {
            mutation::set_version(r, &root, &details)
        })?;
        self.production_export(task_id)?;
        self.production_get(task_id)
    }
}
