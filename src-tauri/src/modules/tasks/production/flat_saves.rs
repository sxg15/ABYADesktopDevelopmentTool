use super::save_layout::{compat_link, part};
use super::*;
use std::{fs, time::SystemTime};
static GATE: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
const KINDS: [&str; 4] = ["编辑数据", "版本", "试玩副本", "验证副本"];

fn link(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
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
fn newest(root: &Path) -> AppResult<SystemTime> {
    let mut latest = SystemTime::UNIX_EPOCH;
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        if link(&path) {
            continue;
        }
        let m = fs::metadata(&path)?;
        if m.is_dir() {
            for item in fs::read_dir(path)? {
                pending.push(item?.path())
            }
        } else {
            latest = latest.max(m.modified()?);
        }
    }
    Ok(latest)
}
#[cfg(windows)]
fn hide(path: &Path) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM, GetFileAttributesW, INVALID_FILE_ATTRIBUTES,
        SetFileAttributesW,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // Set attributes on the directory entry, including junctions, not their targets.
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == INVALID_FILE_ATTRIBUTES {
        return Err(std::io::Error::last_os_error().into());
    }
    if unsafe {
        SetFileAttributesW(
            wide.as_ptr(),
            attrs | FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn hide(_path: &Path) -> AppResult<()> {
    Ok(())
}

impl TaskService {
    pub(super) fn archive_internal_root(&self, task: &str) -> AppResult<PathBuf> {
        let root = self.production_workspace(task)?;
        let dir = files::safe_path(&root, "artifacts/save-storage")?;
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
    pub(super) fn refresh_flat_saves(&self, task: &str) -> AppResult<()> {
        let _guard = GATE.lock();
        let task_record = self.get_readonly(task)?;
        let base = self.save_folder_raw(task)?;
        let storage = self.archive_internal_root(task)?;
        let mut records: Value = if storage.join("entries.json").is_file() {
            serde_json::from_slice(&fs::read(storage.join("entries.json"))?)?
        } else {
            json!({})
        };
        if !records.is_object() {
            return Err(conflict());
        }
        files::write_file(&storage, ".flat-layout", b"1")?;
        for kind in KINDS {
            let old = base.join(kind);
            let dest = storage.join(kind);
            if !link(&old) && old.is_dir() {
                if self.database.with_connection(|c|Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM game_instances WHERE task_id=?1 AND process_state IN ('running','launching','stopping'))",[task],|r|r.get::<_,bool>(0))?))?{return Err(AppError::validation("请先关闭本任务的运行实例，再整理存档。"))}
                if dest.exists() {
                    return Err(AppError::validation("内部存档目录已存在，请核对迁移位置。"));
                }
                fs::rename(&old, &dest)?;
            }
            if dest.is_dir() {
                if !old.exists() {
                    compat_link(&old, &dest)?;
                }
                if old.canonicalize()? != dest.canonicalize()? {
                    return Err(conflict());
                }
                hide(&old)?;
            }
        }
        files::write_file(&storage, ".flat-layout", b"1")?;
        // Remove only legacy presentation links pointing into this task's storage.
        for entry in fs::read_dir(&base)? {
            let entry = entry?;
            let p = entry.path();
            let n = entry.file_name().to_string_lossy().into_owned();
            if !KINDS.contains(&n.as_str())
                && !records
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|v| v.as_str() == Some(&n))
                && link(&p)
                && p.canonicalize().is_ok_and(|v| v.starts_with(&storage))
            {
                fs::remove_dir(p)?;
            }
        }
        let production = self.database.with_connection(|c| load(c, task))?;
        for kind in KINDS {
            let dir = storage.join(kind);
            if !dir.is_dir() {
                continue;
            }
            let mut pending = vec![dir];
            while let Some(path) = pending.pop() {
                if link(&path) {
                    continue;
                }
                if path.join("Main.PBArc").is_file() {
                    let key = path
                        .strip_prefix(&storage)
                        .map_err(AppError::internal)?
                        .to_string_lossy()
                        .replace('\\', "/");
                    let data: Value = serde_json::from_slice(&fs::read(path.join("Main.PBArc"))?)?;
                    let game = data["Name"]
                        .as_str()
                        .unwrap_or(&task_record.title)
                        .split('_')
                        .next()
                        .unwrap_or(&task_record.title);
                    let feature = if kind == "版本" {
                        path.parent()
                            .and_then(Path::file_name)
                            .and_then(|v| v.to_str())
                            .unwrap_or("版本")
                            .to_owned()
                    } else if ["试玩副本", "验证副本"].contains(&kind) {
                        let owner = path
                            .parent()
                            .and_then(Path::parent)
                            .map(|p| p.join("owner.json"));
                        let owner: Value = owner
                            .filter(|p| p.is_file())
                            .map(|p| {
                                fs::read(p).map_err(AppError::from).and_then(|b| {
                                    serde_json::from_slice(&b).map_err(AppError::from)
                                })
                            })
                            .transpose()?
                            .unwrap_or(Value::Null);
                        format!(
                            "{}{}",
                            owner["version"]
                                .as_str()
                                .or_else(|| production.as_ref()?.current_version.as_deref())
                                .unwrap_or("运行版"),
                            if kind == "试玩副本" {
                                "试玩"
                            } else {
                                "验证"
                            }
                        )
                    } else {
                        "编辑版".into()
                    };
                    let date: chrono::DateTime<chrono::Local> = newest(&path)?.into();
                    let feature = feature.split('_').nth(1).unwrap_or(&feature);
                    let label = format!(
                        "{}-{}-{}",
                        part(game, 28),
                        part(feature, 35),
                        date.format("%Y%m%d-%H%M%S")
                    );
                    let mut alias = base.join(&label);
                    if alias.exists() && alias.canonicalize()? != path.canonicalize()? {
                        use sha2::{Digest, Sha256};
                        let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
                        alias = base.join(format!(
                            "{}-{}-{}-{}",
                            part(game, 28),
                            part(feature, 25),
                            &hash[..6],
                            date.format("%Y%m%d-%H%M%S")
                        ));
                    }
                    let old = records[&key].as_str().map(str::to_owned);
                    if !alias.exists() {
                        compat_link(&alias, &path)?;
                    }
                    if alias.canonicalize()? != path.canonicalize()? {
                        return Err(AppError::validation("存档显示名称冲突。"));
                    }
                    let filename = alias
                        .file_name()
                        .and_then(|x| x.to_str())
                        .ok_or_else(conflict)?;
                    if let Some(old) = old
                        && old != filename
                    {
                        if Path::new(&old).file_name().and_then(|v| v.to_str()) != Some(&old) {
                            return Err(conflict());
                        }
                        let previous = base.join(&old);
                        if link(&previous)
                            && previous.canonicalize().ok() == path.canonicalize().ok()
                        {
                            fs::remove_dir(previous)?;
                        }
                    }
                    records[&key] = json!(filename);
                } else {
                    for entry in fs::read_dir(path)? {
                        let entry = entry?;
                        if entry.file_type()?.is_dir() {
                            pending.push(entry.path())
                        }
                    }
                }
            }
        }
        files::write_file(&storage, "entries.json", &serde_json::to_vec(&records)?)?;
        files::write_file(&storage, ".flat-layout", b"1")?;
        Ok(())
    }
}
