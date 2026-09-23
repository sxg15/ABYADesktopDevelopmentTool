use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveTransferStatus {
    Preparing,
    WaitingAcceptance,
    Transferring,
    Finalizing,
    Completed,
    Rejected,
    Cancelled,
    Failed,
    Interrupted,
}

impl ArchiveTransferStatus {
    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::WaitingAcceptance => "waitingAcceptance",
            Self::Transferring => "transferring",
            Self::Finalizing => "finalizing",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    pub(crate) fn from_db(value: &str) -> Self {
        match value {
            "preparing" => Self::Preparing,
            "waitingAcceptance" => Self::WaitingAcceptance,
            "transferring" => Self::Transferring,
            "finalizing" => Self::Finalizing,
            "completed" => Self::Completed,
            "rejected" => Self::Rejected,
            "cancelled" => Self::Cancelled,
            "interrupted" => Self::Interrupted,
            _ => Self::Failed,
        }
    }

    pub(crate) fn active(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::WaitingAcceptance | Self::Transferring | Self::Finalizing
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveTransferTarget {
    pub instance_id: String,
    pub name: String,
    pub origin: String,
    pub game_version: String,
    pub platform: String,
    pub remote_address: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveTransferRecord {
    pub id: String,
    pub instance_id: String,
    pub source_main_path: String,
    pub archive_path: String,
    pub archive_guid: String,
    pub archive_name: String,
    pub author: String,
    pub file_count: u64,
    pub uncompressed_bytes: u64,
    pub package_bytes: u64,
    pub acknowledged_bytes: u64,
    pub sha256: String,
    pub status: ArchiveTransferStatus,
    pub phase: String,
    pub progress_percent: f64,
    pub installed_path: String,
    pub error_code: String,
    pub error_message: String,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartArchiveTransferInput {
    pub instance_id: String,
    pub main_archive_path: String,
}
