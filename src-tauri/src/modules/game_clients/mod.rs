mod auth;
mod github;
mod install;
mod models;
mod oauth;
#[cfg(test)]
mod oauth_tests;
#[cfg(test)]
mod tests;

use crate::foundation::{
    AppError, AppResult, Database, GameClientSource, PublicSettingsUpdate, SettingsService,
};
use github::{GitHub, error};
pub use models::*;
use parking_lot::Mutex;
use rusqlite::OptionalExtension;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const INSTALL_PREFIX: &str = "game-clients/installation/";

#[derive(Clone)]
pub struct GameClientService {
    database: Database,
    settings: SettingsService,
    default_storage: PathBuf,
    auth: Arc<Mutex<auth::AuthState>>,
    refresh_gate: Arc<Mutex<()>>,
    #[cfg(test)]
    oauth_base: Option<String>,
    #[cfg(test)]
    github_base: Option<String>,
    progress: Progress,
    busy: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
struct Progress {
    state: Arc<Mutex<ClientOperation>>,
    cancelled: Arc<AtomicBool>,
}

impl Progress {
    fn stage(&self, name: &str, total: u64) {
        let mut state = self.state.lock();
        state.stage = name.into();
        state.completed_bytes = 0;
        state.total_bytes = total;
    }
    fn advance(&self, count: u64) {
        self.state.lock().completed_bytes += count;
    }
    fn check(&self) -> AppResult<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(error("clientCancelled"))
        } else {
            Ok(())
        }
    }
}

struct BusyGuard {
    busy: Arc<AtomicBool>,
    progress: Progress,
}
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.progress.state.lock().active = false;
        self.busy.store(false, Ordering::Release);
    }
}

impl GameClientService {
    pub fn new(database: Database, settings: SettingsService, data_dir: PathBuf) -> Self {
        Self {
            database,
            settings,
            default_storage: data_dir.join("GameClients"),
            auth: Arc::new(Mutex::new(auth::AuthState::default())),
            refresh_gate: Arc::new(Mutex::new(())),
            #[cfg(test)]
            oauth_base: None,
            #[cfg(test)]
            github_base: None,
            progress: Progress::default(),
            busy: Arc::new(AtomicBool::new(false)),
        }
    }

