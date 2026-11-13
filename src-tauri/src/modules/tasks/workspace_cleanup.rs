use super::{AppError, AppResult, Path, PathBuf, TaskService, Uuid, read_task};
use std::fs;

impl TaskService {
    pub fn delete(&self, id: &str, clean_workspace: bool) -> AppResult<()> {
        let staged = self.database.with_connection(|c| {
            let tx =
                rusqlite::Transaction::new_unchecked(c, rusqlite::TransactionBehavior::Immediate)?;
            Self::check_deletable(&tx, id)?;
            let workspace = if clean_workspace {
                let task = read_task(&tx, id)?;
                owned_directory(
                    &tx,
                    id,
                    Path::new(&task.workspace_path),
                    &self.workspace_root.read(),
                )?
            } else {
                None
            };
            tx.execute("DELETE FROM development_tasks WHERE id=?1", [id])?;
            // Removing the original name in one operation leaves the task intact if the folder is locked.
            let staged = if let Some(path) = &workspace {
                let temp = path.with_file_name(format!(".abya-delete-{}-{}", id, Uuid::new_v4()));
                fs::rename(path, &temp)?;
                Some(temp)
            } else {
                None
            };
            if let Err(err) = tx.commit() {
                if let (Some(temp), Some(path)) = (&staged, &workspace) {
                    fs::rename(temp, path).map_err(|e| {
                        AppError::internal(format!(
                            "恢复工作区失败：{} → {}：{e}",
                            temp.display(),
                            path.display()
                        ))
                    })?;
                }
                return Err(err.into());
            }
            Ok(staged)
        })?;
        crate::foundation::change_signal::notify();
        if let Some(temp) = staged {
            fs::remove_dir_all(&temp).map_err(|e| {
                AppError::internal(format!(
                    "任务已删除，剩余文件可在 {} 清理：{e}",
                    temp.display()
                ))
            })?;
        }
        Ok(())
    }
}

fn comparable(path: &Path) -> PathBuf {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path
    }
}

fn owned_directory(
    c: &rusqlite::Connection,
    id: &str,
    path: &Path,
    root: &Path,
) -> AppResult<Option<PathBuf>> {
    Uuid::parse_str(id).map_err(AppError::internal)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !path.is_absolute()
        || path
            .components()
            .any(|p| p == std::path::Component::ParentDir)
        || !(name.ends_with(&format!("-{}", &id[..8])) || name.ends_with(&format!("-{id}")))
    {
        return Err(AppError::validation("无法确认该目录为此任务独占的工作区。"));
    }
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !meta.is_dir() || linked(&meta) {
        return Err(AppError::validation("工作区路径不是普通目录。"));
    }
    for ancestor in path.ancestors().skip(1) {
        if linked(&fs::symlink_metadata(ancestor)?) {
            return Err(AppError::validation("工作区位于链接目录中，无法安全清理。"));
        }
    }
    let target = comparable(path);
    if comparable(root).starts_with(&target) {
        return Err(AppError::validation("无法删除工作区根目录。"));
    }
    let mut query = c.prepare(
        "SELECT workspace_path FROM development_tasks WHERE id<>?1 AND workspace_path<>''",
    )?;
    for other in query.query_map([id], |row| row.get::<_, String>(0))? {
        let other = comparable(Path::new(&other?));
        if other.starts_with(&target) || target.starts_with(other) {
            return Err(AppError::validation("工作区与其他任务共用或嵌套。"));
        }
    }
    Ok(Some(path.canonicalize()?))
}

fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.is_symlink()
    }
}
