use super::memory_probe::MemorySample;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryPolicy {
    pub private_limit_mib: u64,
    pub growth_limit_mib: u64,
    pub available_floor_mib: u64,
    pub commit_limit_percent: u64,
}
impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            private_limit_mib: 4096,
            growth_limit_mib: 768,
            available_floor_mib: 8192,
            commit_limit_percent: 90,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatus {
    pub instance_id: String,
    pub latest: Option<MemorySample>,
    pub peak_private_bytes: u64,
    pub pressured: bool,
    pub auto_stop: bool,
    pub incident_id: Option<String>,
    pub report_path: Option<String>,
    pub error: String,
}
