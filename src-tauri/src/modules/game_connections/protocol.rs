use super::models::{GameHello, LogBatch};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ClientMessage {
    Hello(GameHello),
    Heartbeat,
    LogBatch(LogBatch),
    ArchiveTransferDecision {
        #[serde(rename = "transferId")]
        transfer_id: String,
        accepted: bool,
        #[serde(default)]
        reason: String,
    },
    ArchiveTransferAck {
        #[serde(rename = "transferId")]
        transfer_id: String,
        #[serde(rename = "receivedBytes")]
        received_bytes: u64,
    },
    ArchiveTransferProgress {
        #[serde(rename = "transferId")]
        transfer_id: String,
        #[serde(default)]
        phase: String,
        #[serde(default)]
        percent: f64,
        #[serde(default)]
        message: String,
    },
    ArchiveTransferResult {
        #[serde(rename = "transferId")]
        transfer_id: String,
        status: String,
        #[serde(rename = "installedPath", default)]
        installed_path: String,
        #[serde(rename = "errorCode", default)]
        error_code: String,
        #[serde(default)]
        message: String,
    },
    Disconnect {
        #[serde(default)]
        reason: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerMessage {
    Welcome {
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        #[serde(rename = "toolId")]
        tool_id: String,
        #[serde(rename = "connectionId")]
        connection_id: String,
        #[serde(rename = "instanceId")]
        instance_id: String,
        origin: String,
        #[serde(rename = "heartbeatIntervalMs")]
        heartbeat_interval_ms: u64,
        #[serde(rename = "disconnectTimeoutMs")]
        disconnect_timeout_ms: u64,
        #[serde(rename = "reconnectGraceMs")]
        reconnect_grace_ms: u64,
        #[serde(rename = "logSessionId")]
        log_session_id: String,
        #[serde(rename = "resumeAfterSequence")]
        resume_after_sequence: i64,
        #[serde(rename = "logsEnabled")]
        logs_enabled: bool,
    },
    LogAck {
        #[serde(rename = "batchId")]
        batch_id: String,
        #[serde(rename = "latestSequence")]
        latest_sequence: i64,
        #[serde(rename = "acceptedEvents")]
        accepted_events: usize,
    },
    LogsControl {
        enabled: bool,
    },
    ArchiveTransferOffer(super::models::ArchiveTransferOffer),
    ArchiveTransferComplete {
        #[serde(rename = "transferId")]
        transfer_id: String,
    },
    ArchiveTransferCancel {
        #[serde(rename = "transferId")]
        transfer_id: String,
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
    Disconnect {
        reason: String,
    },
}
