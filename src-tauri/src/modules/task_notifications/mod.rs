mod model;
mod projection;
mod store;
#[cfg(test)]
mod tests;

use crate::foundation::notifications::{self, NotificationBackend, ToastMessage};
use crate::foundation::{AppError, AppPaths, AppResult, Database, SettingsService, change_signal};
use crate::modules::codex_terminal::CodexTerminalService;
use crate::modules::grok_terminal::GrokTerminalService;
use crate::modules::tasks::{DevelopmentTask, TaskService, TaskStatus};
pub use model::{NotificationActivation, NotificationSettings, TaskAttentionSnapshot};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};
use store::NotificationStore;

type ChangedHandler = Arc<dyn Fn(TaskAttentionSnapshot) + Send + Sync>;
type ActivatedHandler = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct MonitorState {
    initialized: bool,
    baseline_tasks: HashSet<String>,
    snapshots: HashMap<String, TaskAttentionSnapshot>,
    shown: HashSet<String>,
}

struct Inner {
    tasks: TaskService,
    codex: CodexTerminalService,
    grok: GrokTerminalService,
    settings: SettingsService,
    store: NotificationStore,
    backend: Arc<dyn NotificationBackend>,
    foreground: Arc<dyn Fn() -> bool + Send + Sync>,
    state: Mutex<MonitorState>,
    scan: Mutex<()>,
    stopped: AtomicBool,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    activation_input: Mutex<mpsc::Receiver<String>>,
    activations: Mutex<VecDeque<NotificationActivation>>,
    changed: ChangedHandler,
    activated: ActivatedHandler,
    last_error: Mutex<String>,
}

#[derive(Clone)]
pub struct TaskNotificationService(Arc<Inner>);

