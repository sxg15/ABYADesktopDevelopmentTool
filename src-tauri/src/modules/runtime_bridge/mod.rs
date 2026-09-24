mod process;
use crate::foundation::{AppError, AppResult};
use crate::modules::{instances::InstanceService, tasks::TaskService};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBridgeState {
    pub instance_id: String,
    pub endpoint: String,
    pub connected: bool,
    pub runtime_instance_id: String,
    pub origin: String,
    pub game_version: String,
    pub platform: String,
    pub capabilities: Vec<String>,
    pub cli_available: bool,
    pub server_name: String,
    pub server_version: String,
    pub instructions: String,
    pub last_error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

struct ProcessSlot(Arc<AtomicUsize>);
impl Drop for ProcessSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

type ActiveOperations = HashMap<String, (String, String, Arc<AtomicBool>)>;

#[derive(Clone)]
pub struct RuntimeBridgeService {
    instances: InstanceService,
    tasks: TaskService,
    active: Arc<Mutex<ActiveOperations>>,
    in_flight: Arc<AtomicUsize>,
}
impl RuntimeBridgeService {
    pub fn new(instances: InstanceService, tasks: TaskService) -> Self {
        Self {
            instances,
            tasks,
            active: Default::default(),
            in_flight: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn run(
        &self,
        id: &str,
        args: &[&str],
        input: Option<Value>,
        session: &str,
        operation: &str,
        timeout: Duration,
        cancel: Arc<AtomicBool>,
    ) -> AppResult<Value> {
        if self.in_flight.fetch_add(1, Ordering::SeqCst) >= 8 {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            return Err(AppError::new(
                "runtimeCliBusy",
                "At most eight CLI operations may run concurrently.",
                "not_executed",
            ));
        }
        let _slot = ProcessSlot(self.in_flight.clone());
        let pid = self.instances.runtime_pid(id)?;
        let instance = self.instances.read(id)?;
        let task = self.tasks.get(
            instance
                .task_id
                .as_deref()
                .ok_or_else(|| AppError::validation("No task owner."))?,
        )?;
        let output = PathBuf::from(task.workspace_path)
            .join("artifacts/runtime")
            .join(id)
            .join(operation);
        process::execute(
            pid, id, args, input, session, operation, &output, timeout, cancel,
        )
    }
    pub fn read_artifact(&self, task_id: &str, file: &str) -> AppResult<String> {
        use base64::Engine as _;
        let task = self.tasks.get(task_id)?;
        let root = PathBuf::from(task.workspace_path)
            .join("artifacts/runtime")
            .canonicalize()?;
        let file = PathBuf::from(file).canonicalize()?;
        if !file.starts_with(&root) {
            return Err(AppError::validation("Artifact is outside this task."));
        }
        let mime = match file.extension().and_then(|v| v.to_str()) {
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            _ => {
                return Err(AppError::validation(
                    "Only runtime image artifacts can be previewed.",
                ));
            }
        };
        if file.metadata()?.len() > 16 * 1024 * 1024 {
            return Err(AppError::validation("Image exceeds 16 MiB."));
        }
        Ok(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(std::fs::read(file)?)
        ))
    }
    pub fn state(&self, id: &str) -> RuntimeBridgeState {
        let instance = self.instances.read(id).ok();
        let result = self.run(
            id,
            &["status"],
            None,
            "desktop",
            &uuid::Uuid::new_v4().to_string(),
            Duration::from_secs(5),
            Default::default(),
        );
        RuntimeBridgeState {
            instance_id: id.into(),
            endpoint: result
                .as_ref()
                .ok()
                .and_then(|v| v["endpoint"].as_str())
                .unwrap_or("")
                .into(),
            connected: result.is_ok(),
            cli_available: result.is_ok(),
            runtime_instance_id: instance
                .as_ref()
                .and_then(|v| v.runtime_instance_id.clone())
                .unwrap_or_default(),
            origin: instance
                .as_ref()
                .map(|v| v.origin.as_db().into())
                .unwrap_or_default(),
            game_version: instance
                .as_ref()
                .map(|v| v.game_version.clone())
                .unwrap_or_default(),
            platform: instance
                .as_ref()
                .map(|v| v.platform.clone())
                .unwrap_or_default(),
            capabilities: instance
                .as_ref()
                .map(|v| v.runtime_capabilities.clone())
                .unwrap_or_default(),
            server_name: "Abya CLI".into(),
            server_version: "1".into(),
            instructions: String::new(),
            last_error: result.err().map(|e| e.message).unwrap_or_default(),
        }
    }
    pub fn list_tools(&self, id: &str) -> AppResult<Value> {
        let value = self.run(
            id,
            &["capability", "list"],
            None,
            "desktop",
            &uuid::Uuid::new_v4().to_string(),
            Duration::from_secs(30),
            Default::default(),
        )?;
        Ok(value["data"]["tools"].clone())
    }
    pub fn call_tool_scoped(
        &self,
        id: &str,
        name: &str,
        input: Value,
        session: &str,
        operation: &str,
    ) -> AppResult<RuntimeToolResult> {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock();
            if active.contains_key(operation) {
                return Err(AppError::validation(
                    "Operation is already running; do not replay writes.",
                ));
            }
            active.insert(
                operation.into(),
                (id.into(), session.into(), cancel.clone()),
            );
        }
        let result = self.run(
            id,
            &["capability", "run", name],
            Some(input),
            session,
            operation,
            Duration::from_secs(690),
            cancel,
        );
        self.active.lock().remove(operation);
        let value = result?;
        Ok(RuntimeToolResult {
            content: value["content"].as_array().cloned().unwrap_or_default(),
            is_error: value["success"] != true,
        })
    }
    pub fn cancel(&self, id: &str, operation: &str, session: &str) -> AppResult<Value> {
        let active = self.active.lock();
        let (owner, caller, flag) = active
            .get(operation)
            .ok_or_else(|| AppError::not_found("Active operation"))?;
        if owner != id || caller != session {
            return Err(AppError::validation("Operation context mismatch."));
        }
        flag.store(true, Ordering::SeqCst);
        Ok(json!({"cancelRequested":true,"outcome":"unknown_until_verified"}))
    }
    pub fn wait_for_cli(&self, id: &str, timeout: Duration) -> AppResult<RuntimeBridgeState> {
        let start = Instant::now();
        loop {
            let state = self.state(id);
            if state.connected {
                return Ok(state);
            }
            if start.elapsed() >= timeout {
                return Err(AppError::new("runtimeCliTimeout", state.last_error, ""));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    pub fn cancel_session(&self, session: &str) {
        for (_, caller, flag) in self.active.lock().values() {
            if caller == session {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }
    pub fn shutdown(&self) {
        for (_, _, flag) in self.active.lock().values() {
            flag.store(true, Ordering::SeqCst);
        }
    }
    pub fn forget_instance(&self, id: &str) {
        for (owner, _, flag) in self.active.lock().values() {
            if owner == id {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }
}
