use super::versions::{archive_root, digest, inventory};
use super::*;

pub(super) fn copy_verified(source: &Path, destination_parent: &Path) -> AppResult<PathBuf> {
    let source = archive_root(source)?;
    let before = inventory(std::slice::from_ref(&source))?;
    if before.len() > 20000
        || before.iter().map(|(_, _, size, _)| size).sum::<u64>() > 1024 * 1024 * 1024
    {
        return Err(AppError::validation(
            "存档快照超过 20000 文件或 1 GiB，请先检查源目录。",
        ));
    }
    let expected = digest(std::slice::from_ref(&source))?;
    let destination = destination_parent.join(
        source
            .file_name()
            .ok_or_else(|| AppError::validation("存档目录名称无效。"))?,
    );
    if destination.exists() {
        return Err(AppError::validation("快照目标已存在，未覆盖。"));
    }
    std::fs::create_dir_all(&destination)?;
    for (_, file, _, _) in &before {
        let relative = file.strip_prefix(&source).map_err(AppError::internal)?;
        let target = destination.join(relative);
        std::fs::create_dir_all(target.parent().unwrap())?;
        std::fs::copy(file, &target)?;
    }
    if before != inventory(std::slice::from_ref(&source))?
        || digest(std::slice::from_ref(&source))? != expected
        || digest(std::slice::from_ref(&destination))? != expected
    {
        return Err(AppError::validation(
            "复制期间存档变化，快照未登记，请重新保存后再试。",
        ));
    }
    Ok(destination)
}
pub(super) fn manifest(archive: &Path) -> AppResult<Value> {
    let entries = inventory(&[archive.to_owned()])?
        .into_iter()
        .map(|(_, file, bytes, _)| {
            let relative = file
                .strip_prefix(archive)
                .map_err(AppError::internal)?
                .to_string_lossy()
                .replace('\\', "/");
            let hash = files::hash_file(&file)?.0;
            let kind = match file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "lua" => "logic",
                "png" | "jpg" | "jpeg" | "webp" | "ttf" | "otf" => "visual",
                _ => "unknown",
            };
            Ok(json!({"path":relative,"sha256":hash,"bytes":bytes,"kind":kind}))
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(json!({"schemaVersion":1,"coverage":"file-hashes-only","files":entries}))
}
pub(super) fn difference(old: &Value, new: &Value, player_changed: bool) -> Value {
    let before = old["files"].as_array();
    let after = new["files"].as_array();
    let mut changed = vec![];
    if let Some(after) = after {
        for file in after {
            if before.is_none_or(|list| {
                !list
                    .iter()
                    .any(|v| v["path"] == file["path"] && v["sha256"] == file["sha256"])
            }) {
                changed.push(file.clone());
            }
        }
    }
    if let Some(before) = before {
        for file in before {
            if after.is_none_or(|list| !list.iter().any(|v| v["path"] == file["path"])) {
                changed.push(file.clone());
            }
        }
    }
    let unknown =
        player_changed || before.is_none() || changed.iter().any(|v| v["kind"] == "unknown");
    let required = if unknown {
        vec![
            "architecture",
            "lua",
            "core-loop",
            "input",
            "visual",
            "lifecycle",
            "save-reload",
            "authority",
        ]
    } else if changed.iter().any(|v| v["kind"] == "logic") {
        vec![
            "architecture",
            "lua",
            "core-loop",
            "input",
            "lifecycle",
            "save-reload",
            "authority",
        ]
    } else if !changed.is_empty() {
        vec!["visual", "input", "save-reload"]
    } else {
        vec!["save-reload"]
    };
    json!({"mode":"shadow","playerChanged":player_changed,"unknownImpact":unknown,"changedFiles":changed,
        "mustRetest":required,"note":"影响清单仅辅助安排验证；PBArc/PBConf 等未知内容不自动继承，现有验收门槛不降低。"})
}
impl TaskService {
    pub fn production_acceptance_archive(&self, task: &str, version: &str) -> AppResult<String> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if r.current_version.as_deref() != Some(version) {
            return Err(conflict());
        }
        super::versions::verify(&r.version_details)?;
        let source = Path::new(mutation::text(&r.version_details, "archiveSource")?);
        if r.version_details["frozenSnapshot"] != true {
            return Ok(source.to_string_lossy().into_owned());
        }
        let root = self.production_workspace(task)?;
        let parent = root
            .join("artifacts/acceptance-runs")
            .join(uuid::Uuid::new_v4().to_string());
        let copied = copy_verified(source, &parent)?;
        let display = copied.to_string_lossy();
        Ok(if let Some(rest) = display.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{rest}")
        } else {
            display.strip_prefix(r"\\?\").unwrap_or(&display).into()
        })
    }
}
