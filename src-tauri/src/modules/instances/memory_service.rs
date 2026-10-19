use super::{InstanceOrigin, InstanceService, MemoryPolicy, MemoryStatus, ProcessState};
use crate::foundation::{AppError, AppResult};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

impl InstanceService {
    pub fn memory_status(&self, id: &str) -> AppResult<MemoryStatus> {
        self.read(id)?;
        Ok(self.memory.status(id))
    }
    pub fn memory_policy(&self) -> MemoryPolicy {
        self.memory.policy()
    }
    pub fn set_memory_policy(&self, policy: MemoryPolicy) -> AppResult<()> {
        self.memory.set_policy(policy)
    }
    pub fn arm_memory_stop(&self, id: &str, enabled: bool) -> AppResult<()> {
        let instance = self.read(id)?;
        if instance.origin != InstanceOrigin::Managed
            || !matches!(instance.process_state, ProcessState::Running)
        {
            return Err(AppError::validation("仅可为当前托管实例设置自动停止。"));
        }
        self.memory.arm(id, enabled)
    }
    pub fn memory_report(&self, id: &str) -> AppResult<PathBuf> {
        self.read(id)?;
        self.memory.report(id)
    }
    pub fn memory_reports_directory(&self) -> AppResult<PathBuf> {
        let root = self.paths.reports_dir.join("MemoryIncidents");
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }
    pub fn set_memory_context_reader(&self, reader: Arc<dyn Fn(&str) -> Value + Send + Sync>) {
        *self.memory.context.lock() = Some(reader);
    }
    pub fn memory_pressured(&self, id: &str) -> bool {
        self.memory.pressured(id)
    }
    pub fn record_memory_operation(&self, id: &str, event: Value) -> bool {
        self.memory.event(id, event)
    }
}
