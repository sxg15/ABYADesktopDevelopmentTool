use super::{
    GameInstance,
    memory_models::{MemoryPolicy, MemoryStatus},
    memory_probe::MemorySample,
    memory_report,
};
use crate::foundation::{AppError, AppResult};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
};

pub(super) type ContextReader = Arc<dyn Fn(&str) -> Value + Send + Sync>;
#[derive(Default)]
struct InstanceTrace {
    identity: Value,
    samples: VecDeque<MemorySample>,
    events: VecDeque<Value>,
    status: MemoryStatus,
    recovered_samples: usize,
}
#[derive(Clone)]
pub(super) struct MemoryGuard {
    root: PathBuf,
    policy: Arc<Mutex<MemoryPolicy>>,
    traces: Arc<Mutex<HashMap<String, InstanceTrace>>>,
    pub context: Arc<Mutex<Option<ContextReader>>>,
}
impl MemoryGuard {
    pub fn new(root: PathBuf) -> Self {
        let loaded = std::fs::read(root.join("policy.json"))
            .ok()
            .filter(|bytes| bytes.len() < 4096)
            .and_then(|bytes| serde_json::from_slice::<MemoryPolicy>(&bytes).ok())
            .filter(valid_policy)
            .unwrap_or_default();
        let guard = Self {
            root,
            policy: Arc::new(Mutex::new(loaded)),
            traces: Default::default(),
            context: Default::default(),
        };
        guard.recover();
        guard
    }
    pub fn policy(&self) -> MemoryPolicy {
        self.policy.lock().clone()
    }
    pub fn status(&self, id: &str) -> MemoryStatus {
        self.traces
            .lock()
            .get(id)
            .map(|v| v.status.clone())
            .unwrap_or_else(|| MemoryStatus {
                instance_id: id.into(),
                ..Default::default()
            })
    }
    pub fn pressured(&self, id: &str) -> bool {
        self.traces
            .lock()
            .get(id)
            .is_some_and(|trace| trace.status.pressured)
    }
    pub fn launch_warning(&self) -> Option<String> {
        #[cfg(windows)]
        {
            let sample = match super::memory_probe::MemoryProbe::new(std::process::id())
                .and_then(|probe| probe.sample())
            {
                Ok(sample) => sample,
                Err(_) => return Some("启动前内存采样不可用；已继续启动，请留意系统资源。".into()),
            };
            let policy = self.policy();
            let startup = policy.private_limit_mib.min(3072) * 1024 * 1024;
            let pending: u64 = self
                .traces
                .lock()
                .values()
                .filter(|t| t.identity["ended"] != true)
                .map(|t| {
                    startup.saturating_sub(
                        t.status
                            .latest
                            .as_ref()
                            .map(|s| s.private_bytes)
                            .unwrap_or(0),
                    )
                })
                .sum();
            let required = policy.available_floor_mib * 1024 * 1024 + pending + startup;
            if sample.available_bytes < required
                || sample
                    .commit_limit_bytes
                    .saturating_sub(sample.commit_bytes)
                    < pending + startup
            {
                return Some(
                    "系统可用内存或提交余量偏低，可能影响稳定性；此项仅警告，已继续启动。".into(),
                );
            }
        }
        None
    }
    fn recover(&self) {
        let Ok(entries) = std::fs::read_dir(self.root.join("active")) else {
            return;
        };
        for entry in entries.flatten().take(1024) {
            let path = entry.path();
            let Some(id) = path.file_stem().and_then(|v| v.to_str()) else {
                continue;
            };
            if uuid::Uuid::parse_str(id).is_err() || !entry.file_type().is_ok_and(|v| v.is_file()) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if bytes.len() > 32768 {
                continue;
            }
            let Ok(mut identity) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if identity["ended"] == true || identity["instanceId"] != id {
                continue;
            }
            let read_tail = |suffix: &str| -> Vec<Value> {
                use std::io::{Read, Seek, SeekFrom};
                let Ok(mut file) = std::fs::File::open(
                    self.root
                        .join("active")
                        .join(format!("{id}.{suffix}.jsonl")),
                ) else {
                    return vec![];
                };
                let length = file.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = file.seek(SeekFrom::Start(length.saturating_sub(256 * 1024)));
                let mut data = Vec::new();
                let _ = file.take(256 * 1024).read_to_end(&mut data);
                String::from_utf8_lossy(&data)
                    .lines()
                    .rev()
                    .take(120)
                    .filter_map(|line| serde_json::from_str(line).ok())
                    .collect()
            };
            let samples: Vec<MemorySample> = read_tail("samples")
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect();
            if let Ok(incident) = memory_report::incident(
                &self.root,
                &identity,
                &self.policy(),
                "上次监测未正常结束；可能为 APP 异常退出或系统中断，不能据此认定 OOM",
                &samples,
                &read_tail("operations"),
            ) {
                let _ = memory_report::write_json(
                    &self.root.join(&incident).join("codex-context.json"),
                    &json!({"unavailable":"恢复记录不伪造故障时 Codex 状态；请按持久请求编号核对原工作流"}),
                );
                identity["ended"] = json!(true);
                identity["recoveredIncidentId"] = json!(incident);
                let _ = memory_report::write_json(&path, &identity);
            }
        }
    }
    pub fn start(&self, instance: &GameInstance) -> std::io::Result<()> {
        let identity = json!({"instanceId":instance.id,"taskId":instance.task_id,"pid":instance.pid,
            "startedAt":instance.started_at,"executablePath":instance.executable_path,
            "archive":instance.profile.as_ref().and_then(|p|p.archive.as_ref()),"ended":false,
            "desktopRelease":option_env!("ABYA_RELEASE_ID").unwrap_or("development"),"playerLogPath":instance.log_file_path});
        self.traces.lock().insert(
            instance.id.clone(),
            InstanceTrace {
                identity,
                status: MemoryStatus {
                    instance_id: instance.id.clone(),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        std::fs::create_dir_all(self.root.join("active"))?;
        memory_report::write_json(
            &self
                .root
                .join("active")
                .join(format!("{}.json", instance.id)),
            &self.traces.lock()[&instance.id].identity,
        )?;
        Ok(())
    }
    pub fn arm(&self, id: &str, auto_stop: bool) -> AppResult<()> {
        if !self.event(id, json!({"at":chrono::Utc::now().to_rfc3339(),"kind":"auto_stop_permission","enabled":auto_stop,"source":"app-user"})) && auto_stop {
            return Err(AppError::validation("无法记录自动停止授权，设置未应用。"));
        }
        let mut traces = self.traces.lock();
        let trace = traces
            .get_mut(id)
            .ok_or_else(|| AppError::validation("实例尚未开始内存采样。"))?;
        trace.status.auto_stop = auto_stop;
        trace.identity["autoStopAuthorized"] = json!(auto_stop);
        Ok(())
    }
    pub fn set_policy(&self, policy: MemoryPolicy) -> AppResult<()> {
        if !valid_policy(&policy) {
            return Err(AppError::validation("内存保护阈值不在允许范围内。"));
        }
        std::fs::create_dir_all(&self.root)?;
        memory_report::write_json(&self.root.join("policy.json"), &json!(policy))?;
        *self.policy.lock() = policy.clone();
        let active: Vec<_> = self
            .traces
            .lock()
            .iter()
            .filter(|(_, trace)| trace.identity["ended"] != true)
            .map(|(id, _)| id.clone())
            .collect();
        for id in active {
            self.event(&id,json!({"at":chrono::Utc::now().to_rfc3339(),"kind":"policy_changed","policy":policy,"source":"app-user"}));
        }
        Ok(())
    }
    pub fn error(&self, id: &str, error: &str) {
        if let Some(trace) = self.traces.lock().get_mut(id) {
            trace.status.error = memory_report::safe_text(error);
        }
    }
    pub fn event(&self, id: &str, event: Value) -> bool {
        let mut traces = self.traces.lock();
        let Some(trace) = traces.get_mut(id) else {
            return false;
        };
        trace.events.push_back(event.clone());
        if trace.events.len() > 128 {
            trace.events.pop_front();
        }
        let file = self
            .root
            .join("active")
            .join(format!("{id}.operations.jsonl"));
        let result = memory_report::append(&file, &event, 512 * 1024).and_then(|_| {
            if let Some(incident) = &trace.status.incident_id {
                memory_report::append(
                    &self.root.join(incident).join("operations.jsonl"),
                    &event,
                    512 * 1024,
                )?;
            }
            Ok(())
        });
        if let Err(error) = result {
            trace.status.error = error.to_string();
            return false;
        }
        true
    }
    pub fn sample(&self, id: &str, sample: MemorySample) -> bool {
        let policy = self.policy();
        let mut traces = self.traces.lock();
        let Some(trace) = traces.get_mut(id) else {
            return false;
        };
        // 冷启动自然增长不作为泄漏信号；绝对占用保护始终有效，系统余量仅警告。
        let previous = if trace.samples.len() >= 120 {
            trace.samples.iter().rev().nth(59)
        } else {
            None
        };
        let reason = pressure_reason(&policy, &sample, previous);
        trace.status.warning = system_warning(&policy, &sample);
        trace.status.peak_private_bytes = trace.status.peak_private_bytes.max(sample.private_bytes);
        trace.status.latest = Some(sample.clone());
        trace.samples.push_back(sample.clone());
        if trace.samples.len() > 120 {
            trace.samples.pop_front();
        }
        let result = memory_report::append(
            &self.root.join("active").join(format!("{id}.samples.jsonl")),
            &json!(sample),
            512 * 1024,
        );
        if let Err(error) = result {
            trace.status.error = error.to_string();
        }
        let new_episode = !reason.is_empty() && !trace.status.pressured;
        if !reason.is_empty() {
            trace.status.pressured = true;
            trace.recovered_samples = 0;
            if new_episode {
                Self::create_incident(&self.root, trace, &policy, &reason);
            }
        } else if trace.status.pressured {
            trace.recovered_samples += 1;
            if trace.recovered_samples >= 10 {
                trace.status.pressured = false;
            }
        }
        if let Some(incident) = &trace.status.incident_id
            && let Err(error) = memory_report::append(
                &self.root.join(incident).join("samples.jsonl"),
                &json!(sample),
                512 * 1024,
            )
        {
            trace.status.error = error.to_string();
        }
        let stop = trace.status.pressured && trace.status.auto_stop;
        let context_id = new_episode
            .then(|| trace.status.incident_id.clone())
            .flatten();
        let task = trace.identity["taskId"].as_str().unwrap_or("").to_string();
        drop(traces);
        if let Some(incident) = context_id {
            self.capture_context(&incident, &task);
        }
        stop
    }
    fn create_incident(
        root: &Path,
        trace: &mut InstanceTrace,
        policy: &MemoryPolicy,
        reason: &str,
    ) {
        let samples: Vec<_> = trace.samples.iter().cloned().collect();
        let events: Vec<_> = trace.events.iter().cloned().collect();
        match memory_report::incident(root, &trace.identity, policy, reason, &samples, &events) {
            Ok(id) => {
                trace.status.report_path = Some(
                    root.join(&id)
                        .join("report.md")
                        .to_string_lossy()
                        .into_owned(),
                );
                trace.status.incident_id = Some(id);
            }
            Err(error) => trace.status.error = format!("异常报告写入失败：{error}"),
        }
    }
    fn capture_context(&self, incident: &str, task: &str) {
        let reader = self.context.lock().clone();
        let target = self.root.join(incident).join("codex-context.json");
        let task = task.to_owned();
        let _ = memory_report::write_json(
            &target,
            &json!({"status":"pending","note":"上下文采集不阻塞保护和采样"}),
        );
        std::thread::spawn(move || {
            let value = reader
                .map(|read| read(&task))
                .unwrap_or_else(|| json!({"unavailable":"Codex 上下文读取器未连接"}));
            let _ = memory_report::write_json(&target, &value);
            if let Ok(mut report) = std::fs::OpenOptions::new()
                .append(true)
                .open(target.with_file_name("report.md"))
            {
                use std::io::Write;
                let _ = writeln!(
                    report,
                    "\n## Codex 操作上下文\n\n```json\n{}\n```",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
            }
        });
    }
    pub fn finish(&self, id: &str, outcome: Value, abnormal: bool) {
        let policy = self.policy();
        let mut traces = self.traces.lock();
        let Some(trace) = traces.get_mut(id) else {
            return;
        };
        let new_incident = abnormal && trace.status.incident_id.is_none();
        if new_incident {
            Self::create_incident(
                &self.root,
                trace,
                &policy,
                "进程异常结束；尚未确认是否由内存导致",
            );
        }
        trace.identity["ended"] = json!(true);
        trace.identity["outcome"] = outcome.clone();
        let result = memory_report::write_json(
            &self.root.join("active").join(format!("{id}.json")),
            &trace.identity,
        )
        .and_then(|_| {
            if let Some(incident) = &trace.status.incident_id {
                memory_report::append(
                    &self.root.join(incident).join("outcomes.jsonl"),
                    &outcome,
                    64 * 1024,
                )?;
                memory_report::write_json(
                    &self.root.join(incident).join("outcome.json"),
                    &outcome,
                )?;
                memory_report::player_tail(
                    &self.root.join(incident).join("player-tail-after.log"),
                    &trace.identity,
                )
            } else {
                Ok(())
            }
        });
        if let Err(error) = result {
            trace.status.error = error.to_string();
        }
        let context_id = new_incident
            .then(|| trace.status.incident_id.clone())
            .flatten();
        let task = trace.identity["taskId"].as_str().unwrap_or("").to_string();
        if traces.len() > 128 {
            let old = traces
                .iter()
                .find(|(key, item)| *key != id && item.identity["ended"] == true)
                .map(|(key, _)| key.clone());
            if let Some(old) = old {
                traces.remove(&old);
            }
        }
        drop(traces);
        if let Some(incident) = context_id {
            self.capture_context(&incident, &task);
        }
    }
    pub fn report(&self, id: &str) -> AppResult<PathBuf> {
        let path = self
            .status(id)
            .report_path
            .ok_or_else(|| AppError::not_found("内存异常报告"))?;
        let path = PathBuf::from(path).canonicalize()?;
        if !path.starts_with(self.root.canonicalize()?) {
            return Err(AppError::validation("报告路径越界。"));
        }
        Ok(path)
    }
    pub fn record_stop_outcome(&self, id: &str, outcome: Value) {
        let mut traces = self.traces.lock();
        let Some(trace) = traces.get_mut(id) else {
            return;
        };
        let Some(incident) = &trace.status.incident_id else {
            return;
        };
        let result = memory_report::append(
            &self.root.join(incident).join("outcomes.jsonl"),
            &outcome,
            64 * 1024,
        )
        .and_then(|_| {
            memory_report::write_json(&self.root.join(incident).join("outcome.json"), &outcome)
        });
        if let Err(error) = result {
            trace.status.error = error.to_string();
        }
    }
}

fn valid_policy(policy: &MemoryPolicy) -> bool {
    (256..=262144).contains(&policy.private_limit_mib)
        && (64..=65536).contains(&policy.growth_limit_mib)
        && (256..=262144).contains(&policy.available_floor_mib)
        && (50..=98).contains(&policy.commit_limit_percent)
}

pub(super) fn pressure_reason(
    policy: &MemoryPolicy,
    sample: &MemorySample,
    previous: Option<&MemorySample>,
) -> String {
    const MIB: u64 = 1024 * 1024;
    let mut reasons = Vec::new();
    if sample.private_bytes > policy.private_limit_mib * MIB {
        reasons.push("游戏私有提交超过阈值");
    }
    if previous.is_some_and(|old| {
        sample.private_bytes.saturating_sub(old.private_bytes) > policy.growth_limit_mib * MIB
    }) {
        reasons.push("近一分钟私有提交增长过快");
    }
    reasons.join("；")
}

pub(super) fn system_warning(policy: &MemoryPolicy, sample: &MemorySample) -> String {
    const MIB: u64 = 1024 * 1024;
    let mut reasons = Vec::new();
    if sample.available_bytes < policy.available_floor_mib * MIB {
        reasons.push("系统可用物理内存不足");
    }
    if sample.commit_limit_bytes > 0
        && sample.commit_bytes as u128 * 100
            >= sample.commit_limit_bytes as u128 * policy.commit_limit_percent as u128
    {
        reasons.push("系统提交量接近上限");
    }
    if reasons.is_empty() {
        String::new()
    } else {
        format!("{}；仅警告，不限制启动或工具操作。", reasons.join("；"))
    }
}
