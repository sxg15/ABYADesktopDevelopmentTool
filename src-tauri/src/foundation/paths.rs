use super::{AppError, AppResult};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
    pub bootstrap_logs_dir: PathBuf,
    pub reports_dir: PathBuf,
    pub archive_transfers_dir: PathBuf,
    pub default_archive_root: PathBuf,
    pub default_workspace_root: PathBuf,
}

impl AppPaths {
    pub fn legacy_workspace_root() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("ABYA Desktop Development Tool/Workspaces"))
    }
    pub fn discover() -> AppResult<Self> {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| AppError::new("pathUnavailable", "LOCALAPPDATA is unavailable.", ""))?;
        let user = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| AppError::new("pathUnavailable", "USERPROFILE is unavailable.", ""))?;
        let data_dir = local.join("ABYA Desktop Development Tool").join("Data");
        let paths = Self {
            database_path: data_dir.join("app.db"),
            bootstrap_logs_dir: data_dir.join("InstanceLogs"),
            reports_dir: data_dir.join("Reports"),
            archive_transfers_dir: data_dir.join("ArchiveTransfers"),
            default_archive_root: user
                .join("AppData")
                .join("LocalLow")
                .join("ABYA ProductionTeam")
                .join("ABYA_PB")
                .join("Data")
                .join("Archives"),
            default_workspace_root: user.join("ABYA Desktop Development ToolWorkspaces"),
            data_dir,
        };
        paths.ensure()?;
        Ok(paths)
    }

    fn ensure(&self) -> AppResult<()> {
        for path in [
            self.data_dir.as_path(),
            self.bootstrap_logs_dir.as_path(),
            self.reports_dir.as_path(),
            self.archive_transfers_dir.as_path(),
            self.default_workspace_root.as_path(),
        ] {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }

    pub fn instance_log_path(&self, instance_id: &str) -> PathBuf {
        self.bootstrap_logs_dir.join(format!("{instance_id}.log"))
    }

    pub fn instance_report_path(&self, instance_id: &str) -> PathBuf {
        self.reports_dir.join(format!("{instance_id}.json"))
    }

    pub fn display(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }
}
