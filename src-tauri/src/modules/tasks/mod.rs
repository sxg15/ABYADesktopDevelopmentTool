use crate::foundation::{AppError, AppResult, Database};
use chrono::Utc;
use parking_lot::RwLock;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const MANAGED_SKILL_NAME: &str = "abya-game-development-task";
const MANAGED_LLM_DIRECTORIES: [&str; 2] = [".codex", ".grok"];
const TEMPLATE_KINDS: [&str; 2] = ["abya-task-template", "abya-art-template"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskTemplateMarker {
    schema_version: u32,
    kind: String,
    id: String,
    display_name: String,
    description: String,
    source_skill_name: String,
}

fn is_managed_template(directory: &Path, name: &str) -> bool {
    let Some(kind) = TEMPLATE_KINDS
        .iter()
        .find(|kind| name.starts_with(&format!("{kind}-")))
    else {
        return false;
    };
    if name.len() <= kind.len() + 1
        || name.len() > 64
        || !name
            .bytes()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == b'-')
        || !directory.join("SKILL.md").is_file()
    {
        return false;
    }
    std::fs::read(directory.join(format!("{kind}.json")))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<TaskTemplateMarker>(&bytes).ok())
        .is_some_and(|marker| {
            marker.schema_version == 1
                && marker.kind == *kind
                && marker.id == name
                && !marker.display_name.trim().is_empty()
                && !marker.description.trim().is_empty()
                && !marker.source_skill_name.trim().is_empty()
        })
}

#[derive(Clone)]
struct ManagedSkillSource {
    llm_directory: &'static str,
    source: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub archived_at: Option<String>,
    pub workspace_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Active,
    Completed,
    Archived,
}

impl TaskStatus {
    fn as_db(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }

    fn from_db(value: &str) -> Self {
        match value {
            "completed" => Self::Completed,
            "archived" => Self::Archived,
            _ => Self::Active,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub title: String,
    pub description: String,
}

#[derive(Clone)]
pub struct TaskService {
    database: Database,
    workspace_root: Arc<RwLock<PathBuf>>,
    managed_skill_sources: Vec<ManagedSkillSource>,
}

impl TaskService {
    pub fn new(database: Database, workspace_root: PathBuf) -> Self {
        Self {
            database,
            workspace_root: Arc::new(RwLock::new(workspace_root)),
            managed_skill_sources: MANAGED_LLM_DIRECTORIES
                .into_iter()
                .map(|llm_directory| ManagedSkillSource {
                    llm_directory,
                    source: discover_managed_skill_source(llm_directory),
                })
                .collect(),
        }
    }

    pub fn set_workspace_root(&self, workspace_root: PathBuf) {
        *self.workspace_root.write() = workspace_root;
    }

    pub fn list(&self) -> AppResult<Vec<DevelopmentTask>> {
        let tasks = self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id,title,description,status,created_at,updated_at,completed_at,archived_at,workspace_path
                 FROM development_tasks
                 ORDER BY CASE status WHEN 'active' THEN 0 WHEN 'completed' THEN 1 ELSE 2 END, updated_at DESC",
            )?;
            let rows = statement.query_map([], map_task)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })?;
        tasks
            .into_iter()
            .map(|task| self.ensure_workspace(task))
            .collect()
    }

    pub fn get(&self, id: &str) -> AppResult<DevelopmentTask> {
        let task = self
            .database
            .with_connection(|connection| read_task(connection, id))?;
        self.ensure_workspace(task)
    }

    pub fn create(&self, input: TaskInput) -> AppResult<DevelopmentTask> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err(AppError::validation("Task title is required."));
        }
        let now = Utc::now().to_rfc3339();
        let task = DevelopmentTask {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            description: input.description.trim().to_string(),
            status: TaskStatus::Active,
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
            archived_at: None,
            workspace_path: String::new(),
        };
        let workspace_path = self.create_workspace(&task.id, &task.title)?;
        self.sync_managed_skills(&workspace_path)?;
        let mut task = task;
        task.workspace_path = display_path(&workspace_path);
        if let Err(error) = self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO development_tasks(id,title,description,status,created_at,updated_at,workspace_path)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    task.id,
                    task.title,
                    task.description,
                    task.status.as_db(),
                    task.created_at,
                    task.updated_at,
                    task.workspace_path
                ],
            )?;
            Ok(())
        }) {
            let _ = std::fs::remove_dir_all(&workspace_path);
            return Err(error);
        }
        Ok(task)
    }

    pub fn update(&self, id: &str, input: TaskInput) -> AppResult<DevelopmentTask> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err(AppError::validation("Task title is required."));
        }
        self.database.with_connection(|connection| {
            let count = connection.execute(
                "UPDATE development_tasks SET title=?2,description=?3,updated_at=?4 WHERE id=?1",
                rusqlite::params![id, title, input.description.trim(), Utc::now().to_rfc3339()],
            )?;
            if count == 0 {
                return Err(AppError::not_found("Task"));
            }
            read_task(connection, id)
        })
    }

    pub fn set_status(&self, id: &str, status: TaskStatus) -> AppResult<DevelopmentTask> {
        let now = Utc::now().to_rfc3339();
        let completed = matches!(status, TaskStatus::Completed).then_some(now.clone());
        let archived = matches!(status, TaskStatus::Archived).then_some(now.clone());
        self.database.with_connection(|connection| {
            let count = connection.execute(
                "UPDATE development_tasks
                 SET status=?2,updated_at=?3,completed_at=?4,archived_at=?5 WHERE id=?1",
                rusqlite::params![id, status.as_db(), now, completed, archived],
            )?;
            if count == 0 {
                return Err(AppError::not_found("Task"));
            }
            read_task(connection, id)
        })
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        self.ensure_deletable(id)?;
        self.database.with_connection(|connection| {
            let count = connection.execute("DELETE FROM development_tasks WHERE id=?1", [id])?;
            if count == 0 {
                return Err(AppError::not_found("Task"));
            }
            Ok(())
        })
    }

    pub fn ensure_deletable(&self, id: &str) -> AppResult<()> {
        self.database.with_connection(|connection| {
            let exists: i64 = connection.query_row(
                "SELECT COUNT(*) FROM development_tasks WHERE id=?1",
                [id],
                |row| row.get(0),
            )?;
            if exists == 0 {
                return Err(AppError::not_found("Task"));
            }
            let running: i64 = connection.query_row(
                "SELECT COUNT(*) FROM game_instances
                 WHERE task_id=?1 AND process_state IN ('launching','running','stopping')",
                [id],
                |row| row.get(0),
            )?;
            if running > 0 {
                return Err(AppError::validation(
                    "Stop all running instances before deleting the task.",
                ));
            }
            Ok(())
        })
    }
}

