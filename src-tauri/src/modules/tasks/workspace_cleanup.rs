use super::{AppError, AppResult, Path, PathBuf, TaskService, Uuid, read_task};
use rusqlite::OptionalExtension;
use std::fs;

impl TaskService {
    /// Returns true when the confirmed deletion still has files to clean up.
    pub fn delete(&self, id: &str, clean_workspace: bool) -> AppResult<bool> {
        let token = Uuid::new_v4().to_string();
        let queued = self.database.with_connection(|c| {
            let tx = rusqlite::Transaction::new_unchecked(c, rusqlite::TransactionBehavior::Immediate)?;
            Self::check_deletable(&tx, id)?;
            let task = read_task(&tx,id)?;
            let workspace = if clean_workspace { owned_directory(&tx,id,Path::new(&task.workspace_path),&self.workspace_root.read())? } else { None };
            tx.execute("DELETE FROM development_tasks WHERE id=?1",[id])?;
            tx.execute("DELETE FROM settings WHERE key LIKE ?1",[format!("chat-migration/{id}/%")])?;
            if let Some(path)=&workspace {
                tx.execute("INSERT INTO pending_workspace_deletions(token,task_id,original_path) VALUES(?1,?2,?3)",rusqlite::params![token,id,path.to_string_lossy()])?;
                // A replacement folder at the same path must never inherit deletion consent.
                fs::File::create_new(path.join(format!(".abya-cleanup-{token}"))).map_err(|e|AppError::validation(format!("无法登记清理目录 {}，请关闭占用该目录的程序后重试：{e}",path.display())))?;
            }
            if let Err(err)=tx.commit(){
                if let Some(path)=&workspace {let _=fs::remove_file(path.join(format!(".abya-cleanup-{token}")));}
                return Err(err.into());
            }
            Ok(workspace.is_some())
        })?;
        crate::foundation::change_signal::notify();
        if !queued {
            return Ok(false);
        }
        let _guard = self.cleanup_gate.lock();
        Ok(!self.finish_workspace_deletion(&token).unwrap_or(false))
    }

    pub fn retry_workspace_deletions(&self) -> AppResult<()> {
        let _guard = self.cleanup_gate.lock();
        let tokens = self.database.with_connection(|c| {
            let mut stmt = c.prepare("SELECT token FROM pending_workspace_deletions")?;
            Ok(stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?)
        })?;
        for token in tokens {
            if let Err(e) = self.finish_workspace_deletion(&token) {
                eprintln!("Workspace cleanup {token}: {e}");
            }
        }
        Ok(())
    }

    fn finish_workspace_deletion(&self, token: &str) -> AppResult<bool> {
        Uuid::parse_str(token).map_err(AppError::internal)?;
        let staged = self.database.with_connection(|c| {
            let Some((id, raw)) = c
                .query_row(
                    "SELECT task_id,original_path FROM pending_workspace_deletions WHERE token=?1",
                    [token],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .optional()?
            else {
                return Ok(None);
            };
            Uuid::parse_str(&id).map_err(AppError::internal)?;
            let path = Path::new(&raw);
            let temp = path.with_file_name(format!(".abya-delete-{id}-{token}"));
            if !path.is_absolute()
                || path
                    .components()
                    .any(|p| p == std::path::Component::ParentDir)
            {
                return Err(AppError::validation("无效的待清理目录。"));
            }
            for parent in temp.parent().into_iter().flat_map(Path::ancestors) {
                if let Ok(meta) = fs::symlink_metadata(parent)
                    && linked(&meta)
                {
                    return Err(AppError::validation("清理目录的上级现在是链接。"));
                }
            }
            match fs::symlink_metadata(&temp) {
                Ok(m) if m.is_dir() && !linked(&m) => return Ok(Some(temp)),
                Ok(_) => return Err(AppError::validation("待清理路径已被替换。")),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
            if let Some(original) = owned_directory(c, &id, path, &self.workspace_root.read())? {
                let marker = original.join(format!(".abya-cleanup-{token}"));
                match fs::symlink_metadata(marker) {
                    Ok(m) if m.is_file() && !linked(&m) => (),
                    Ok(_) => return Err(AppError::validation("清理标识已被替换。")),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(e.into()),
                }
                fs::rename(original, &temp)?;
                Ok(Some(temp))
            } else {
                Ok(None)
            }
        })?;
        if let Some(temp) = staged {
            fs::remove_dir_all(&temp)?;
        }
        self.database.with_connection(|c| {
            c.execute(
                "DELETE FROM pending_workspace_deletions WHERE token=?1",
                [token],
            )?;
            Ok(true)
        })
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