impl TaskNotificationService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        paths: &AppPaths,
        database: Database,
        settings: SettingsService,
        tasks: TaskService,
        codex: CodexTerminalService,
        grok: GrokTerminalService,
        changed: ChangedHandler,
        activated: ActivatedHandler,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let backend = notifications::start(
            paths,
            Arc::new(move |key| {
                let _ = sender.send(key);
                change_signal::notify();
            }),
        );
        Self(Arc::new(Inner {
            tasks,
            codex,
            grok,
            settings,
            store: NotificationStore(database),
            backend,
            foreground: Arc::new(notifications::is_foreground),
            state: Default::default(),
            scan: Default::default(),
            stopped: AtomicBool::new(false),
            worker: Default::default(),
            activation_input: Mutex::new(receiver),
            activations: Default::default(),
            changed,
            activated,
            last_error: Default::default(),
        }))
    }

    pub fn start(&self) {
        let mut worker = self.0.worker.lock();
        if worker.is_some() {
            return;
        }
        let service = self.clone();
        *worker = Some(std::thread::spawn(move || {
            // History is scoped to this app/data identity, never another application's notifications.
            let _ = service.0.backend.clear();
            let _ = service.0.store.reset_active();
            let mut revision = change_signal::revision();
            while !service.0.stopped.load(Ordering::SeqCst) {
                if let Err(error) = service.refresh() {
                    *service.0.last_error.lock() = error.message;
                }
                service.process_activations();
                revision = change_signal::wait(revision, Duration::from_secs(3));
                // Coalesce bursty workflow events without depending on a WebView timer.
                std::thread::sleep(Duration::from_millis(100));
            }
        }));
    }

    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::SeqCst);
        change_signal::notify();
        if let Some(worker) = self.0.worker.lock().take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
        self.0.backend.shutdown();
    }

    pub fn snapshot(&self, task_id: &str) -> TaskAttentionSnapshot {
        self.0
            .state
            .lock()
            .snapshots
            .get(task_id)
            .cloned()
            .unwrap_or_else(|| TaskAttentionSnapshot {
                task_id: task_id.into(),
                unavailable: true,
                ..Default::default()
            })
    }

    pub fn notification_settings(&self) -> AppResult<NotificationSettings> {
        let platform = self.0.backend.status();
        let error = self.0.last_error.lock().clone();
        Ok(NotificationSettings {
            enabled: self.0.store.enabled()?,
            available: platform.available,
            reason: if platform.reason.is_empty() {
                error
            } else {
                platform.reason
            },
            identity: platform.identity,
            test_environment: platform.test_environment,
        })
    }

    pub fn set_enabled(&self, enabled: bool) -> AppResult<NotificationSettings> {
        self.0.store.set_enabled(enabled)?;
        change_signal::notify();
        self.notification_settings()
    }

    pub fn test(&self) -> AppResult<()> {
        let state = self.notification_settings()?;
        if !state.available {
            return Err(AppError::validation(state.reason));
        }
        let english = self.english();
        self.0.backend.show(ToastMessage {
            key: "test".into(),
            test: true,
            title: if english {
                "ABYA: Notification test"
            } else {
                "ABYA：后台提醒测试"
            }
            .into(),
            body: if english {
                "Click to return to notification settings. Tasks are not changed."
            } else {
                "点击返回通知设置。此测试不会修改任务或启动 AI。"
            }
            .into(),
        })?;
        *self.0.last_error.lock() = String::new();
        Ok(())
    }

    pub fn take_activations(&self) -> Vec<NotificationActivation> {
        self.0.activations.lock().iter().cloned().collect()
    }

    pub fn acknowledge_activations(&self, ids: &[String]) {
        self.0
            .activations
            .lock()
            .retain(|activation| !ids.contains(&activation.id));
    }

    fn english(&self) -> bool {
        match self.0.settings.locale().unwrap_or_default().as_str() {
            "zh-CN" => false,
            "en-US" => true,
            _ => notifications::system_english(),
        }
    }

    fn capture(&self, task: &DevelopmentTask) -> AppResult<TaskAttentionSnapshot> {
        if task.status != TaskStatus::Active {
            return Ok(TaskAttentionSnapshot {
                task_id: task.id.clone(),
                ..Default::default()
            });
        }
        let production = self.0.tasks.production_attention(task)?;
        let mut conversations = self.0.codex.attention_states(task, &production)?;
        conversations.extend(self.0.grok.attention_states(task)?);
        Ok(projection::project(task, &production, &conversations))
    }

    fn refresh(&self) -> AppResult<()> {
        let _scan = self.0.scan.lock();
        let tasks = self.0.tasks.list_readonly()?;
        {
            let mut state = self.0.state.lock();
            if !state.initialized {
                state.baseline_tasks = tasks.iter().map(|t| t.id.clone()).collect();
                state.initialized = true;
            }
        }
        let enabled = self.0.store.enabled()?;
        let english = self.english();
        let platform = self.0.backend.status();
        let mut live_keys = HashSet::new();
        let mut unreadable_tasks = HashSet::new();
        let mut changed = Vec::new();
        for task in &tasks {
            if self.0.stopped.load(Ordering::SeqCst) {
                return Ok(());
            }
            let snapshot = match self.capture(task) {
                Ok(value) => value,
                Err(_) => {
                    unreadable_tasks.insert(task.id.clone());
                    // Failed reads are not evidence of resolution or a new execution failure.
                    let next = TaskAttentionSnapshot {
                        task_id: task.id.clone(),
                        unavailable: true,
                        ..Default::default()
                    };
                    let mut state = self.0.state.lock();
                    if state.snapshots.get(&task.id) != Some(&next) {
                        changed.push(next.clone());
                    }
                    state.snapshots.insert(task.id.clone(), next);
                    continue;
                }
            };
            let baseline = self.0.state.lock().baseline_tasks.remove(&task.id);
            for item in &snapshot.items {
                live_keys.insert(item.key.clone());
                let disposition = if baseline {
                    "baseline"
                } else if !enabled {
                    "disabled"
                } else if (self.0.foreground)() {
                    "foreground"
                } else if !platform.available {
                    "unavailable"
                } else {
                    "attempted"
                };
                if self.0.store.observe(&item.key, &item.target, disposition)?
                    && disposition == "attempted"
                {
                    // Recheck the current item after persistence, immediately before dispatch.
                    let current = self
                        .0
                        .tasks
                        .get_readonly(&task.id)
                        .and_then(|task| self.capture(&task));
                    if !current.is_ok_and(|s| s.items.iter().any(|i| i.key == item.key)) {
                        continue;
                    }
                    let text = item.text(english);
                    match self.0.backend.show(ToastMessage {
                        key: item.key.clone(),
                        title: format!(
                            "{}：{}",
                            task.title.chars().take(60).collect::<String>(),
                            text.title
                        ),
                        body: text.instruction.clone(),
                        test: false,
                    }) {
                        Ok(shown) => {
                            self.0.store.delivered(&item.key, shown)?;
                            if shown {
                                self.0.state.lock().shown.insert(item.key.clone());
                            }
                        }
                        Err(error) => {
                            *self.0.last_error.lock() = error.message;
                        }
                    }
                }
            }
            let mut state = self.0.state.lock();
            if state.snapshots.get(&task.id) != Some(&snapshot) {
                changed.push(snapshot.clone());
            }
            state.snapshots.insert(task.id.clone(), snapshot);
        }
        let task_ids: HashSet<_> = tasks.iter().map(|t| t.id.as_str()).collect();
        {
            let mut state = self.0.state.lock();
            let deleted = state
                .snapshots
                .keys()
                .filter(|id| !task_ids.contains(id.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            for id in deleted {
                state.snapshots.remove(&id);
                changed.push(TaskAttentionSnapshot {
                    task_id: id,
                    ..Default::default()
                });
            }
        }
        // Keep in-memory keys as well: task deletion cascades durable receipts.
        let active = self.0.store.active()?;
        let mut protected = HashSet::new();
        for (key, task_id) in &active {
            if unreadable_tasks.contains(task_id) && enabled {
                protected.insert(key.clone());
            }
        }
        let mut shown = self.0.state.lock().shown.clone();
        shown.extend(active.into_iter().map(|(key, _)| key));
        for key in shown {
            if ((!live_keys.contains(&key) && !protected.contains(&key)) || !enabled)
                && self.0.backend.remove(&key).is_ok()
            {
                self.0.store.retire(&key)?;
                self.0.state.lock().shown.remove(&key);
            }
        }
        drop(_scan);
        for snapshot in changed {
            (self.0.changed)(snapshot);
        }
        Ok(())
    }

    fn process_activations(&self) {
        let keys = self
            .0
            .activation_input
            .lock()
            .try_iter()
            .collect::<Vec<_>>();
        for key in keys {
            if !notifications::valid_key(&key) {
                continue;
            }
            let activation = if key == "test" {
                NotificationActivation {
                    id: uuid::Uuid::new_v4().to_string(),
                    target: None,
                    status: "test".into(),
                }
            } else {
                let target = self.0.store.target(&key).ok().flatten();
                let status = match target.as_ref() {
                    None => "missing",
                    Some(target) => match self.0.tasks.get_readonly(&target.task_id) {
                        Err(error) if error.code == "notFound" => "missing",
                        Err(_) => "unavailable",
                        Ok(task) => match self.capture(&task) {
                            Ok(snapshot) if snapshot.items.iter().any(|i| i.key == key) => {
                                "current"
                            }
                            Ok(_) => "resolved",
                            Err(_) => "unavailable",
                        },
                    },
                };
                NotificationActivation {
                    id: uuid::Uuid::new_v4().to_string(),
                    target,
                    status: status.into(),
                }
            };
            let _ = self.0.backend.remove(&key);
            let _ = self.0.store.retire(&key);
            {
                let mut pending = self.0.activations.lock();
                if pending.len() >= 32 {
                    pending.pop_front();
                }
                pending.push_back(activation);
            }
            (self.0.activated)();
        }
    }
}