fn read_task(connection: &rusqlite::Connection, id: &str) -> AppResult<DevelopmentTask> {
    connection
        .query_row(
            "SELECT id,title,description,status,created_at,updated_at,completed_at,archived_at,workspace_path
             FROM development_tasks WHERE id=?1",
            [id],
            map_task,
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Task"))
}

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<DevelopmentTask> {
    let status: String = row.get(3)?;
    Ok(DevelopmentTask {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: TaskStatus::from_db(&status),
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        completed_at: row.get(6)?,
        archived_at: row.get(7)?,
        workspace_path: row.get(8)?,
    })
}

impl TaskService {
    fn ensure_workspace(&self, mut task: DevelopmentTask) -> AppResult<DevelopmentTask> {
        let workspace = if task.workspace_path.trim().is_empty() {
            self.create_workspace(&task.id, &task.title)?
        } else {
            PathBuf::from(&task.workspace_path)
        };
        std::fs::create_dir_all(&workspace)?;
        if !workspace.is_dir() {
            return Err(AppError::validation(
                "The task workspace path must be a directory.",
            ));
        }
        self.sync_managed_skills(&workspace)?;
        let display = display_path(&workspace);
        if task.workspace_path != display {
            self.database.with_connection(|connection| {
                connection.execute(
                    "UPDATE development_tasks SET workspace_path=?2 WHERE id=?1",
                    rusqlite::params![task.id, display],
                )?;
                Ok(())
            })?;
            task.workspace_path = display;
        }
        Ok(task)
    }

    fn create_workspace(&self, task_id: &str, title: &str) -> AppResult<PathBuf> {
        let root = self.workspace_root.read().clone();
        std::fs::create_dir_all(&root)?;
        let directory = root.join(format!("{}-{}", slugify(title), &task_id[..8]));
        std::fs::create_dir_all(&directory)?;
        Ok(directory)
    }

    fn sync_managed_skills(&self, workspace: &Path) -> AppResult<()> {
        for managed_skill in &self.managed_skill_sources {
            self.sync_managed_skill(workspace, managed_skill)?;
            if let Some(root) = managed_skill
                .source
                .as_ref()
                .and_then(|source| source.parent())
            {
                for entry in std::fs::read_dir(root)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if entry.file_type()?.is_dir() && is_managed_template(&entry.path(), &name) {
                        sync_skill_directory(
                            workspace,
                            managed_skill.llm_directory,
                            &entry.path(),
                            &name,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn sync_managed_skill(
        &self,
        workspace: &Path,
        managed_skill: &ManagedSkillSource,
    ) -> AppResult<()> {
        let source = managed_skill.source.as_ref().ok_or_else(|| {
            AppError::new(
                "managedSkillUnavailable",
                format!(
                    "The bundled ABYA game-development task Skill for {} was not found.",
                    managed_skill.llm_directory
                ),
                format!(
                    "{}/skills/{MANAGED_SKILL_NAME}",
                    managed_skill.llm_directory
                ),
            )
        })?;
        sync_skill_directory(
            workspace,
            managed_skill.llm_directory,
            source,
            MANAGED_SKILL_NAME,
        )
    }
}

fn sync_skill_directory(
    workspace: &Path,
    llm_directory: &str,
    source: &Path,
    name: &str,
) -> AppResult<()> {
    let skills_root = workspace.join(llm_directory).join("skills");
    std::fs::create_dir_all(&skills_root)?;
    let workspace_resolved = workspace.canonicalize()?;
    let skills_resolved = skills_root.canonicalize()?;
    if !skills_resolved.starts_with(&workspace_resolved) {
        return Err(AppError::validation(
            "The managed Skill destination is outside the task workspace.",
        ));
    }
    let target = skills_root.join(name);
    if target.exists() && name != MANAGED_SKILL_NAME && !is_managed_template(&target, name) {
        return Err(AppError::validation(format!(
            "Refusing to overwrite a user-owned Skill: {name}"
        )));
    }
    if target.is_symlink() {
        return Err(AppError::validation(
            "Refusing to replace a linked Skill destination.",
        ));
    }
    if target.is_dir() && directories_equal(source, &target)? {
        return Ok(());
    }
    let temporary = skills_root.join(format!(".{name}-{}", Uuid::new_v4()));
    if let Err(error) = copy_directory(source, &temporary) {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(error);
    }

    let workspace_resolved = workspace.canonicalize()?;
    let skills_resolved = skills_root.canonicalize()?;
    if !skills_resolved.starts_with(&workspace_resolved) {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(AppError::validation(
            "The managed Skill destination is outside the task workspace.",
        ));
    }
    if target.exists() {
        let target_resolved = match target.canonicalize() {
            Ok(value) => value,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&temporary);
                return Err(error.into());
            }
        };
        if !target_resolved.starts_with(&workspace_resolved)
            || target.file_name() != Some(std::ffi::OsStr::new(name))
        {
            let _ = std::fs::remove_dir_all(&temporary);
            return Err(AppError::validation(
                "Refusing to replace an invalid managed Skill destination.",
            ));
        }
        std::fs::remove_dir_all(&target)?;
    }
    if let Err(error) = std::fs::rename(&temporary, &target) {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(error.into());
    }
    Ok(())
}

fn discover_managed_skill_source(llm_directory: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(current) = std::env::current_dir() {
        roots.push(current.clone());
        if let Some(parent) = current.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        roots.push(parent.to_path_buf());
    }
    roots
        .into_iter()
        .map(|root| {
            root.join(llm_directory)
                .join("skills")
                .join(MANAGED_SKILL_NAME)
        })
        .find(|candidate| candidate.is_dir())
}

fn copy_directory(source: &Path, target: &Path) -> AppResult<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

fn directories_equal(left: &Path, right: &Path) -> AppResult<bool> {
    let mut left_entries = std::fs::read_dir(left)?
        .map(|entry| entry.map(|value| value.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut right_entries = std::fs::read_dir(right)?
        .map(|entry| entry.map(|value| value.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    left_entries.sort();
    right_entries.sort();
    if left_entries != right_entries {
        return Ok(false);
    }
    for name in left_entries {
        let left_path = left.join(&name);
        let right_path = right.join(&name);
        let left_type = std::fs::metadata(&left_path)?.file_type();
        let right_type = std::fs::metadata(&right_path)?.file_type();
        if left_type.is_dir() != right_type.is_dir() {
            return Ok(false);
        }
        if left_type.is_dir() {
            if !directories_equal(&left_path, &right_path)? {
                return Ok(false);
            }
        } else if std::fs::read(&left_path)? != std::fs::read(&right_path)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn slugify(value: &str) -> String {
    let mut slug = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    slug = slug.trim().trim_matches('.').replace(' ', "-");
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "task" } else { slug };
    slug.chars().take(60).collect()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{AppPaths, Database};

    #[test]
    fn creates_unique_workspace_for_each_task() {
        let root = std::env::temp_dir().join(format!("abya-task-service-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let database = Database::open(&paths).unwrap();
        let service = TaskService::new(database, paths.default_workspace_root.clone());
        let first = service
            .create(TaskInput {
                title: "Same title".into(),
                description: String::new(),
            })
            .unwrap();
        let second = service
            .create(TaskInput {
                title: "Same title".into(),
                description: String::new(),
            })
            .unwrap();

        assert_ne!(first.workspace_path, second.workspace_path);
        assert!(Path::new(&first.workspace_path).is_dir());
        assert!(Path::new(&second.workspace_path).is_dir());
        for llm_directory in MANAGED_LLM_DIRECTORIES {
            assert!(
                Path::new(&first.workspace_path)
                    .join(llm_directory)
                    .join("skills")
                    .join(MANAGED_SKILL_NAME)
                    .join("SKILL.md")
                    .is_file()
            );
            assert!(
                Path::new(&first.workspace_path)
                    .join(llm_directory)
                    .join("skills")
                    .join(MANAGED_SKILL_NAME)
                    .join("references")
                    .join("gameplay-architecture.md")
                    .is_file()
            );
            assert!(
                Path::new(&first.workspace_path)
                    .join(llm_directory)
                    .join("skills")
                    .join(MANAGED_SKILL_NAME)
                    .join("assets")
                    .join("gameplay-architecture.v1.template.json")
                    .is_file()
            );
        }
        assert_eq!(service.list().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refreshes_only_the_managed_skill_directory() {
        let root = std::env::temp_dir().join(format!("abya-task-skill-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let database = Database::open(&paths).unwrap();
        let service = TaskService::new(database, paths.default_workspace_root.clone());
        let task = service
            .create(TaskInput {
                title: "Managed Skill".into(),
                description: String::new(),
            })
            .unwrap();
        let workspace = Path::new(&task.workspace_path);
        for llm_directory in MANAGED_LLM_DIRECTORIES {
            let managed = workspace
                .join(llm_directory)
                .join("skills")
                .join(MANAGED_SKILL_NAME);
            let user_skill = workspace
                .join(llm_directory)
                .join("skills")
                .join("user-skill");
            std::fs::create_dir_all(&user_skill).unwrap();
            std::fs::write(user_skill.join("SKILL.md"), "user content").unwrap();
            std::fs::write(managed.join("SKILL.md"), "stale managed content").unwrap();
        }

        service.get(&task.id).unwrap();

        for llm_directory in MANAGED_LLM_DIRECTORIES {
            let managed = workspace
                .join(llm_directory)
                .join("skills")
                .join(MANAGED_SKILL_NAME);
            let user_skill = workspace
                .join(llm_directory)
                .join("skills")
                .join("user-skill");
            assert_eq!(
                std::fs::read_to_string(user_skill.join("SKILL.md")).unwrap(),
                "user content"
            );
            assert!(
                std::fs::read_to_string(managed.join("SKILL.md"))
                    .unwrap()
                    .contains("ABYA Game Development Task")
            );
            assert!(
                managed
                    .join("references")
                    .join("gameplay-architecture.md")
                    .is_file()
            );
            assert!(
                managed
                    .join("assets")
                    .join("gameplay-architecture.v1.template.json")
                    .is_file()
            );
        }
        assert!(
            std::fs::read_to_string(
                workspace
                    .join(".grok")
                    .join("skills")
                    .join(MANAGED_SKILL_NAME)
                    .join("SKILL.md")
            )
            .unwrap()
            .contains("provider: \"grok\"")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn discovers_and_refreshes_templates_without_overwriting_unmarked_skills() {
        for kind in TEMPLATE_KINDS {
            let template_marker = format!("{kind}.json");
            let root = std::env::temp_dir().join(format!("abya-templates-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            let paths = AppPaths {
                data_dir: root.clone(),
                database_path: root.join("app.db"),
                bootstrap_logs_dir: root.join("logs"),
                reports_dir: root.join("reports"),
                archive_transfers_dir: root.join("transfers"),
                default_archive_root: root.join("archives"),
                default_workspace_root: root.join("workspaces"),
            };
            let database = Database::open(&paths).unwrap();
            let mut service = TaskService::new(database, paths.default_workspace_root);
            let template_id = format!("{kind}-example");
            let id = template_id.as_str();
            let marker = serde_json::json!({
                "schemaVersion": 1, "kind": kind, "id": id,
                "displayName": "Example", "description": "Example workflow", "sourceSkillName": "example"
            });
            for source in &mut service.managed_skill_sources {
                let skills = root
                    .join("bundle")
                    .join(source.llm_directory)
                    .join("skills");
                let general = skills.join(MANAGED_SKILL_NAME);
                std::fs::create_dir_all(&general).unwrap();
                std::fs::write(general.join("SKILL.md"), "general").unwrap();
                source.source = Some(general);
                let template = skills.join(id);
                std::fs::create_dir_all(template.join("references")).unwrap();
                std::fs::write(template.join("SKILL.md"), "template").unwrap();
                std::fs::write(template.join(&template_marker), marker.to_string()).unwrap();
                std::fs::write(template.join("references/rules.md"), "rules").unwrap();
                let invalid = skills.join(format!("{kind}-invalid"));
                std::fs::create_dir_all(&invalid).unwrap();
                std::fs::write(invalid.join("SKILL.md"), "not a registered template").unwrap();
                std::fs::write(invalid.join(&template_marker), marker.to_string()).unwrap();
            }
            let task = service
                .create(TaskInput {
                    title: "Templates".into(),
                    description: String::new(),
                })
                .unwrap();
            let workspace = Path::new(&task.workspace_path);
            for provider in MANAGED_LLM_DIRECTORIES {
                let skills = workspace.join(provider).join("skills");
                assert_eq!(
                    std::fs::read_to_string(skills.join(id).join("references/rules.md")).unwrap(),
                    "rules"
                );
                assert!(!skills.join(format!("{kind}-invalid")).exists());
                std::fs::write(skills.join(id).join("references/rules.md"), "stale").unwrap();
                std::fs::write(skills.join(id).join("stale.md"), "remove on refresh").unwrap();
            }
            service.get(&task.id).unwrap();
            for provider in MANAGED_LLM_DIRECTORIES {
                let template = workspace.join(provider).join("skills").join(id);
                assert_eq!(
                    std::fs::read_to_string(template.join("references/rules.md")).unwrap(),
                    "rules"
                );
                assert!(!template.join("stale.md").exists());
            }
            let collision = workspace.join(".codex/skills").join(id);
            std::fs::remove_file(collision.join(&template_marker)).unwrap();
            std::fs::write(collision.join("SKILL.md"), "user-owned").unwrap();
            assert!(
                service
                    .get(&task.id)
                    .unwrap_err()
                    .message
                    .contains("user-owned")
            );
            assert_eq!(
                std::fs::read_to_string(collision.join("SKILL.md")).unwrap(),
                "user-owned"
            );
            // No templates remains a supported bundle: only the general workflow is required.
            for source in &service.managed_skill_sources {
                std::fs::remove_dir_all(source.source.as_ref().unwrap().parent().unwrap().join(id))
                    .unwrap();
            }
            service.get(&task.id).unwrap();
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn refuses_to_delete_a_task_while_owned_instances_are_running() {
        let root = std::env::temp_dir().join(format!("abya-task-delete-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let database = Database::open(&paths).unwrap();
        let service = TaskService::new(database.clone(), paths.default_workspace_root.clone());
        let task = service
            .create(TaskInput {
                title: "Busy".into(),
                description: String::new(),
            })
            .unwrap();
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO game_instances(id,task_id,origin,name,process_state,connection_state)
                     VALUES('instance',?1,'managed','Game','running','disconnected')",
                    [&task.id],
                )?;
                Ok(())
            })
            .unwrap();

        let error = service.delete(&task.id).unwrap_err();
        assert_eq!(error.code, "validation");
        assert!(error.message.contains("Stop all running instances"));
        assert_eq!(service.list().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn deletes_an_idle_task_record_without_removing_the_workspace() {
        let root = std::env::temp_dir().join(format!("abya-task-idle-delete-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let database = Database::open(&paths).unwrap();
        let service = TaskService::new(database, paths.default_workspace_root);
        let task = service
            .create(TaskInput {
                title: "Idle".into(),
                description: String::new(),
            })
            .unwrap();
        let workspace = PathBuf::from(&task.workspace_path);
        service.delete(&task.id).unwrap();
        assert!(service.list().unwrap().is_empty());
        assert!(workspace.is_dir());
        let _ = std::fs::remove_dir_all(root);
    }
}