    fn run<T>(&self, kind: &str, action: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(error("clientBusy"));
        }
        let _guard = BusyGuard {
            busy: self.busy.clone(),
            progress: self.progress.clone(),
        };
        self.progress.cancelled.store(false, Ordering::Relaxed);
        *self.progress.state.lock() = ClientOperation {
            active: true,
            kind: kind.into(),
            stage: "connecting".into(),
            ..Default::default()
        };
        let result = action();
        let result = if result.is_err() && self.progress.cancelled.load(Ordering::Relaxed) {
            Err(error("clientCancelled"))
        } else {
            result
        };
        let mut state = self.progress.state.lock();
        state.stage = if result.is_ok() { "complete" } else { "failed" }.into();
        state.error_code = result.as_ref().err().map(|e| e.code.clone());
        result
    }

    pub fn operation(&self) -> ClientOperation {
        self.progress.state.lock().clone()
    }
    pub fn cancel(&self) {
        let operation = self.operation();
        if operation.active && matches!(operation.kind.as_str(), "install" | "upload") {
            self.progress.cancelled.store(true, Ordering::Relaxed);
        }
    }

    fn read<T: DeserializeOwned>(&self, key: &str) -> AppResult<Option<T>> {
        self.database.with_connection(|db| {
            let json = db
                .query_row(
                    "SELECT value_json FROM settings WHERE key=?1",
                    [key],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            json.map(|value| serde_json::from_str(&value).map_err(AppError::from))
                .transpose()
        })
    }

    fn write(&self, key: &str, value: &impl Serialize) -> AppResult<()> {
        let json = serde_json::to_string(value)?;
        self.database.with_connection(|db| {
            db.execute("INSERT INTO settings(key,value_json,updated_at) VALUES(?1,?2,?3) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",
                rusqlite::params![key,json,chrono::Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }

    fn github(&self, required: bool) -> AppResult<GitHub> {
        let token = self.token()?;
        if required && token.is_none() {
            return Err(error("clientLoginRequired"));
        }
        GitHub::new(token)
    }

    pub fn state(&self) -> AppResult<GameClientState> {
        Ok(GameClientState {
            repository: REPOSITORY.into(),
            account: self.account()?,
            installations: self.installations()?,
            default_storage_root: self.default_storage.to_string_lossy().into_owned(),
            operation: self.operation(),
            authorization: self.authorization(),
        })
    }

    pub fn releases(&self) -> AppResult<Vec<ClientRelease>> {
        let mut releases = self.github(false)?.releases()?;
        releases.sort_by(|a, b| b.published_at.cmp(&a.published_at).then(b.id.cmp(&a.id)));
        Ok(releases)
    }
    pub fn release(&self, id: u64) -> AppResult<ClientRelease> {
        self.github(false)?.release(id)
    }
    pub fn save_release(&self, input: ReleaseInput) -> AppResult<ClientRelease> {
        self.run("release", || self.github(true)?.save_release(input))
    }
    pub fn delete_release(&self, id: u64) -> AppResult<()> {
        self.run("release", || self.github(true)?.delete_release(id))
    }
    pub fn delete_asset(&self, release_id: u64, asset_id: u64) -> AppResult<()> {
        self.run("release", || {
            self.github(true)?.delete_asset(release_id, asset_id)
        })
    }

    pub fn upload(&self, release_id: u64, path: String) -> AppResult<ReleaseAsset> {
        self.run("upload", || {
            let path = PathBuf::from(path);
            let file = File::open(&path)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_ASSET_BYTES {
                return Err(error("clientUploadSize"));
            }
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| error("clientUnsafePath"))?;
            self.progress.stage("uploading", metadata.len());
            let body = reqwest::blocking::Body::sized(
                UploadReader {
                    file,
                    progress: self.progress.clone(),
                },
                metadata.len(),
            );
            self.github(true)?.upload(release_id, name, body)
        })
    }

    pub fn install(&self, input: InstallInput) -> AppResult<ClientInstallation> {
        self.run("install", || {
            let github = self.github(false)?;
            let release = github.release(input.release_id)?;
            let asset = release
                .assets
                .iter()
                .find(|asset| asset.id == input.asset_id)
                .ok_or_else(|| error("clientAssetMissing"))?;
            if !asset.installable() || asset.size > MAX_ASSET_BYTES {
                return Err(error("clientPackageFormat"));
            }
            let response = github.download(asset.id)?;
            install::install(
                response,
                &release,
                asset,
                &input.storage_root,
                &self.progress,
                |record| self.write(&format!("{INSTALL_PREFIX}{}", record.id), record),
            )
        })
    }

    fn installations(&self) -> AppResult<Vec<ClientInstallation>> {
        self.database.with_connection(|db| {
            let mut statement = db.prepare(
                "SELECT value_json FROM settings WHERE key LIKE ?1 ORDER BY updated_at DESC",
            )?;
            let rows = statement.query_map([format!("{INSTALL_PREFIX}%")], |row| {
                row.get::<_, String>(0)
            })?;
            rows.map(|row| serde_json::from_str(&row?).map_err(AppError::from))
                .collect()
        })
    }

    pub fn resolve_settings(&self, input: &mut PublicSettingsUpdate) -> AppResult<()> {
        if input.game_client.source == GameClientSource::Local {
            if input.game_client.local_executable_path.is_empty() {
                input.game_client.local_executable_path = input.game_executable_path.trim().into();
            }
            input.game_executable_path = input.game_client.local_executable_path.trim().into();
        } else {
            let record = self
                .installations()?
                .into_iter()
                .find(|record| record.id == input.game_client.installation_id)
                .ok_or_else(|| error("clientInstallationMissing"))?;
            input.game_executable_path = install::resolve(&record, &input.game_client)?;
        }
        Ok(())
    }

    pub fn check_update(&self) -> AppResult<Option<ClientUpdate>> {
        let settings = self.settings.get()?;
        if settings.game_client.source != GameClientSource::Remote || self.token()?.is_none() {
            return Ok(None);
        }
        let Some(current) = self
            .installations()?
            .into_iter()
            .find(|r| r.id == settings.game_client.installation_id)
        else {
            return Ok(None);
        };
        let github = self.github(false)?;
        let releases = github.releases()?;
        let Some(candidate) = update_candidate(&current, &releases) else {
            return Ok(None);
        };
        Ok(Some(ClientUpdate {
            release_id: candidate.id,
            tag_name: candidate.tag_name.clone(),
            current_tag: current.tag_name,
        }))
    }
}

fn update_candidate<'a>(
    current: &ClientInstallation,
    releases: &'a [ClientRelease],
) -> Option<&'a ClientRelease> {
    releases
        .iter()
        .filter(|release| {
            if release.draft
                || (!current.prerelease && release.prerelease)
                || release.published_at.is_none()
            {
                return false;
            }
            if !release.assets.iter().any(ReleaseAsset::installable) {
                return false;
            }
            if release.id == current.release_id {
                release.assets.iter().any(|asset| {
                    asset.installable()
                        && asset.name == current.asset_name
                        && (asset.id != current.asset_id
                            || asset.updated_at > current.asset_updated_at)
                })
            } else {
                release.published_at > current.published_at
            }
        })
        .max_by(|a, b| a.published_at.cmp(&b.published_at).then(a.id.cmp(&b.id)))
}

struct UploadReader {
    file: File,
    progress: Progress,
}
impl Read for UploadReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.progress
            .check()
            .map_err(|_| std::io::Error::other("Upload cancelled"))?;
        let n = self.file.read(buffer)?;
        self.progress.advance(n as u64);
        Ok(n)
    }
}
