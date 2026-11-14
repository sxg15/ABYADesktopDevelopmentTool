use super::{ROOT, SkillEntry, TaskService, catalog, files, load, now, policy};
use crate::foundation::{AppError, AppResult};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMigration {
    pub title: String,
    pub status: String,
    pub reason: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Receipt {
    release: u64,
    batch: String,
    export_pending: bool,
    changed: bool,
}
#[derive(Serialize, Deserialize)]
struct Swap {
    path: String,
    old: Option<String>,
    new: String,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    task_id: String,
    workspace: PathBuf,
    batch: uuid::Uuid,
    swaps: Vec<Swap>,
}

fn recovery_dir(batch: uuid::Uuid) -> String {
    format!("{ROOT}/release-backups/{batch}")
}

// Leave unrecognized fields in older/imported project records intact. Only
// serialize the parts this release actually adapted.
fn apply_changes(
    raw: &mut serde_json::Value,
    before: &serde_json::Value,
    after: &serde_json::Value,
) {
    if before == after {
        return;
    }
    if let (Some(dest), Some(old), Some(new)) =
        (raw.as_object_mut(), before.as_object(), after.as_object())
    {
        for (key, value) in new {
            if old.get(key) == Some(value) {
                continue;
            }
            match (dest.get_mut(key), old.get(key)) {
                (Some(existing), Some(previous)) => apply_changes(existing, previous, value),
                _ => {
                    dest.insert(key.clone(), value.clone());
                }
            }
        }
        for key in old.keys().filter(|key| !new.contains_key(*key)) {
            dest.remove(key);
        }
    } else {
        raw.clone_from(after);
    }
}
fn read_receipt(c: &rusqlite::Connection, key: &str) -> AppResult<Receipt> {
    let raw: Option<String> = c
        .query_row("SELECT value_json FROM settings WHERE key=?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(raw
        .map(|v| serde_json::from_str(&v))
        .transpose()?
        .unwrap_or_default())
}
fn save_receipt(c: &rusqlite::Connection, key: &str, value: &Receipt) -> AppResult<()> {
    c.execute(
        "INSERT OR REPLACE INTO settings(key,value_json,updated_at) VALUES(?1,?2,?3)",
        params![key, serde_json::to_string(value)?, now()],
    )?;
    Ok(())
}
fn slot(root: &Path, rel: &str) -> AppResult<PathBuf> {
    let p = files::safe_path(root, rel)?;
    fs::create_dir_all(p.parent().unwrap())?;
    Ok(p)
}

impl Journal {
    fn apply(&self, root: &Path) -> AppResult<()> {
        let backup = recovery_dir(self.batch);
        for s in &self.swaps {
            let live = slot(root, &s.path)?;
            let original = if live.exists() {
                Some(catalog::tree_hash(root, &s.path)?)
            } else {
                None
            };
            if original != s.old {
                return Err(AppError::validation("暂存期间项目文件有变化，请重新核对。"));
            }
            if s.old.is_some() {
                fs::rename(&live, slot(root, &format!("{backup}/old/{}", s.path))?)?;
            }
            fs::rename(
                files::safe_path(root, &format!("{backup}/new/{}", s.path))?,
                &live,
            )?;
        }
        Ok(())
    }

    fn rollback(&self, root: &Path) -> AppResult<()> {
        let backup = recovery_dir(self.batch);
        for s in self.swaps.iter().rev() {
            let live = files::safe_path(root, &s.path)?;
            let old = files::safe_path(root, &format!("{backup}/old/{}", s.path))?;
            if s.old.is_some() && !old.exists() {
                continue;
            } // Swap not started, or already restored.
            if old.exists()
                && Some(catalog::tree_hash(
                    &files::safe_path(root, &format!("{backup}/old"))?,
                    &s.path,
                )?) != s.old
            {
                return Err(AppError::validation("备份校验失败，请核对恢复目录。"));
            }
            if s.old.is_none()
                && files::safe_path(root, &format!("{backup}/new/{}", s.path))?.exists()
            {
                continue; // The staged addition has never been installed.
            }
            if live.exists() {
                if catalog::tree_hash(root, &s.path)? != s.new {
                    return Err(AppError::validation(
                        "迁移中断后文件有新修改，请核对项目备份。",
                    ));
                }
                let retired = slot(root, &format!("{backup}/interrupted/{}", s.path))?;
                fs::rename(&live, retired)?;
            }
            if old.exists() {
                fs::rename(old, live)?;
            }
        }
        Ok(())
    }
}

impl TaskService {
    pub fn migrate_desktop_projects(
        &self,
        release: u64,
        package: &Path,
        recovery: &Path,
    ) -> AppResult<Vec<ProjectMigration>> {
        let bundle = catalog::scan(package)?
            .into_iter()
            .filter(|s| s.kind != "user")
            .collect::<Vec<_>>();
        if !bundle.iter().any(|s| s.kind == "workflow") {
            return Err(AppError::validation("正式包缺少制作流程 Skill。"));
        }
        let mut backed_up = false;
        let mut report = Vec::new();
        for task in self.list_readonly()? {
            let result =
                self.migrate_project(&task, release, package, &bundle, recovery, &mut || {
                    if !backed_up {
                        fs::create_dir_all(recovery)?;
                        let snapshot = recovery
                            .join(format!("desktop-{release}-{}.sqlite", uuid::Uuid::new_v4()));
                        self.database.with_connection(|c| {
                            c.execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])?;
                            let check: String = rusqlite::Connection::open_with_flags(
                                &snapshot,
                                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                            )?
                            .query_row(
                                "PRAGMA quick_check",
                                [],
                                |r| r.get(0),
                            )?;
                            if check != "ok" {
                                return Err(AppError::validation("迁移前数据库备份校验失败。"));
                            }
                            Ok(())
                        })?;
                        backed_up = true;
                    }
                    Ok(())
                });
            let (status, reason) = match result {
                Ok(true) => ("adapted", String::new()),
                Ok(false) => ("retained", String::new()),
                Err(e) => ("review", e.to_string()),
            };
            report.push(ProjectMigration {
                title: task.title,
                status: status.into(),
                reason,
            });
        }
        Ok(report)
    }

    fn migrate_project(
        &self,
        task: &super::super::DevelopmentTask,
        release: u64,
        package: &Path,
        bundle: &[SkillEntry],
        recovery: &Path,
        backup_db: &mut impl FnMut() -> AppResult<()>,
    ) -> AppResult<bool> {
        self.migrate_save_layout(&task.id)?;
        self.refresh_flat_saves(&task.id)?;
        let key = format!("desktop-project-migration/{}", task.id);
        let mut receipt = self.database.with_connection(|c| read_receipt(c, &key))?;
        let Some(mut record) = self.database.with_connection(|c| load(c, &task.id))? else {
            return Ok(false); // No pinned production flow. Normal workspace provisioning owns these tasks.
        };
        let root = Path::new(&task.workspace_path);
        if !root.is_absolute() || !root.is_dir() {
            return Err(AppError::validation(
                "项目工作区不可用，请连接原目录后重启。",
            ));
        }
        let task_uuid = uuid::Uuid::parse_str(&task.id).map_err(AppError::internal)?;
        let journal_name = format!("pending-{task_uuid}.json");
        let pending = recovery.join(&journal_name);
        if pending.exists() {
            let journal: Journal = serde_json::from_slice(&fs::read(&pending)?)?;
            if journal.task_id != task.id
                || journal.workspace != root.canonicalize()?
                || journal.swaps.iter().any(|s| {
                    !s.path.strip_prefix(".codex/skills/").is_some_and(|name| {
                        !name.is_empty()
                            && name
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    })
                })
            {
                return Err(AppError::validation("恢复记录与当前项目不符。"));
            }
            if receipt.batch == journal.batch.to_string() {
                fs::remove_file(&pending)?;
            } else {
                journal.rollback(root)?;
                fs::remove_file(&pending)?;
            }
        }
        if receipt.export_pending {
            self.production_export(&task.id)?;
            receipt.export_pending = false;
            self.database
                .with_connection(|c| save_receipt(c, &key, &receipt))?;
        }
        if receipt.release > release {
            return Err(AppError::validation(
                "此项目已在更新的桌面发布包中打开，请使用新版。",
            ));
        }
        if receipt.release == release {
            return Ok(receipt.changed);
        }
        let next = policy();
        let version = |s: &str| {
            s.split('.')
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
        };
        if version(&record.workflow_version)
            .ok()
            .zip(version(next["workflowVersion"].as_str().unwrap_or("")).ok())
            .is_none_or(|(old, new)| old > new)
        {
            return Err(AppError::validation(
                "项目流程版本较新或无法识别，请用相应版本打开。",
            ));
        }
        let prior_revision = record.revision;
        let before = serde_json::to_value(&record)?;
        let batch = uuid::Uuid::new_v4();
        let dir = recovery_dir(batch);
        let policy_changed = record.policy != next;
        if policy_changed {
            for field in [
                "rounds",
                "questions",
                "dimensions",
                "stages",
                "checkStatuses",
                "stageStatuses",
            ] {
                if record.policy[field] != next[field] {
                    return Err(AppError::validation(
                        "新版阶段契约有变化，需要核对后手动升级。",
                    ));
                }
            }
            let stage = record.current_stage.clone();
            let stages = record.stages.clone();
            catalog::upgrade_policy(&mut record, &dir, true)?;
            if record.current_stage != stage || record.stages != stages {
                return Err(AppError::validation(
                    "流程升级需要调整项目阶段，请人工核对。",
                ));
            }
        }
        let mut swaps = Vec::new();
        for target in bundle {
            let pin = record.skill_pins.iter().find(|p| p.path == target.path);
            let live = files::safe_path(root, &target.path)?;
            let old = if live.exists() {
                Some(catalog::tree_hash(root, &target.path)?)
            } else {
                None
            };
            if pin.is_some_and(|p| p.kind == "user")
                || pin.map(|p| p.sha256.as_str()) != old.as_deref()
            {
                return Err(AppError::validation(format!(
                    "{} 有本地修改、缺失或名称冲突，已保留原项目。",
                    target.id
                )));
            }
            if old.as_ref() != Some(&target.sha256) {
                swaps.push(Swap {
                    path: target.path.clone(),
                    old,
                    new: target.sha256.clone(),
                });
                record.skill_pins.retain(|p| p.path != target.path);
                record.skill_pins.push(target.clone());
            }
        }
        let changed = policy_changed || !swaps.is_empty();
        let mut next_receipt = Receipt {
            release,
            batch: batch.to_string(),
            changed,
            export_pending: changed,
        };
        if !changed {
            self.database
                .with_connection(|c| save_receipt(c, &key, &next_receipt))?;
            return Ok(false);
        }

        if crate::foundation::cli_sessions::task_active(&task.id) {
            return Err(AppError::validation("请结束当前任务终端后重启以适配项目。"));
        }
        backup_db()?;
        for s in &swaps {
            let staged = slot(root, &format!("{dir}/new/{}", s.path))?;
            super::super::copy_directory(&files::safe_path(package, &s.path)?, &staged)?;
            let staged_root = files::safe_path(root, &format!("{dir}/new"))?;
            if catalog::tree_hash(&staged_root, &s.path)? != s.new {
                return Err(AppError::validation("新版 Skill 暂存校验失败。"));
            }
        }
        record.revision += 1;
        record.updated_at = now();
        let after = serde_json::to_value(&record)?;
        let journal = Journal {
            batch,
            swaps,
            task_id: task.id.clone(),
            workspace: root.canonicalize()?,
        };
        files::write_file(recovery, &journal_name, &serde_json::to_vec(&journal)?)?;
        let result = self.database.with_connection(|c| {
            let tx = c.unchecked_transaction()?;
            let raw: String = tx
                .query_row(
                    "SELECT body FROM task_production WHERE task_id=?1 AND revision=?2",
                    params![task.id, prior_revision as i64],
                    |r| r.get(0),
                )
                .optional()?
                .ok_or_else(super::conflict)?;
            let mut updated: serde_json::Value = serde_json::from_str(&raw)?;
            apply_changes(&mut updated, &before, &after);
            let body = serde_json::to_string(&updated)?;
            if body.len() > 8 * 1024 * 1024 {
                return Err(AppError::validation("适配后的制作记录超过大小限制。"));
            }
            tx.execute(
                "INSERT INTO task_production_history(task_id,revision,body) VALUES(?1,?2,?3)",
                params![task.id, prior_revision as i64, raw],
            )?;
            journal.apply(root)?;
            for s in &journal.swaps {
                if catalog::tree_hash(root, &s.path)? != s.new {
                    return Err(AppError::validation("适配后的 Skill 校验失败。"));
                }
            }
            tx.execute(
                "UPDATE task_production SET revision=?2,body=?3 WHERE task_id=?1",
                params![task.id, record.revision as i64, body],
            )?;
            save_receipt(&tx, &key, &next_receipt)?;
            tx.commit()?;
            Ok(())
        });
        if let Err(error) = result {
            journal.rollback(root)?;
            fs::remove_file(&pending)?;
            return Err(error);
        }
        fs::remove_file(pending)?;
        self.production_export(&task.id)?;
        next_receipt.export_pending = false;
        self.database
            .with_connection(|c| save_receipt(c, &key, &next_receipt))?;
        Ok(true)
    }
}
