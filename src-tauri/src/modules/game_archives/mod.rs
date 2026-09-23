use crate::foundation::{AppError, AppPaths, AppResult};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::fs::{DirEntry, Metadata};
use std::path::{Component, Path, PathBuf};

pub const MAX_ARCHIVE_FILES: u64 = 100_000;
pub const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveOption {
    pub archive_path: String,
    pub archive_guid: String,
    pub archive_name: String,
    pub author: String,
    pub levels: Vec<LevelOption>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelOption {
    pub level_guid: String,
    pub level_name: String,
    pub is_start: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferableArchive {
    pub main_archive_path: String,
    pub archive_path: String,
    pub archive_guid: String,
    pub archive_name: String,
    pub author: String,
    pub file_count: u64,
    pub uncompressed_bytes: u64,
    pub last_modified_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ArchiveSnapshot {
    pub archive: TransferableArchive,
    pub entries: Vec<ArchiveEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct ArchiveEntry {
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub is_directory: bool,
}

#[derive(Clone)]
pub struct ArchiveCatalogService {
    paths: AppPaths,
}

impl ArchiveCatalogService {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn discover(&self, root: Option<&str>) -> AppResult<Vec<ArchiveOption>> {
        let root = root
            .filter(|value| !value.trim().is_empty())
            .map(Path::new)
            .unwrap_or(&self.paths.default_archive_root);
        discover_archives(root)
    }

    pub fn list_transferable(&self) -> AppResult<Vec<TransferableArchive>> {
        let root = &self.paths.default_archive_root;
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut archives = Vec::new();
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let main = entry.path().join("Main.PBArc");
            if main.is_file()
                && let Ok(archive) = self.inspect_main(&main)
            {
                archives.push(archive);
            }
        }
        archives.sort_by(|left, right| {
            left.archive_name
                .to_lowercase()
                .cmp(&right.archive_name.to_lowercase())
        });
        Ok(archives)
    }

    pub fn inspect_main(&self, main_path: &Path) -> AppResult<TransferableArchive> {
        Ok(self.snapshot(main_path)?.archive)
    }

    pub(crate) fn snapshot(&self, main_path: &Path) -> AppResult<ArchiveSnapshot> {
        validate_main_path(main_path)?;
        let root = main_path
            .parent()
            .ok_or_else(|| AppError::validation("Archive path is invalid."))?
            .canonicalize()?;
        let main = main_path.canonicalize()?;
        if main.parent() != Some(root.as_path()) {
            return Err(AppError::validation(
                "Main.PBArc must be directly inside the selected archive folder.",
            ));
        }
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&main)?)?;
        let archive_guid = required_string(&value, "Guid")?;
        let archive_name = required_string(&value, "Name")?;
        let author = string_field(&value, "Author");
        let mut entries = Vec::new();
        let mut file_count = 0_u64;
        let mut total_bytes = 0_u64;
        let mut last_modified = std::fs::metadata(&main)?.modified().ok();
        collect_entries(
            &root,
            &root,
            &mut entries,
            &mut file_count,
            &mut total_bytes,
            &mut last_modified,
        )?;
        if file_count == 0 {
            return Err(AppError::validation(
                "The archive folder contains no files.",
            ));
        }
        let last_modified_at = last_modified
            .map(DateTime::<Utc>::from)
            .map(|value| value.to_rfc3339())
            .unwrap_or_default();
        Ok(ArchiveSnapshot {
            archive: TransferableArchive {
                main_archive_path: main.to_string_lossy().into_owned(),
                archive_path: root.to_string_lossy().into_owned(),
                archive_guid,
                archive_name,
                author,
                file_count,
                uncompressed_bytes: total_bytes,
                last_modified_at,
            },
            entries,
        })
    }
}

pub(crate) fn discover_archives(root: &Path) -> AppResult<Vec<ArchiveOption>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_main_archives(root, &mut files)?;
    let mut archives = files
        .into_iter()
        .filter_map(|path| parse_archive(&path).ok())
        .collect::<Vec<_>>();
    archives.sort_by(|left, right| {
        left.archive_name
            .to_lowercase()
            .cmp(&right.archive_name.to_lowercase())
    });
    Ok(archives)
}

fn validate_main_path(path: &Path) -> AppResult<()> {
    if !path.is_file() {
        return Err(AppError::validation("Select an existing Main.PBArc file."));
    }
    let is_main = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("Main.PBArc"));
    if !is_main {
        return Err(AppError::validation(
            "The selected file must be named Main.PBArc.",
        ));
    }
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(AppError::validation(
            "Symbolic links cannot be used as archive sources.",
        ));
    }
    Ok(())
}

