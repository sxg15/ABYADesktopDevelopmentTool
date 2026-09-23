use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchMode {
    #[serde(rename = "editor")]
    Editor,
    #[serde(rename = "offline", alias = "normal")]
    Offline,
    #[serde(rename = "lan-host", alias = "host")]
    LanHost,
    #[serde(rename = "lan-client", alias = "clientOnly")]
    LanClient,
    #[serde(rename = "igp-hosted")]
    IgpHosted,
}

impl LaunchMode {
    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Offline => "offline",
            Self::LanHost => "lan-host",
            Self::LanClient => "lan-client",
            Self::IgpHosted => "igp-hosted",
        }
    }

    pub(crate) fn from_db(value: &str) -> Self {
        match value {
            "editor" => Self::Editor,
            "lan-host" | "host" => Self::LanHost,
            "lan-client" | "clientOnly" => Self::LanClient,
            "igp-hosted" => Self::IgpHosted,
            _ => Self::Offline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstanceOrigin {
    Managed,
    External,
}

impl InstanceOrigin {
    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::External => "external",
        }
    }

    pub(crate) fn from_db(value: &str) -> Self {
        if value == "external" {
            Self::External
        } else {
            Self::Managed
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowMode {
    Windowed,
    Borderless,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowVisibilityMode {
    Background,
    #[default]
    Visible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSelection {
    pub archive_path: String,
    pub archive_guid: String,
    pub archive_name: String,
    pub level_guid: String,
    pub level_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProfile {
    pub mode: LaunchMode,
    pub window_mode: WindowMode,
    #[serde(default)]
    pub visibility_mode: WindowVisibilityMode,
    pub width: u16,
    pub height: u16,
    pub archive: Option<ArchiveSelection>,
    pub host_instance_id: Option<String>,
    #[serde(default)]
    pub exit_on_failure: bool,
    #[serde(default)]
    pub language: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchInstanceInput {
    pub task_id: String,
    pub name: String,
    pub executable_path: String,
    pub profile: LaunchProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProcessState {
    Launching,
    Running,
    Stopping,
    Exited,
    Failed,
    Interrupted,
    Unmanaged,
}

impl ProcessState {
    pub(crate) fn from_db(value: &str) -> Self {
        match value {
            "launching" => Self::Launching,
            "running" => Self::Running,
            "stopping" => Self::Stopping,
            "exited" => Self::Exited,
            "failed" => Self::Failed,
            "unmanaged" => Self::Unmanaged,
            _ => Self::Interrupted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    Waiting,
    Connected,
    Disconnected,
}

impl ConnectionState {
    pub(crate) fn from_db(value: &str) -> Self {
        match value {
            "waiting" => Self::Waiting,
            "connected" => Self::Connected,
            _ => Self::Disconnected,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstance {
    pub id: String,
    pub task_id: Option<String>,
    pub origin: InstanceOrigin,
    pub name: String,
    pub mode: Option<LaunchMode>,
    pub host_instance_id: Option<String>,
    pub executable_path: Option<String>,
    pub profile: Option<LaunchProfile>,
    pub sanitized_args: Vec<String>,
    pub pid: Option<u32>,
    pub process_state: ProcessState,
    pub connection_state: ConnectionState,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub failure_reason: String,
    pub host_port: Option<u16>,
    pub log_file_path: Option<String>,
    pub runtime_instance_id: Option<String>,
    pub remote_address: Option<String>,
    pub game_version: String,
    pub platform: String,
    pub runtime_capabilities: Vec<String>,
    pub connected_at: Option<String>,
    pub disconnected_at: Option<String>,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRuntimeInfo {
    pub instance: GameInstance,
    pub gateway_endpoint: String,
    pub report_path: Option<String>,
    pub process_alive: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeMcpEndpoint {
    pub endpoint: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStopResult {
    pub instance: GameInstance,
    pub graceful_exit_requested: bool,
    pub graceful_exit_observed: bool,
    pub forced_termination: bool,
    pub timed_out: bool,
    pub process_alive: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReportResult {
    pub instance_id: String,
    pub path: String,
    pub exists: bool,
    pub report: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_launch_modes_deserialize_to_canonical_modes() {
        assert_eq!(
            serde_json::from_str::<LaunchMode>("\"normal\"").unwrap(),
            LaunchMode::Offline
        );
        assert_eq!(
            serde_json::from_str::<LaunchMode>("\"host\"").unwrap(),
            LaunchMode::LanHost
        );
        assert_eq!(
            serde_json::from_str::<LaunchMode>("\"clientOnly\"").unwrap(),
            LaunchMode::LanClient
        );
        assert_eq!(
            serde_json::to_string(&LaunchMode::LanClient).unwrap(),
            "\"lan-client\""
        );
    }

    #[test]
    fn legacy_launch_profile_defaults_to_visible() {
        let profile = serde_json::from_value::<LaunchProfile>(serde_json::json!({
            "mode": "offline",
            "windowMode": "windowed",
            "width": 1280,
            "height": 720,
            "archive": null,
            "hostInstanceId": null,
            "exitOnFailure": true,
            "language": ""
        }))
        .unwrap();

        assert_eq!(profile.visibility_mode, WindowVisibilityMode::Visible);
    }
}
