mod launch;
mod models;
mod service;
mod window;

pub use models::{
    ArchiveSelection, ConnectionState, GameInstance, InstanceOrigin, InstanceRuntimeInfo,
    InstanceStopResult, LaunchInstanceInput, LaunchMode, LaunchProfile, LaunchReportResult,
    ProcessState, WindowMode, WindowVisibilityMode,
};
pub use service::{InstanceLogRepairReport, InstanceService};

pub(crate) use launch::{build_launch_contract, validate_executable};
pub(crate) use models::RuntimeMcpEndpoint;
