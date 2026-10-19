mod launch;
mod memory;
mod memory_models;
mod memory_probe;
mod memory_report;
mod memory_service;
#[cfg(test)]
mod memory_tests;
mod models;
mod recording;
mod service;
mod window;
pub use memory_models::{MemoryPolicy, MemoryStatus};
pub(crate) use memory_report::safe_text as diagnostic_text;
pub use recording::recorder_executable;

pub use models::{
    ArchiveSelection, ConnectionState, GameInstance, InstanceOrigin, InstanceRuntimeInfo,
    InstanceStopResult, LaunchInstanceInput, LaunchMode, LaunchProfile, LaunchReportResult,
    ProcessState, WindowMode, WindowVisibilityMode,
};
pub use service::{InstanceLogRepairReport, InstanceService};

pub(crate) use launch::{build_launch_contract, validate_executable};
