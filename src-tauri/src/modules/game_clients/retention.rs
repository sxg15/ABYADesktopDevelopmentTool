use super::{GameClientService, INSTALL_PREFIX, github::error, install, models::*};
use crate::foundation::{AppResult, AppSettings, GameClientSelection, GameClientSource};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .to_lowercase()
}

fn contains(directory: &Path, value: &str) -> bool {
    let directory = normalized(directory);
    let path = Path::new(value)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(value));
    normalized(&path).starts_with(&format!("{directory}/"))
}

/// Only a recorded UUID directory at the installer's exact path is eligible.
fn owned_directory(record: &ClientInstallation) -> AppResult<PathBuf> {
    if uuid::Uuid::parse_str(&record.id).is_err() {
        return Err(error("clientUnsafePath"));
    }
    let directory = Path::new(&record.directory);
    let root = Path::new(&record.storage_root);
    if !root.is_absolute()
        || !directory.is_absolute()
        || root
            .components()
            .chain(directory.components())
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(error("clientUnsafePath"));
    }
    install::no_links(root)?;
    install::no_links(directory)?;
    let expected = root.join("PlatverseOutput").join(format!(
        "{}-{}-{}",
        record.release_id, record.asset_id, record.id
    ));
    if normalized(directory) != normalized(&expected) {
        return Err(error("clientUnsafePath"));
    }
    if !directory.exists() {
        return Ok(expected);
    }
    let directory = directory.canonicalize()?;
    let parent = root.join("PlatverseOutput").canonicalize()?;
    if directory.parent() != Some(parent.as_path()) {
        return Err(error("clientUnsafePath"));
    }
    let marker = directory.join(".abya-client.json");
    install::no_links(&marker)?;
    let file = fs::File::open(marker)?;
    if file.metadata()?.len() > 1024 * 1024 {
        return Err(error("clientUnsafePath"));
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024).read_to_end(&mut bytes)?;
    let manifest: ClientInstallation = serde_json::from_slice(&bytes)?;
    if manifest.id != record.id
        || manifest.sha256 != record.sha256
        || manifest.asset_id != record.asset_id
        || manifest.release_id != record.release_id
        || normalized(Path::new(&manifest.directory)) != normalized(Path::new(&record.directory))
    {
        return Err(error("clientUnsafePath"));
    }
    // Preflight the complete tree before any removal, including user-added junctions.
    let mut pending = vec![directory.clone()];
    let mut count = 0;
    while let Some(path) = pending.pop() {
        install::no_links(&path)?;
        count += 1;
        if count > 100_002 {
            return Err(error("clientUnsafePath"));
        }
        if fs::symlink_metadata(&path)?.is_dir() {
            for entry in fs::read_dir(path)? {
                pending.push(entry?.path());
            }
        }
    }
    Ok(directory)
}

impl GameClientService {
    pub fn activate(&self, id: &str, executable: &str) -> AppResult<AppSettings> {
        self.run("select", || {
            let record = self
                .installations()?
                .into_iter()
                .find(|r| r.id == id)
                .ok_or_else(|| error("clientInstallationMissing"))?;
            let selection = GameClientSelection {
                source: GameClientSource::Remote,
                local_executable_path: String::new(),
                installation_id: record.id.clone(),
                executable_relative_path: executable.into(),
                storage_root: record.storage_root.clone(),
            };
            let executable_path = install::resolve(&record, &selection)?;
            let settings = self
                .settings
                .update_game_client(selection, executable_path)?;
            *self.pending_installation.lock() = None;
            Ok(settings)
        })
    }

    pub fn discard_pending(&self, id: &str) -> AppResult<()> {
        self.run("select", || {
            let mut pending = self.pending_installation.lock();
            if pending.as_deref() == Some(id) {
                *pending = None;
            }
            Ok(())
        })
    }

    /// Caller holds EXECUTABLE_FILES exclusively while collecting live usage and cleaning.
    pub fn maintain(&self, pinned: &[String], running: &[String]) -> AppResult<ClientRetention> {
        self.run("cleanup", || {
            let settings = self.settings.get()?;
            let pending = self.pending_installation.lock().clone();
            let mut report = ClientRetention::default();
            for record in self.installations()? {
                if record.id == settings.game_client.installation_id
                    || pending.as_deref() == Some(&record.id)
                {
                    continue;
                }
                let path = Path::new(&record.directory);
                let reason = if pinned.iter().any(|p| contains(path, p)) {
                    Some("task")
                } else if running.iter().any(|p| contains(path, p)) {
                    Some("running")
                } else if contains(path, &settings.game_client.local_executable_path) {
                    Some("local")
                } else {
                    None
                };
                let result = if reason.is_some() {
                    Ok(false)
                } else {
                    owned_directory(&record).and_then(|directory| {
                        if directory.exists() {
                            fs::remove_dir_all(&directory)?;
                        }
                        self.database.with_connection(|db| {
                            db.execute(
                                "DELETE FROM settings WHERE key=?1",
                                [format!("{INSTALL_PREFIX}{}", record.id)],
                            )?;
                            Ok(())
                        })?;
                        Ok(true)
                    })
                };
                if matches!(result, Ok(true)) {
                    report.removed += 1;
                } else {
                    report.deferred.push(DeferredClient {
                        installation_id: record.id,
                        tag_name: record.tag_name,
                        reason: reason.unwrap_or("unavailable").into(),
                    });
                }
            }
            *self.retention.lock() = report.clone();
            Ok(report)
        })
    }
}
