use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u16 = 1;
pub const GATEWAY_PATH: &str = "/game/v1/connect";
pub const DISCOVERY_PORT: u16 = 47611;
pub const HEARTBEAT_INTERVAL_MS: u64 = 5_000;
pub const DISCONNECT_TIMEOUT_MS: u64 = 15_000;
pub const RECONNECT_GRACE_MS: u64 = 30_000;
pub const HELLO_TIMEOUT_MS: u64 = 8_000;
pub const MAX_MESSAGE_BYTES: usize = 512 * 1024;
pub const MAX_LOG_BATCH_EVENTS: usize = 100;
pub const MAX_LOG_BATCH_BYTES: usize = 256 * 1024;
pub const ARCHIVE_TRANSFER_CAPABILITY: &str = "archive-transfer-v1";
pub const ARCHIVE_TRANSFER_CHUNK_BYTES: usize = 256 * 1024;
pub const ARCHIVE_TRANSFER_WINDOW_CHUNKS: usize = 4;
pub const ARCHIVE_TRANSFER_OFFER_TIMEOUT_MS: u64 = 10 * 60 * 1000;
pub const ARCHIVE_TRANSFER_DATA_TIMEOUT_MS: u64 = 30 * 1000;
pub const ARCHIVE_TRANSFER_FINALIZE_TIMEOUT_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanInterface {
    pub id: String,
    pub name: String,
    pub address: String,
    pub broadcast_address: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameConnectionState {
    pub running: bool,
    pub broadcast_running: bool,
    pub bind_endpoint: String,
    pub preferred_endpoint: String,
    pub gateway_port: u16,
    pub discovery_port: u16,
    pub path: String,
    pub protocol_version: u16,
    pub preferred_adapter_id: String,
    pub advertised_addresses: Vec<String>,
    pub connected_count: usize,
    pub tool_id: String,
    pub last_error: String,
}

impl Default for GameConnectionState {
    fn default() -> Self {
        Self {
            running: false,
            broadcast_running: false,
            bind_endpoint: String::new(),
            preferred_endpoint: String::new(),
            gateway_port: 0,
            discovery_port: DISCOVERY_PORT,
            path: GATEWAY_PATH.to_string(),
            protocol_version: PROTOCOL_VERSION,
            preferred_adapter_id: String::new(),
            advertised_addresses: Vec::new(),
            connected_count: 0,
            tool_id: String::new(),
            last_error: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameHello {
    pub protocol_version: u16,
    pub runtime_instance_id: String,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub game_version: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub log_session_id: String,
}

#[derive(Debug, Clone)]
pub struct ConnectionRegistration {
    pub instance_id: String,
    pub origin: String,
    pub log_session_id: String,
    pub resume_after_sequence: i64,
    pub logs_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogBatch {
    pub batch_id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub dropped_events: i64,
}

#[derive(Debug, Clone)]
pub struct LogBatchAck {
    pub latest_sequence: i64,
    pub accepted_events: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveTransferOffer {
    pub transfer_id: String,
    pub source_display_name: String,
    pub archive_guid: String,
    pub archive_name: String,
    pub author: String,
    pub file_count: u64,
    pub uncompressed_bytes: u64,
    pub package_bytes: u64,
    pub sha256: String,
    pub format: String,
    pub conflict_policy: String,
    pub chunk_size: usize,
}

#[derive(Debug, Clone)]
pub enum ArchiveTransferEvent {
    Decision {
        accepted: bool,
        reason: String,
    },
    Ack {
        received_bytes: u64,
    },
    Progress {
        phase: String,
        percent: f64,
        message: String,
    },
    Result {
        status: String,
        installed_path: String,
        error_code: String,
        message: String,
    },
    Disconnected {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SessionSnapshot {
    pub instance_id: String,
    pub connection_id: String,
    pub remote_address: String,
    pub origin: String,
    pub game_version: String,
    pub platform: String,
    pub capabilities: Vec<String>,
}
