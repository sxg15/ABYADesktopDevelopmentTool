use super::*;
use std::fs;

const BASE: &str = "存档";
static LAYOUT_GATE: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

pub(super) fn part(value: &str, max: usize) -> String {
    let s: String = value
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*_".contains(c) {
                '-'
            } else {
                c
            }
        })
        .take(max)
        .collect();
    s.trim_matches([' ', '.', '-']).to_owned()
}
fn normalized(s: &str) -> bool {
    let mut fields = s.split('_');
    let (Some(game), Some(feature), Some(time), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return false;
    };
    !game.is_empty()
        && !feature.is_empty()
        && chrono::NaiveDateTime::parse_from_str(time, "%Y%m%d-%H%M%S-%3f").is_ok()
}

fn name(game: &str, feature: &str, at: chrono::DateTime<chrono::Local>) -> String {
    let game = part(game, 28);
    let feature = part(feature, 22);
    format!(
        "{}-{}-{}",
        if game.is_empty() { "游戏" } else { &game },
        if feature.is_empty() {
            "编辑版"
        } else {
            &feature
        },
        at.format("%Y%m%d-%H%M%S-%3f")
    )
}
impl TaskService {
    fn check_save_migration_idle(&self, task: &str) -> AppResult<()> {
        if self.database.with_connection(|c|Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM game_instances WHERE task_id=?1 AND process_state IN ('running','launching','stopping'))",[task],|r|r.get::<_,bool>(0))?))? {return Err(AppError::validation("请先关闭本任务的游戏实例，再迁移存档。"));}
        Ok(())
    }
    pub fn project_save_folder(&self, task: &str) -> AppResult<PathBuf> {
        self.refresh_flat_saves(task)?;
        self.save_folder_raw(task)
    }
    pub(super) fn save_folder_raw(&self, task: &str) -> AppResult<PathBuf> {
        let t = self.get_readonly(task)?;
        let root = Path::new(&t.workspace_path);
        let folder = files::safe_path(root, BASE)?;
        fs::create_dir_all(&folder)?;
        Ok(folder)
    }
    pub fn project_archive_name(&self, task: &str, feature: &str) -> AppResult<String> {
        let t = self.get_readonly(task)?;
        let feature = feature
            .trim()
            .strip_prefix(t.title.trim())
            .unwrap_or(feature)
            .trim();
        Ok(name(&t.title, feature, chrono::Local::now()))
    }
    pub(super) fn archive_storage(&self, task: &str, kind: &str) -> AppResult<PathBuf> {
        let folder = self.archive_internal_root(task)?;
        files::safe_path(&folder, kind)
    }
    pub(super) fn migrate_save_layout(&self, task: &str) -> AppResult<()> {
        let _guard = LAYOUT_GATE.lock();
        let t = self.get_readonly(task)?;
        let root = Path::new(&t.workspace_path);
        let base = self.save_folder_raw(task)?;
        if root.join("artifacts/save-storage/.flat-layout").is_file() {
            return self.refresh_flat_saves(task);
        }
        let parent = root.parent().ok_or_else(conflict)?;
        let legacy_parent = files::safe_path(parent, ".abya-authoring")?;
        let old = legacy_parent.join(task);
        let dest = files::safe_path(&base, "编辑数据")?;
        let snaps = files::safe_path(root, "artifacts/version-snapshots")?;
        // Recover the short interval between relocating a directory and installing
        // its compatibility junction. Journal entries are constrained to this task.
        let journal = files::safe_path(&base, ".migration.json")?;
        if journal.is_file() {
            let saved: Value = serde_json::from_slice(&fs::read(&journal)?)?;
            if saved["task"] != task {
                return Err(conflict());
            }
            let from = mutation::text(&saved, "from")?;
            let label = mutation::text(&saved, "to")?;
            let (src, dst) = if from == "editor" && label == "编辑数据" {
                (old.clone(), dest.clone())
            } else {
                uuid::Uuid::parse_str(from).map_err(|_| conflict())?;
                if !normalized(label)
                    || Path::new(label).file_name().and_then(|n| n.to_str()) != Some(label)
                {
                    return Err(conflict());
                }
                (
                    snaps.join(from),
                    files::safe_path(&base, &format!("版本/{label}"))?,
                )
            };
            if !src.exists() && dst.is_dir() {
                compat_link(&src, &dst)?;
            }
            if !src.is_dir() || (dst.exists() && src.canonicalize()? != dst.canonicalize()?) {
                return Err(AppError::validation(
                    "存档迁移尚未完成，请核对原目录和目标目录。",
                ));
            }
            fs::remove_file(&journal)?;
        }
        let relocate = |src: &Path,
                        dst: &Path,
                        boundary: &Path,
                        relative: &str,
                        from: &str|
         -> AppResult<()> {
            if dst.is_dir() && src.canonicalize()? == dst.canonicalize()? {
                return Ok(());
            }
            files::safe_path(boundary, relative)?;
            if dst.exists() {
                return Err(AppError::validation("存档迁移目标已存在，未覆盖。"));
            }
            files::write_file(
                &base,
                ".migration.json",
                &serde_json::to_vec(
                    &json!({"task":task,"from":from,"to":dst.file_name().and_then(|n|n.to_str())}),
                )?,
            )?;
            fs::rename(src, dst)?;
            if let Err(e) = compat_link(src, dst) {
                fs::rename(dst, src)?;
                return Err(e);
            }
            fs::remove_file(&journal)?;
            Ok(())
        };
        if !old.exists() && !snaps.is_dir() && !dest.exists() {
            return Ok(());
        }
        if old.exists() {
            let owner: Value = serde_json::from_slice(&fs::read(old.join("owner.json"))?)?;
            if owner["taskId"] != task {
                return Err(AppError::validation("旧编辑目录不属于本任务。"));
            }
            if old.canonicalize().ok() != dest.canonicalize().ok() {
                self.check_save_migration_idle(task)?;
            }
            relocate(
                &old,
                &dest,
                parent,
                &format!(".abya-authoring/{task}"),
                "editor",
            )?;
        }
        // Keep the old PB directory name (part of its registered digest). Present a
        // normalized entry outside Archives so the Player never discovers a duplicate GUID.
        let archives = files::safe_path(&base, "编辑数据/Archives")?;
        if archives.is_dir() {
            for entry in fs::read_dir(&archives)? {
                let entry = entry?;
                let path = entry.path();
                if !entry.file_type()?.is_dir() || !path.join("Main.PBArc").is_file() {
                    continue;
                }
                let original = entry.file_name().to_string_lossy().into_owned();
                if normalized(&original) {
                    continue;
                }
                let meta = entry.metadata()?;
                let label = name(
                    &t.title,
                    &original,
                    meta.created().or_else(|_| meta.modified())?.into(),
                );
                let alias = base.join(label);
                if alias.exists() {
                    if alias.canonicalize()? != path.canonicalize()? {
                        return Err(AppError::validation("旧档显示名称冲突，请核对目录。"));
                    }
                } else {
                    compat_link(&alias, &path)?;
                }
            }
        }
        if snaps.is_dir() {
            files::safe_path(root, "artifacts/version-snapshots")?;
            let versions = files::safe_path(&base, "版本")?;
            fs::create_dir_all(&versions)?;
            for entry in fs::read_dir(&snaps)? {
                let entry = entry?;
                let key = entry.file_name().to_string_lossy().into_owned();
                if uuid::Uuid::parse_str(&key).is_err() {
                    continue;
                }
                let path = entry.path();
                if let Ok(real) = path.canonicalize()
                    && real.parent() == Some(versions.canonicalize()?.as_path())
                {
                    continue;
                }
                self.check_save_migration_idle(task)?;
                let meta = entry.metadata()?;
                if !meta.is_dir() {
                    continue;
                }
                let at = meta.created().or_else(|_| meta.modified())?;
                let feature=self.database.with_connection(|c|Ok(c.query_row("SELECT json_extract(body,'$.currentVersion') FROM (SELECT body FROM task_production WHERE task_id=?1 UNION ALL SELECT body FROM task_production_history WHERE task_id=?1) WHERE instr(json_extract(body,'$.versionDetails.archiveSource'),?2)>0 LIMIT 1",rusqlite::params![task,key],|r|r.get::<_,String>(0)).optional()?))?;
                let mut dest = versions.join(name(
                    &t.title,
                    feature.as_deref().unwrap_or(&key[..8]),
                    at.into(),
                ));
                if dest.exists() {
                    dest = versions.join(name(
                        &t.title,
                        &format!("{}-{}", &key[..8], feature.as_deref().unwrap_or("快照")),
                        at.into(),
                    ));
                }
                relocate(&path, &dest, &snaps, &key, &key)?;
            }
        }
        Ok(())
    }
}
#[cfg(windows)]
pub(super) fn compat_link(old: &Path, dest: &Path) -> AppResult<()> {
    use std::os::windows::process::CommandExt;
    let normal = |p: &Path| super::acceptance::launch_path(p.to_string_lossy().into_owned());
    let shell = PathBuf::from(std::env::var_os("SystemRoot").ok_or_else(conflict)?)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let out=std::process::Command::new(shell).creation_flags(0x08000000)
        .args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path ([Management.Automation.WildcardPattern]::Escape($env:ABYA_OLD_SAVE)) -Target $env:ABYA_NEW_SAVE | Out-Null"])
        .env("ABYA_OLD_SAVE",normal(old)).env("ABYA_NEW_SAVE",normal(dest)).output()?;
    if !out.status.success() {
        return Err(AppError::internal(String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}
#[cfg(not(windows))]
fn compat_link(old: &Path, dest: &Path) -> AppResult<()> {
    Ok(std::os::unix::fs::symlink(dest, old)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_have_three_safe_parts() {
        let label = name("A/B_C", "测试:一版", chrono::Local::now());
        assert!(label.starts_with("A-B-C-测试-一版-"));
        assert!(!label.contains('_'));
    }
}