fn collect_entries(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<ArchiveEntry>,
    file_count: &mut u64,
    total_bytes: &mut u64,
    last_modified: &mut Option<std::time::SystemTime>,
) -> AppResult<()> {
    for item in std::fs::read_dir(directory)? {
        let item = item?;
        let metadata = item.path().symlink_metadata()?;
        reject_link(&item, &metadata)?;
        let canonical = item.path().canonicalize()?;
        if !canonical.starts_with(root) {
            return Err(AppError::validation(
                "Archive content escapes the selected archive folder.",
            ));
        }
        let relative = canonical.strip_prefix(root).map_err(AppError::internal)?;
        let relative_path = normalize_relative_path(relative)?;
        if metadata.is_dir() {
            entries.push(ArchiveEntry {
                absolute_path: canonical.clone(),
                relative_path,
                is_directory: true,
            });
            collect_entries(
                root,
                &canonical,
                entries,
                file_count,
                total_bytes,
                last_modified,
            )?;
        } else if metadata.is_file() {
            *file_count = file_count.saturating_add(1);
            *total_bytes = total_bytes.saturating_add(metadata.len());
            if *file_count > MAX_ARCHIVE_FILES {
                return Err(AppError::validation(format!(
                    "Archive contains more than {MAX_ARCHIVE_FILES} files."
                )));
            }
            if *total_bytes > MAX_ARCHIVE_BYTES {
                return Err(AppError::validation(
                    "Archive is larger than the 8 GiB transfer limit.",
                ));
            }
            if let Ok(modified) = metadata.modified()
                && last_modified.is_none_or(|value| modified > value)
            {
                *last_modified = Some(modified);
            }
            entries.push(ArchiveEntry {
                absolute_path: canonical,
                relative_path,
                is_directory: false,
            });
        }
    }
    Ok(())
}

fn reject_link(entry: &DirEntry, metadata: &Metadata) -> AppResult<()> {
    if metadata.file_type().is_symlink() {
        return Err(AppError::validation(format!(
            "Archive contains a symbolic link: {}",
            entry.path().display()
        )));
    }
    Ok(())
}

fn normalize_relative_path(path: &Path) -> AppResult<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy().into_owned()),
            _ => {
                return Err(AppError::validation(
                    "Archive contains an unsafe relative path.",
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(AppError::validation("Archive entry path is empty."));
    }
    Ok(parts.join("/"))
}

fn collect_main_archives(directory: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_symlink() {
            continue;
        }
        if path.is_dir() {
            collect_main_archives(&path, files)?;
        } else if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("Main.PBArc")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn parse_archive(path: &Path) -> AppResult<ArchiveOption> {
    let root: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let archive_path = path
        .parent()
        .ok_or_else(|| AppError::validation("Archive path is invalid."))?;
    let levels = root
        .get("Levels")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let level_guid = string_field(item, "LevelGuid");
                    if level_guid.is_empty() {
                        return None;
                    }
                    Some(LevelOption {
                        level_guid,
                        level_name: string_field(item, "LevelName"),
                        is_start: item
                            .get("IsStart")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ArchiveOption {
        archive_path: archive_path.to_string_lossy().into_owned(),
        archive_guid: string_field(&root, "Guid"),
        archive_name: string_field(&root, "Name"),
        author: string_field(&root, "Author"),
        levels,
    })
}

fn required_string(value: &Value, name: &str) -> AppResult<String> {
    let result = string_field(value, name);
    if result.trim().is_empty() {
        Err(AppError::validation(format!(
            "Main.PBArc is missing a non-empty {name}."
        )))
    } else {
        Ok(result)
    }
}

fn string_field(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn paths(root: &Path) -> AppPaths {
        AppPaths {
            data_dir: root.join("data"),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        }
    }

    #[test]
    fn inspects_complete_archive_folder() {
        let root = std::env::temp_dir().join(format!("abya-catalog-{}", Uuid::new_v4()));
        let archive = root.join("archives").join("Demo");
        std::fs::create_dir_all(archive.join("Resources")).unwrap();
        std::fs::write(
            archive.join("Main.PBArc"),
            r#"{"Name":"Demo","Author":"QA","Guid":"archive-1","Levels":[]}"#,
        )
        .unwrap();
        std::fs::write(archive.join("Resources").join("asset.bin"), [1_u8, 2, 3]).unwrap();
        let result = ArchiveCatalogService::new(paths(&root))
            .inspect_main(&archive.join("Main.PBArc"))
            .unwrap();
        assert_eq!(result.archive_guid, "archive-1");
        assert_eq!(result.file_count, 2);
        assert!(result.uncompressed_bytes > 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_non_main_file() {
        let root = std::env::temp_dir().join(format!("abya-catalog-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("Other.PBArc");
        std::fs::write(&path, "{}").unwrap();
        let error = ArchiveCatalogService::new(paths(&root))
            .inspect_main(&path)
            .unwrap_err();
        assert_eq!(error.code, "validation");
        let _ = std::fs::remove_dir_all(root);
    }
}
