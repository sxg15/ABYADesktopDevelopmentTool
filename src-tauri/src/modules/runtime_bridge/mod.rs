mod acceptance;
mod authoring;
mod operation_receipts;
mod operation_trace;
mod process;
fn memory_safe_tool(name: &str) -> bool {
    matches!(
        name,
        "read_me_first" | "editor_get_state" | "capability_search" | "capability_describe"
    )
}
use crate::foundation::{AppError, AppResult};
use crate::modules::{instances::InstanceService, tasks::TaskService};
use operation_trace::operation_metadata;
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
    acceptance_busy: Arc<Mutex<std::collections::HashSet<String>>>,
    acceptance_progress: Arc<Mutex<HashMap<String, Value>>>,
}
impl RuntimeBridgeService {
    pub fn new(instances: InstanceService, tasks: TaskService) -> Self {
        Self {
            instances,
            tasks,
            active: Default::default(),
            in_flight: Default::default(),
            acceptance_busy: Default::default(),
            acceptance_progress: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn run_inner(
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
        if args.first() == Some(&"capability")
            && args.get(1) == Some(&"run")
            && !memory_safe_tool(args.get(2).copied().unwrap_or(""))
            && self.instances.memory_pressured(id)
        {
            return Err(AppError::new(
                "instanceMemoryPressure",
                "实例处于内存保护状态；请查看异常报告，不要重放写操作。",
                "not_executed",
            ));
        }
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
        let tool = args.get(2).or(args.first()).copied().unwrap_or("unknown");
        let event = |status: &str| {
            json!({"at":chrono::Utc::now().to_rfc3339(),"sessionId":session,
            "requestId":operation,"tool":crate::modules::instances::diagnostic_text(tool),"status":status})
        };
        let mut started = event("started");
        if let Some(input) = &input {
            match operation_metadata(input) {
                Ok(metadata) => started["input"] = metadata,
                Err(error) => {
                    started["status"] = json!("input_rejected_not_executed");
                    self.instances.record_memory_operation(id, started);
                    return Err(error);
                }
            }
        }
        if !self.instances.record_memory_operation(id, started)
            && args.get(1) == Some(&"run")
            && !memory_safe_tool(tool)
        {
            return Err(AppError::new(
                "memoryTraceUnavailable",
                "无法持久化操作诊断记录，已阻止本次操作；请检查日志目录。",
                "not_executed",
            ));
        }
        let pressure_instance = self.instances.clone();
        let pressure_id = id.to_string();
        let result = process::execute(
            pid,
            id,
            args,
            input,
            session,
            operation,
            &output,
            timeout,
            cancel,
            &|| {
                args.get(1) == Some(&"run")
                    && !memory_safe_tool(tool)
                    && pressure_instance.memory_pressured(&pressure_id)
            },
        );
        let mut completed = event(if result.is_ok() {
            "returned"
        } else {
            "failed_or_interrupted"
        });
        if let Ok(value) = &result {
            completed["runtimeSuccess"] = value.get("success").cloned().unwrap_or(Value::Null);
        }
        if let Err(error) = &result {
            completed["errorCode"] = json!(error.code);
            completed["message"] =
                json!(crate::modules::instances::diagnostic_text(&error.message));
        }
        self.instances.record_memory_operation(id, completed);
        result
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
            if !memory_safe_tool(name) && active.values().any(|(owner, _, _)| owner == id) {
                return Err(AppError::new(
                    "runtimeInstanceBusy",
                    "同一实例一次只执行一个工具操作；请等待原请求结束。",
                    "not_executed",
                ));
            }
            active.insert(
                operation.into(),
                (id.into(), session.into(), cancel.clone()),
            );
        }
        let result = self.check_authoring(id, name).and_then(|()| {
            self.run(
                id,
                &["capability", "run", name],
                Some(input),
                session,
                operation,
                Duration::from_secs(690),
                cancel,
            )
        });
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
