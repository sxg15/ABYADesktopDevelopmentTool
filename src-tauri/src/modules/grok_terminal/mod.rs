use crate::foundation::{AppError, AppResult};
use crate::modules::development_terminal::transcript::read_transcript_replay;
use crate::modules::development_terminal::workflow::{
    CodexActivityKind, CodexActivityStatus, CodexObservabilityStatus, CodexPlanStepStatus,
    CodexWorkflowSnapshot, CodexWorkflowTurnStatus, load_workflow, sanitize_detail, save_workflow,
    validate_conversation_title,
};
use crate::modules::tasks::TaskService;
use chrono::Utc;
use parking_lot::Mutex;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use uuid::Uuid;

const MIN_COLUMNS: u16 = 20;
const MAX_COLUMNS: u16 = 500;
const MIN_ROWS: u16 = 5;
const MAX_ROWS: u16 = 200;
const MAX_INPUT_BYTES: usize = 64 * 1024;
const CONVERSATIONS_DIRECTORY: &str = "grok-conversations";
const CONVERSATION_METADATA_FILE: &str = "conversation.json";
const CONVERSATION_TRANSCRIPT_FILE: &str = "transcript.log";
const ACP_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(20);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const PROCESS_TREE_STOP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokTerminalAvailability {
    pub available: bool,
    pub working_directory: String,
    pub reason: String,
    pub observability_available: bool,
    pub observability_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GrokTerminalStatus {
    Starting,
    Running,
    Stopping,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokTerminalState {
    pub task_id: String,
    pub conversation_id: String,
    pub status: GrokTerminalStatus,
    pub pid: Option<u32>,
    pub working_directory: String,
    pub exit_code: Option<u32>,
    pub last_error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GrokTerminalEvent {
    Output { data: String },
    State { state: GrokTerminalState },
    Workflow { workflow: CodexWorkflowSnapshot },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokConversation {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub native_session_id: String,
}

#[derive(Clone)]
struct LiveTerminal {
    task_id: String,
    session_id: String,
    state: Arc<Mutex<GrokTerminalState>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    transcript: Arc<Mutex<File>>,
    killer: Arc<Mutex<Box<dyn ChildKiller + Send + Sync>>>,
    subscriber: Arc<Mutex<Option<Channel<GrokTerminalEvent>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrokLaunchKind {
    Executable,
    CommandScript,
}

#[derive(Debug, Clone)]
struct ResolvedGrok {
    path: PathBuf,
    kind: GrokLaunchKind,
}

type GrokResolver = Arc<dyn Fn() -> Option<ResolvedGrok> + Send + Sync>;

#[derive(Clone)]
pub struct GrokTerminalService {
    tasks: TaskService,
    sessions: Arc<Mutex<HashMap<String, LiveTerminal>>>,
    resolver: GrokResolver,
    workflow_write_lock: Arc<Mutex<()>>,
}

impl GrokTerminalService {
    pub fn new(tasks: TaskService) -> AppResult<Self> {
        Ok(Self {
            tasks,
            sessions: Default::default(),
            resolver: Arc::new(resolve_grok),
            workflow_write_lock: Default::default(),
        })
    }

    pub fn availability(&self) -> GrokTerminalAvailability {
        let grok = (self.resolver)();
        let available = grok.is_some();
        let observability_available = grok.as_ref().is_some_and(probe_grok_observability);
        GrokTerminalAvailability {
            available,
            working_directory: display_path(&discover_working_directory().unwrap_or_default()),
            reason: if available {
                String::new()
            } else {
                "Grok CLI was not found in PATH or the standard user installation directory.".into()
            },
            observability_available,
            observability_reason: if observability_available {
                String::new()
            } else if available {
                "This Grok version does not advertise persistent ACP session updates.".into()
            } else {
                "Grok CLI is unavailable.".into()
            },
        }
    }

    pub fn list_conversations(&self, task_id: &str) -> AppResult<Vec<GrokConversation>> {
        let task = self.tasks.get(task_id)?;
        let root = conversations_root(Path::new(&task.workspace_path));
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        let mut conversations = Vec::new();
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let metadata = entry.path().join(CONVERSATION_METADATA_FILE);
            if !metadata.is_file() {
                continue;
            }
            let conversation: GrokConversation =
                serde_json::from_str(&std::fs::read_to_string(metadata)?)?;
            if conversation.task_id == task_id {
                conversations.push(conversation);
            }
        }
        conversations.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(conversations)
    }

    pub fn create_conversation(
        &self,
        task_id: &str,
        title: Option<String>,
    ) -> AppResult<GrokConversation> {
        let task = self.tasks.get(task_id)?;
        let root = conversations_root(Path::new(&task.workspace_path));
        std::fs::create_dir_all(&root)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let title = title.unwrap_or_default();
        let title = if title.trim().is_empty() {
            format!(
                "Conversation {}",
                self.list_conversations(task_id)?.len() + 1
            )
        } else {
            validate_conversation_title(&title)?
        };
        let conversation = GrokConversation {
            id: id.clone(),
            task_id: task_id.to_string(),
            title,
            created_at: now.clone(),
            updated_at: now,
            native_session_id: id.clone(),
        };
        let directory = root.join(id);
        std::fs::create_dir_all(&directory)?;
        write_conversation(&directory, &conversation)?;
        Ok(conversation)
    }

    pub fn rename_conversation(
        &self,
        task_id: &str,
        conversation_id: &str,
        title: &str,
    ) -> AppResult<GrokConversation> {
        let title = validate_conversation_title(title)?;
        let task = self.tasks.get(task_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let mut conversation = self.conversation(task_id, conversation_id)?;
        conversation.title = title;
        conversation.updated_at = Utc::now().to_rfc3339();
        write_conversation(&directory, &conversation)?;
        Ok(conversation)
    }

    pub fn workflow(
        &self,
        task_id: &str,
        conversation_id: &str,
    ) -> AppResult<CodexWorkflowSnapshot> {
        let task = self.tasks.get(task_id)?;
        self.conversation(task_id, conversation_id)?;
        load_workflow(
            &conversation_directory(Path::new(&task.workspace_path), conversation_id),
            task_id,
            conversation_id,
        )
    }

    pub fn bind_conversation(
        &self,
        task_id: &str,
        conversation_id: &str,
    ) -> AppResult<GrokConversation> {
        self.conversation(task_id, conversation_id)
    }

    pub fn record_reported_activity(
        &self,
        task_id: &str,
        conversation_id: &str,
        status: CodexActivityStatus,
        kind: CodexActivityKind,
        summary: &str,
        detail: &str,
    ) -> AppResult<CodexWorkflowSnapshot> {
        self.conversation(task_id, conversation_id)?;
        let turn_id = self
            .workflow(task_id, conversation_id)?
            .current_turn_id
            .unwrap_or_else(|| format!("reported-{}", Uuid::new_v4()));
        self.record_activity(
            task_id,
            conversation_id,
            &turn_id,
            None,
            kind,
            status,
            summary,
            detail,
            "desktopCliReport",
        )
    }

    pub fn record_desktop_tool_activity(
        &self,
        task_id: &str,
        conversation_id: &str,
        activity_id: &str,
        tool_name: &str,
        arguments: &Value,
        status: CodexActivityStatus,
    ) -> AppResult<CodexWorkflowSnapshot> {
        let workflow = self.workflow(task_id, conversation_id)?;
        let turn_id = workflow
            .current_turn_id
            .unwrap_or_else(|| format!("desktop-cli-{}", Uuid::new_v4()));
        let (kind, summary) = describe_desktop_tool(tool_name);
        self.record_activity(
            task_id,
            conversation_id,
            &turn_id,
            Some(activity_id),
            kind,
            status,
            &summary,
            &sanitize_detail(arguments),
            "desktopCli",
        )
    }

    pub fn delete_conversation(&self, task_id: &str, conversation_id: &str) -> AppResult<()> {
        let conversation = self.conversation(task_id, conversation_id)?;
        self.stop(conversation_id)?;
        let task = self.tasks.get(task_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), &conversation.id);
        if directory.is_dir() {
            std::fs::remove_dir_all(directory)?;
        }
        Ok(())
    }

    pub fn open(
        &self,
        task_id: &str,
        conversation_id: &str,
        columns: u16,
        rows: u16,
        subscriber: Channel<GrokTerminalEvent>,
    ) -> AppResult<GrokTerminalState> {
        validate_size(columns, rows)?;
        if let Some(existing) = self.sessions.lock().get(conversation_id).cloned() {
            // Keep live output behind the transcript snapshot, then atomically
            // attach the new subscriber so replay and live bytes cannot race.
            let transcript_guard = existing.transcript.lock();
            send_transcript(&existing.task_id, conversation_id, &self.tasks, &subscriber)?;
            *existing.subscriber.lock() = Some(subscriber.clone());
            if let Ok(workflow) = self.workflow(&existing.task_id, conversation_id) {
                let _ = subscriber.send(GrokTerminalEvent::Workflow { workflow });
            }
            let state = existing.state.lock().clone();
            let _ = subscriber.send(GrokTerminalEvent::State {
                state: state.clone(),
            });
            drop(transcript_guard);
            return Ok(state);
        }

        let (grok, task) = self.validate_open_request(task_id, conversation_id)?;
        let task = self.tasks.prepare_terminal_workspace(&task.id)?;
        let working_directory = PathBuf::from(&task.workspace_path);
        let conversation = self.conversation(task_id, conversation_id)?;
        let native_session_exists =
            find_grok_session_directory(&conversation.native_session_id).is_some();
        let conversation_directory = conversation_directory(&working_directory, conversation_id);
        std::fs::create_dir_all(&conversation_directory)?;
        let observability_available = probe_grok_observability(&grok);
        let _workflow_guard = self.workflow_write_lock.lock();
        let mut workflow = load_workflow(&conversation_directory, task_id, conversation_id)?;
        workflow.observability = if observability_available {
            CodexObservabilityStatus::Native
        } else {
            CodexObservabilityStatus::Compatibility
        };
        workflow.warning = if observability_available {
            String::new()
        } else {
            "This Grok version does not advertise persistent ACP session updates.".into()
        };
        save_workflow(&conversation_directory, &workflow, "observability")?;
        drop(_workflow_guard);

        let transcript = OpenOptions::new()
            .create(true)
            .append(true)
            .open(conversation_directory.join(CONVERSATION_TRANSCRIPT_FILE))?;
        let starting = GrokTerminalState {
            task_id: task_id.to_string(),
            conversation_id: conversation_id.to_string(),
            status: GrokTerminalStatus::Starting,
            pid: None,
            working_directory: display_path(&working_directory),
            exit_code: None,
            last_error: String::new(),
        };
        let _ = subscriber.send(GrokTerminalEvent::State { state: starting });
        let _ = subscriber.send(GrokTerminalEvent::Workflow {
            workflow: workflow.clone(),
        });
        send_transcript(task_id, conversation_id, &self.tasks, &subscriber)?;

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(pty_size(columns, rows))
            .map_err(AppError::internal)?;
        let reader = pair.master.try_clone_reader().map_err(AppError::internal)?;
        let writer = pair.master.take_writer().map_err(AppError::internal)?;
        let command = build_command(
            &grok,
            &working_directory,
            task_id,
            conversation_id,
            &conversation.native_session_id,
            native_session_exists,
        );
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(AppError::internal)?;
        drop(pair.slave);

        let pid = child.process_id();
        let killer = child.clone_killer();
        let state = GrokTerminalState {
            task_id: task_id.to_string(),
            conversation_id: conversation_id.to_string(),
            status: GrokTerminalStatus::Running,
            pid,
            working_directory: display_path(&working_directory),
            exit_code: None,
            last_error: String::new(),
        };
        let state_ref = Arc::new(Mutex::new(state.clone()));
        let subscriber_ref = Arc::new(Mutex::new(Some(subscriber.clone())));
        let live_session_id = Uuid::new_v4().to_string();
        let live = LiveTerminal {
            task_id: task_id.to_string(),
            session_id: live_session_id.clone(),
            state: state_ref.clone(),
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
            transcript: Arc::new(Mutex::new(transcript)),
            killer: Arc::new(Mutex::new(killer)),
            subscriber: subscriber_ref.clone(),
        };
        let transcript_ref = live.transcript.clone();
        self.sessions
            .lock()
            .insert(conversation_id.to_string(), live);
        let _ = subscriber.send(GrokTerminalEvent::State {
            state: state.clone(),
        });

        spawn_reader(reader, subscriber_ref.clone(), transcript_ref);
        if observability_available {
            spawn_acp_watcher(AcpWatcher {
                tasks: self.tasks.clone(),
                sessions: self.sessions.clone(),
                workflow_write_lock: self.workflow_write_lock.clone(),
                task_id: task_id.to_string(),
                conversation_id: conversation_id.to_string(),
                native_session_id: conversation.native_session_id.clone(),
                live_session_id: live_session_id.clone(),
                skip_existing_events: native_session_exists,
            });
        }
        self.spawn_monitor(
            conversation_id.to_string(),
            live_session_id,
            child,
            state_ref,
            subscriber_ref,
        );
        Ok(state)
    }

    pub fn write(&self, conversation_id: &str, data: &str) -> AppResult<()> {
        if data.len() > MAX_INPUT_BYTES {
            return Err(AppError::validation("Terminal input is too large."));
        }
        let session = self.session(conversation_id)?;
        let mut writer = session.writer.lock();
        writer.write_all(data.as_bytes())?;
        writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, conversation_id: &str, columns: u16, rows: u16) -> AppResult<()> {
        validate_size(columns, rows)?;
        let session = self.session(conversation_id)?;
        session
            .master
            .lock()
            .resize(pty_size(columns, rows))
            .map_err(AppError::internal)
    }

    pub fn stop(&self, conversation_id: &str) -> AppResult<()> {
        crate::foundation::cli_sessions::revoke("grok", conversation_id);
        let Some(session) = self.sessions.lock().remove(conversation_id) else {
            return Ok(());
        };
        disconnect_and_terminate(session)
    }

    pub fn stop_all(&self) {
        let conversation_ids = self.sessions.lock().keys().cloned().collect::<Vec<_>>();
        for conversation_id in conversation_ids {
            let _ = self.stop(&conversation_id);
        }
    }

    pub fn stop_task(&self, task_id: &str) {
        let conversation_ids = self
            .sessions
            .lock()
            .iter()
            .filter(|(_, session)| session.task_id == task_id)
            .map(|(conversation_id, _)| conversation_id.clone())
            .collect::<Vec<_>>();
        for conversation_id in conversation_ids {
            let _ = self.stop(&conversation_id);
        }
    }

    fn conversation(&self, task_id: &str, conversation_id: &str) -> AppResult<GrokConversation> {
        let task = self.tasks.get(task_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        if !directory.is_dir() {
            return Err(AppError::not_found("Grok conversation"));
        }
        let conversation: GrokConversation = serde_json::from_str(&std::fs::read_to_string(
            directory.join(CONVERSATION_METADATA_FILE),
        )?)?;
        if conversation.task_id != task_id || conversation.id != conversation_id {
            return Err(AppError::not_found("Grok conversation"));
        }
        Ok(conversation)
    }

    fn session(&self, conversation_id: &str) -> AppResult<LiveTerminal> {
        self.sessions
            .lock()
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| AppError::validation("The Grok terminal is not running."))
    }

    fn validate_open_request(
        &self,
        task_id: &str,
        conversation_id: &str,
    ) -> AppResult<(ResolvedGrok, crate::modules::tasks::DevelopmentTask)> {
        let task = self.tasks.get(task_id)?;
        self.conversation(task_id, conversation_id)?;
        let grok = (self.resolver)()
            .ok_or_else(|| AppError::validation("Grok CLI is not installed or is unavailable."))?;
        Ok((grok, task))
    }

    #[allow(clippy::too_many_arguments)]
    fn record_activity(
        &self,
        task_id: &str,
        conversation_id: &str,
        turn_id: &str,
        activity_id: Option<&str>,
        kind: CodexActivityKind,
        status: CodexActivityStatus,
        summary: &str,
        detail: &str,
        source: &str,
    ) -> AppResult<CodexWorkflowSnapshot> {
        let task = self.tasks.get(task_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let _workflow_guard = self.workflow_write_lock.lock();
        let mut workflow = load_workflow(&directory, task_id, conversation_id)?;
        workflow.record_activity(turn_id, activity_id, kind, status, summary, detail, source);
        save_workflow(&directory, &workflow, "activity")?;
        send_workflow_to_live_session(&self.sessions, conversation_id, &workflow);
        Ok(workflow)
    }

    fn spawn_monitor(
        &self,
        conversation_id: String,
        session_id: String,
        mut child: Box<dyn portable_pty::Child + Send + Sync>,
        state: Arc<Mutex<GrokTerminalState>>,
        subscriber: Arc<Mutex<Option<Channel<GrokTerminalEvent>>>>,
    ) {
        let sessions = self.sessions.clone();
        std::thread::spawn(move || {
            let result = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => std::thread::sleep(Duration::from_millis(200)),
                    Err(error) => break Err(error),
                }
            };
            let is_current = sessions
                .lock()
                .get(&conversation_id)
                .is_some_and(|session| session.session_id == session_id);
            if is_current {
                sessions.lock().remove(&conversation_id);
                crate::foundation::cli_sessions::revoke("grok", &conversation_id);
            }
            match result {
                Ok(status) => update_state(
                    &state,
                    &subscriber,
                    GrokTerminalStatus::Exited,
                    Some(status.exit_code()),
                    "",
                ),
                Err(error) => update_state(
                    &state,
                    &subscriber,
                    GrokTerminalStatus::Failed,
                    None,
                    &error.to_string(),
                ),
            }
        });
    }

    #[cfg(test)]
    fn for_test(tasks: TaskService, grok: ResolvedGrok) -> Self {
        Self {
            tasks,
            sessions: Default::default(),
            resolver: Arc::new(move || Some(grok.clone())),
            workflow_write_lock: Default::default(),
        }
    }
}

struct AcpWatcher {
    tasks: TaskService,
    sessions: Arc<Mutex<HashMap<String, LiveTerminal>>>,
    workflow_write_lock: Arc<Mutex<()>>,
    task_id: String,
    conversation_id: String,
    native_session_id: String,
    live_session_id: String,
    skip_existing_events: bool,
}

fn spawn_acp_watcher(watcher: AcpWatcher) {
    std::thread::spawn(move || {
        let started = Instant::now();
        let updates_path = loop {
            if !is_live_session(
                &watcher.sessions,
                &watcher.conversation_id,
                &watcher.live_session_id,
            ) {
                return;
            }
            if let Some(path) = find_grok_updates_file(&watcher.native_session_id) {
                break path;
            }
            if started.elapsed() >= ACP_DISCOVERY_TIMEOUT {
                set_observability_warning(&watcher, "Grok ACP session updates were not found.");
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        };

        let Ok(mut file) = OpenOptions::new().read(true).open(&updates_path) else {
            set_observability_warning(&watcher, "Grok ACP session updates could not be opened.");
            return;
        };
        if watcher.skip_existing_events {
            let _ = file.seek(SeekFrom::End(0));
        }
        let mut reader = BufReader::new(file);
        loop {
            if !is_live_session(
                &watcher.sessions,
                &watcher.conversation_id,
                &watcher.live_session_id,
            ) {
                return;
            }
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => std::thread::sleep(Duration::from_millis(120)),
                Ok(_) => {
                    if let Ok(event) = serde_json::from_str::<Value>(&line) {
                        handle_acp_event(&watcher, event);
                    }
                }
                Err(_) => {
                    set_observability_warning(
                        &watcher,
                        "Grok ACP session updates stopped unexpectedly.",
                    );
                    return;
                }
            }
        }
    });
}

fn handle_acp_event(watcher: &AcpWatcher, event: Value) {
    let Some(update) = event.pointer("/params/update") else {
        return;
    };
    let event_type = update
        .get("sessionUpdate")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let turn_id = event
        .pointer("/params/_meta/promptId")
        .or_else(|| update.get("prompt_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("grok-{}", watcher.native_session_id));
    let Ok(task) = watcher.tasks.get(&watcher.task_id) else {
        return;
    };
    let directory =
        conversation_directory(Path::new(&task.workspace_path), &watcher.conversation_id);
    let _workflow_guard = watcher.workflow_write_lock.lock();
    let Ok(mut workflow) = load_workflow(&directory, &watcher.task_id, &watcher.conversation_id)
    else {
        return;
    };
    workflow.observability = CodexObservabilityStatus::Native;
    workflow.warning.clear();

    let save_event = match event_type {
        "plan" => {
            let steps = update
                .get("entries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(parse_plan_entry)
                .collect::<Vec<_>>();
            workflow.update_plan(&turn_id, None, &steps);
            "planUpdated"
        }
        "tool_call" | "tool_call_update" => {
            let Some(activity_id) = update.get("toolCallId").and_then(Value::as_str) else {
                return;
            };
            let status = match update.get("status").and_then(Value::as_str) {
                Some("completed") => CodexActivityStatus::Completed,
                Some("failed") | Some("error") => CodexActivityStatus::Failed,
                Some("in_progress") | Some("inProgress") => CodexActivityStatus::Progress,
                _ => CodexActivityStatus::Started,
            };
            let existing = workflow
                .turns
                .iter()
                .find(|turn| turn.id == turn_id)
                .and_then(|turn| {
                    turn.activities
                        .iter()
                        .find(|activity| activity.id == activity_id)
                });
            let has_description = update.get("kind").is_some()
                || update.get("title").is_some()
                || update.get("rawInput").is_some();
            let (kind, summary, detail) = match (has_description, existing) {
                (false, Some(existing)) => (
                    existing.kind,
                    existing.summary.clone(),
                    existing.detail.clone(),
                ),
                _ => {
                    let (kind, summary) = describe_grok_tool(update);
                    (kind, summary, grok_tool_detail(update))
                }
            };
            workflow.record_activity(
                &turn_id,
                Some(activity_id),
                kind,
                status,
                &summary,
                &detail,
                "grokAcp",
            );
            "activityUpdated"
        }
        "turn_completed" => {
            let status = match update.get("stop_reason").and_then(Value::as_str) {
                Some("cancelled") => CodexWorkflowTurnStatus::Interrupted,
                Some("error") | Some("refusal") => CodexWorkflowTurnStatus::Failed,
                _ => CodexWorkflowTurnStatus::Completed,
            };
            workflow.complete_turn(&turn_id, status);
            "turnCompleted"
        }
        _ => return,
    };

    if save_workflow(&directory, &workflow, save_event).is_ok() {
        send_workflow_to_live_session(&watcher.sessions, &watcher.conversation_id, &workflow);
    }
}

fn parse_plan_entry(entry: &Value) -> Option<(String, CodexPlanStepStatus)> {
    let text = entry
        .get("content")
        .or_else(|| entry.get("step"))
        .or_else(|| entry.get("text"))
        .and_then(Value::as_str)?;
    let status = match entry.get("status").and_then(Value::as_str) {
        Some("in_progress") | Some("inProgress") => CodexPlanStepStatus::InProgress,
        Some("completed") => CodexPlanStepStatus::Completed,
        Some("failed") => CodexPlanStepStatus::Failed,
        _ => CodexPlanStepStatus::Pending,
    };
    Some((text.to_string(), status))
}

fn describe_grok_tool(update: &Value) -> (CodexActivityKind, String) {
    let tool_kind = update
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let title = update
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if title.contains("game instance") || title.contains("runtime mcp") {
        return (
            CodexActivityKind::GameInstance,
            "Using a game development tool".into(),
        );
    }
    if title.contains("mcp") || tool_kind.contains("mcp") {
        return (CodexActivityKind::Mcp, "Using an MCP tool".into());
    }
    if title.contains("test") || tool_kind.contains("test") {
        return (CodexActivityKind::Test, "Running tests".into());
    }
    if ["edit", "write", "delete", "move", "create"]
        .iter()
        .any(|needle| tool_kind.contains(needle) || title.contains(needle))
    {
        return (
            CodexActivityKind::FileChange,
            "Updating project files".into(),
        );
    }
    if ["read", "search", "list"]
        .iter()
        .any(|needle| tool_kind.contains(needle) || title.contains(needle))
    {
        return (
            CodexActivityKind::Command,
            "Inspecting the workspace".into(),
        );
    }
    if ["web", "fetch"]
        .iter()
        .any(|needle| tool_kind.contains(needle) || title.contains(needle))
    {
        return (CodexActivityKind::Web, "Researching information".into());
    }
    if tool_kind.contains("think") {
        return (CodexActivityKind::Analysis, "Analyzing the task".into());
    }
    (
        CodexActivityKind::Command,
        "Running a development tool".into(),
    )
}

fn grok_tool_detail(update: &Value) -> String {
    let raw_input = update.get("rawInput").unwrap_or(&Value::Null);
    let safe_input = match raw_input {
        Value::Object(values) => Value::Object(
            values
                .iter()
                .filter(|(key, _)| {
                    matches!(
                        key.as_str(),
                        "path"
                            | "paths"
                            | "query"
                            | "pattern"
                            | "file"
                            | "files"
                            | "tool"
                            | "server"
                            | "instanceId"
                            | "taskId"
                    )
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        _ => Value::Null,
    };
    sanitize_detail(&json!({
        "kind": update.get("kind"),
        "locations": update.get("locations"),
        "input": safe_input
    }))
}

fn set_observability_warning(watcher: &AcpWatcher, warning: &str) {
    let Ok(task) = watcher.tasks.get(&watcher.task_id) else {
        return;
    };
    let directory =
        conversation_directory(Path::new(&task.workspace_path), &watcher.conversation_id);
    let _workflow_guard = watcher.workflow_write_lock.lock();
    let Ok(mut workflow) = load_workflow(&directory, &watcher.task_id, &watcher.conversation_id)
    else {
        return;
    };
    workflow.observability = CodexObservabilityStatus::Compatibility;
    workflow.warning = warning.to_string();
    if save_workflow(&directory, &workflow, "observability").is_ok() {
        send_workflow_to_live_session(&watcher.sessions, &watcher.conversation_id, &workflow);
    }
}

fn is_live_session(
    sessions: &Arc<Mutex<HashMap<String, LiveTerminal>>>,
    conversation_id: &str,
    session_id: &str,
) -> bool {
    sessions
        .lock()
        .get(conversation_id)
        .is_some_and(|session| session.session_id == session_id)
}

fn send_workflow_to_live_session(
    sessions: &Arc<Mutex<HashMap<String, LiveTerminal>>>,
    conversation_id: &str,
    workflow: &CodexWorkflowSnapshot,
) {
    let subscriber = sessions
        .lock()
        .get(conversation_id)
        .and_then(|session| session.subscriber.lock().clone());
    if let Some(channel) = subscriber {
        let _ = channel.send(GrokTerminalEvent::Workflow {
            workflow: workflow.clone(),
        });
    }
}

fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    subscriber: Arc<Mutex<Option<Channel<GrokTerminalEvent>>>>,
    transcript: Arc<Mutex<File>>,
) {
    std::thread::spawn(move || {
        let mut decoder = Utf8ChunkDecoder::default();
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    if let Some(data) = decoder.finish() {
                        send_output(&subscriber, &transcript, data);
                    }
                    break;
                }
                Ok(count) => {
                    for data in decoder.push(&buffer[..count]) {
                        send_output(&subscriber, &transcript, data);
                    }
                }
                Err(error) => {
                    send_output(
                        &subscriber,
                        &transcript,
                        format!("\r\n[Grok terminal read error: {error}]\r\n"),
                    );
                    break;
                }
            }
        }
    });
}

fn send_output(
    subscriber: &Arc<Mutex<Option<Channel<GrokTerminalEvent>>>>,
    transcript: &Arc<Mutex<File>>,
    data: String,
) {
    {
        let mut file = transcript.lock();
        let _ = file.write_all(data.as_bytes());
        let _ = file.flush();
    }
    if let Some(channel) = subscriber.lock().clone() {
        let _ = channel.send(GrokTerminalEvent::Output { data });
    }
}

fn send_transcript(
    task_id: &str,
    conversation_id: &str,
    tasks: &TaskService,
    subscriber: &Channel<GrokTerminalEvent>,
) -> AppResult<()> {
    let task = tasks.get(task_id)?;
    let path = conversation_directory(Path::new(&task.workspace_path), conversation_id)
        .join(CONVERSATION_TRANSCRIPT_FILE);
    for data in read_transcript_replay(&path)? {
        let _ = subscriber.send(GrokTerminalEvent::Output { data });
    }
    Ok(())
}

fn build_command(
    grok: &ResolvedGrok,
    working_directory: &Path,
    task_id: &str,
    conversation_id: &str,
    native_session_id: &str,
    resume: bool,
) -> CommandBuilder {
    let mut command = match grok.kind {
        GrokLaunchKind::Executable => CommandBuilder::new(&grok.path),
        GrokLaunchKind::CommandScript => {
            let comspec = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
            let mut command = CommandBuilder::new(comspec);
            command.args(["/D", "/S", "/C", "call"]);
            command.arg(&grok.path);
            command
        }
    };
    if resume {
        command.arg("--resume");
        command.arg(native_session_id);
    } else {
        command.arg("--session-id");
        command.arg(native_session_id);
    }
    command.arg("--cwd");
    command.arg(working_directory);
    command.arg("--no-alt-screen");
    command.arg("--minimal");
    command.arg("--rules");
    command.arg(managed_developer_instructions(task_id, conversation_id));
    command.env(
        "ABYA_DESKTOP_CLI",
        crate::foundation::cli_environment::desktop_cli_path(),
    );
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("ABYA_DEVELOPMENT_PROVIDER", "grok");
    command.env("ABYA_DEVELOPMENT_TASK_ID", task_id);
    command.env("ABYA_DEVELOPMENT_CONVERSATION_ID", conversation_id);
    command.env("ABYA_DEVELOPMENT_WORKSPACE", working_directory);
    for (key, value) in crate::foundation::cli_sessions::environment("grok", task_id, conversation_id) { command.env(key, value); }
    command.cwd(working_directory);
    command
}

fn managed_developer_instructions(task_id: &str, conversation_id: &str) -> String {
    format!(
        "You are working in ABYA task {task_id}, conversation {conversation_id}. Publish and maintain a plan for multi-step work. Use only the executable in ABYA_DESKTOP_CLI for ABYA operations. Run doctor and capabilities first. CLI commands inherit the task, provider and conversation context. Report semantic milestones with conversation report. Do not use MCP or start unmanaged game processes. Open returned screenshot paths for visual acceptance. Never include credentials, raw commands, outputs or patches in activity details."
    )
}

fn describe_desktop_tool(tool_name: &str) -> (CodexActivityKind, String) {
    let kind = if tool_name.contains("test") {
        CodexActivityKind::Test
    } else if tool_name.starts_with("game_") {
        CodexActivityKind::GameInstance
    } else {
        CodexActivityKind::Command
    };
    let summary = match tool_name {
        "game_instance_launch" => "Starting a game instance",
        "game_instance_wait_for_state" => "Waiting for the game process",
        "game_instance_wait_for_cli" => "Waiting for the game Runtime CLI",
        "game_instance_set_window_visibility" => "Updating the game window",
        "game_instance_stop" => "Stopping a game instance",
        "game_runtime_call_tool" => "Using a game runtime tool",
        "game_runtime_list_tools" => "Inspecting game runtime capabilities",
        "game_log_query" => "Inspecting game logs",
        other => return (kind, format!("Using {other}")),
    };
    (kind, summary.into())
}

fn conversations_root(workspace: &Path) -> PathBuf {
    workspace.join(CONVERSATIONS_DIRECTORY)
}

fn conversation_directory(workspace: &Path, conversation_id: &str) -> PathBuf {
    conversations_root(workspace).join(conversation_id)
}

fn write_conversation(directory: &Path, conversation: &GrokConversation) -> AppResult<()> {
    std::fs::write(
        directory.join(CONVERSATION_METADATA_FILE),
        serde_json::to_string_pretty(conversation)?,
    )?;
    Ok(())
}

fn update_state(
    state: &Arc<Mutex<GrokTerminalState>>,
    subscriber: &Arc<Mutex<Option<Channel<GrokTerminalEvent>>>>,
    status: GrokTerminalStatus,
    exit_code: Option<u32>,
    last_error: &str,
) {
    let snapshot = {
        let mut value = state.lock();
        value.status = status;
        value.exit_code = exit_code;
        value.last_error = last_error.to_string();
        value.clone()
    };
    if let Some(channel) = subscriber.lock().clone() {
        let _ = channel.send(GrokTerminalEvent::State { state: snapshot });
    }
}

fn validate_size(columns: u16, rows: u16) -> AppResult<()> {
    if !(MIN_COLUMNS..=MAX_COLUMNS).contains(&columns) || !(MIN_ROWS..=MAX_ROWS).contains(&rows) {
        return Err(AppError::validation(
            "Terminal size is outside the supported range.",
        ));
    }
    Ok(())
}

fn pty_size(columns: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols: columns,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn discover_working_directory() -> AppResult<PathBuf> {
    let current = std::env::current_dir().map_err(AppError::internal)?;
    #[cfg(debug_assertions)]
    if current.file_name() == Some(OsStr::new("src-tauri"))
        && let Some(root) = current.parent()
        && root.join("package.json").is_file()
        && root.join("module-registry.json").is_file()
    {
        return Ok(root.to_path_buf());
    }
    Ok(current)
}

fn resolve_grok() -> Option<ResolvedGrok> {
    let mut directories = path_directories(std::env::var_os("PATH").as_deref());
    #[cfg(windows)]
    {
        directories.extend(windows_persisted_path_directories());
        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            directories.push(PathBuf::from(user_profile).join(".grok").join("bin"));
        }
    }
    resolve_grok_in_directories(directories, std::env::var_os("PATHEXT").as_deref())
}

#[cfg(test)]
fn resolve_grok_in(path: Option<&OsStr>, path_ext: Option<&OsStr>) -> Option<ResolvedGrok> {
    resolve_grok_in_directories(path_directories(path), path_ext)
}

fn resolve_grok_in_directories(
    directories: impl IntoIterator<Item = PathBuf>,
    path_ext: Option<&OsStr>,
) -> Option<ResolvedGrok> {
    let mut extensions = path_ext
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .filter_map(|value| value.to_str().map(str::to_ascii_lowercase))
        .collect::<Vec<_>>();
    if extensions.is_empty() {
        extensions = vec![".exe".into(), ".com".into(), ".cmd".into(), ".bat".into()];
    }
    extensions.sort_by_key(|extension| match extension.as_str() {
        ".exe" => 0,
        ".com" => 1,
        ".cmd" => 2,
        ".bat" => 3,
        _ => 4,
    });
    let mut searched = Vec::<PathBuf>::new();
    for directory in directories {
        if searched
            .iter()
            .any(|existing| paths_equal(existing, &directory))
        {
            continue;
        }
        searched.push(directory.clone());
        for extension in &extensions {
            let candidate = directory.join(format!("grok{extension}"));
            if !candidate.is_file() {
                continue;
            }
            let kind = match extension.as_str() {
                ".cmd" | ".bat" => GrokLaunchKind::CommandScript,
                ".exe" | ".com" => GrokLaunchKind::Executable,
                _ => continue,
            };
            return Some(ResolvedGrok {
                path: candidate,
                kind,
            });
        }
    }
    None
}

fn probe_grok_observability(grok: &ResolvedGrok) -> bool {
    let mut command = match grok.kind {
        GrokLaunchKind::Executable => Command::new(&grok.path),
        GrokLaunchKind::CommandScript => {
            let mut command =
                Command::new(std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into()));
            command.args(["/D", "/S", "/C", "call"]);
            command.arg(&grok.path);
            command
        }
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .arg("--help")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).contains("streaming-json"))
        .unwrap_or(false)
}

fn grok_home() -> Option<PathBuf> {
    std::env::var_os("GROK_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .map(|path| path.join(".grok"))
        })
}

fn find_grok_updates_file(session_id: &str) -> Option<PathBuf> {
    let updates = find_grok_session_directory(session_id)?.join("updates.jsonl");
    updates.is_file().then_some(updates)
}

fn find_grok_session_directory(session_id: &str) -> Option<PathBuf> {
    let root = grok_home()?.join("sessions");
    let mut directories = vec![root];
    while let Some(directory) = directories.pop() {
        let entries = std::fs::read_dir(directory).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if entry.file_name() == OsStr::new(session_id) {
                    return Some(path);
                } else {
                    directories.push(path);
                }
            }
        }
    }
    None
}

fn path_directories(path: Option<&OsStr>) -> Vec<PathBuf> {
    path.map(std::env::split_paths)
        .into_iter()
        .flatten()
        .collect()
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(windows)]
fn windows_persisted_path_directories() -> Vec<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    let keys = [
        (RegKey::predef(HKEY_CURRENT_USER), "Environment"),
        (
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ),
    ];
    let mut directories = Vec::new();
    for (root, subkey) in keys {
        let Ok(key) = root.open_subkey(subkey) else {
            continue;
        };
        let Ok(path) = key.get_value::<String, _>("Path") else {
            continue;
        };
        directories.extend(std::env::split_paths(OsStr::new(&path)));
    }
    directories
}

fn disconnect_and_terminate(session: LiveTerminal) -> AppResult<()> {
    // Drop the UI channel before killing so PTY output cannot block on webview IPC.
    let subscriber = session.subscriber.lock().take();
    let pid = {
        let mut state = session.state.lock();
        state.status = GrokTerminalStatus::Stopping;
        state.pid
    };
    let killer = session.killer.clone();
    drop(session);
    let result = terminate_process_tree(pid, &killer);
    drop(subscriber);
    result
}

fn terminate_process_tree(
    pid: Option<u32>,
    killer: &Arc<Mutex<Box<dyn ChildKiller + Send + Sync>>>,
) -> AppResult<()> {
    #[cfg(windows)]
    if let Some(pid) = pid {
        use std::os::windows::process::CommandExt;

        if let Ok(mut child) = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            let deadline = Instant::now() + PROCESS_TREE_STOP_TIMEOUT;
            loop {
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => return Ok(()),
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
    }
    killer.lock().kill().map_err(AppError::internal)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[derive(Default)]
struct Utf8ChunkDecoder {
    pending: Vec<u8>,
}

impl Utf8ChunkDecoder {
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(bytes);
        let mut output = Vec::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(value) => {
                    if !value.is_empty() {
                        output.push(value.to_string());
                    }
                    self.pending.clear();
                    break;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    if valid > 0 {
                        output.push(String::from_utf8_lossy(&self.pending[..valid]).into_owned());
                        self.pending.drain(..valid);
                    }
                    match error.error_len() {
                        Some(length) => {
                            output.push(
                                String::from_utf8_lossy(&self.pending[..length]).into_owned(),
                            );
                            self.pending.drain(..length);
                        }
                        None => break,
                    }
                }
            }
        }
        output
    }

    fn finish(&mut self) -> Option<String> {
        (!self.pending.is_empty()).then(|| {
            let value = String::from_utf8_lossy(&self.pending).into_owned();
            self.pending.clear();
            value
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{AppPaths, Database};

    fn task_service(root: &Path) -> TaskService {
        std::fs::create_dir_all(root).unwrap();
        let paths = AppPaths {
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
            data_dir: root.to_path_buf(),
        };
        let database = Database::open(&paths).unwrap();
        TaskService::new(database, paths.default_workspace_root)
    }

    #[test]
    fn stop_task_does_not_send_ui_events_while_killing_the_process() {
        use crate::modules::tasks::TaskInput;
        use std::sync::atomic::{AtomicBool, Ordering};

        let root = std::env::temp_dir().join(format!("abya-grok-stop-task-{}", Uuid::new_v4()));
        let tasks = task_service(&root);
        let task = tasks
            .create(TaskInput {
                title: "Stop task".into(),
                description: String::new(),
            })
            .unwrap();
        let service = GrokTerminalService::for_test(
            tasks,
            ResolvedGrok {
                path: root.join("grok.exe"),
                kind: GrokLaunchKind::Executable,
            },
        );
        let sent = Arc::new(AtomicBool::new(false));
        insert_live_session(
            &service,
            &task.id,
            "conversation-live",
            sent.clone(),
            &root.join("transcript.log"),
        );

        let started = Instant::now();
        service.stop_task(&task.id);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(!sent.load(Ordering::SeqCst));
        assert!(service.sessions.lock().is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    fn insert_live_session(
        service: &GrokTerminalService,
        task_id: &str,
        conversation_id: &str,
        sent: Arc<std::sync::atomic::AtomicBool>,
        transcript: &Path,
    ) {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let writer = pair.master.take_writer().unwrap();
        let mut command = CommandBuilder::new("cmd.exe");
        command.args(["/C", "ping", "-n", "20", "127.0.0.1"]);
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let transcript = OpenOptions::new()
            .create(true)
            .append(true)
            .open(transcript)
            .unwrap();
        let channel = Channel::new(move |_| {
            sent.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
        service.sessions.lock().insert(
            conversation_id.to_string(),
            LiveTerminal {
                task_id: task_id.to_string(),
                session_id: Uuid::new_v4().to_string(),
                state: Arc::new(Mutex::new(GrokTerminalState {
                    task_id: task_id.to_string(),
                    conversation_id: conversation_id.to_string(),
                    status: GrokTerminalStatus::Running,
                    pid: child.process_id(),
                    working_directory: String::new(),
                    exit_code: None,
                    last_error: String::new(),
                })),
                master: Arc::new(Mutex::new(pair.master)),
                writer: Arc::new(Mutex::new(writer)),
                transcript: Arc::new(Mutex::new(transcript)),
                killer: Arc::new(Mutex::new(child.clone_killer())),
                subscriber: Arc::new(Mutex::new(Some(channel))),
            },
        );
        std::mem::forget(child);
    }

    #[test]
    fn resolves_native_grok_before_command_scripts() {
        let root = std::env::temp_dir().join(format!("abya-grok-resolve-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("grok.cmd"), "@echo off").unwrap();
        std::fs::write(root.join("grok.exe"), "binary").unwrap();
        let resolved =
            resolve_grok_in(Some(root.as_os_str()), Some(OsStr::new(".CMD;.EXE"))).unwrap();
        assert_eq!(resolved.path, root.join("grok.exe"));
        assert_eq!(resolved.kind, GrokLaunchKind::Executable);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn builds_new_and_resume_commands_with_fixed_workspace() {
        let root = PathBuf::from(r"D:\Workspaces\Task");
        let grok = ResolvedGrok {
            path: PathBuf::from(r"C:\Users\User\.grok\bin\grok.exe"),
            kind: GrokLaunchKind::Executable,
        };
        let new_command = build_command(&grok, &root, "task", "conversation", "session", false);
        let new_debug = format!("{new_command:?}");
        assert!(new_debug.contains("--session-id"));
        assert!(new_debug.contains("--minimal"));
        assert!(new_debug.contains("ABYA_DEVELOPMENT_PROVIDER"));

        let resumed = build_command(&grok, &root, "task", "conversation", "session", true);
        let resumed_debug = format!("{resumed:?}");
        assert!(resumed_debug.contains("--resume"));
        assert!(!resumed_debug.contains("--session-id"));
    }

    #[test]
    fn maps_grok_plan_and_tool_updates() {
        let root = std::env::temp_dir().join(format!("abya-grok-workflow-{}", Uuid::new_v4()));
        let tasks = task_service(&root);
        let task = tasks
            .create(crate::modules::tasks::TaskInput {
                title: "Grok".into(),
                description: String::new(),
            })
            .unwrap();
        let service = GrokTerminalService::for_test(
            tasks.clone(),
            ResolvedGrok {
                path: PathBuf::from("grok.exe"),
                kind: GrokLaunchKind::Executable,
            },
        );
        let conversation = service.create_conversation(&task.id, None).unwrap();
        let watcher = AcpWatcher {
            tasks,
            sessions: Default::default(),
            workflow_write_lock: service.workflow_write_lock.clone(),
            task_id: task.id.clone(),
            conversation_id: conversation.id.clone(),
            native_session_id: conversation.native_session_id,
            live_session_id: "live".into(),
            skip_existing_events: false,
        };
        handle_acp_event(
            &watcher,
            json!({
                "params": {
                    "_meta": { "promptId": "turn-1" },
                    "update": {
                        "sessionUpdate": "plan",
                        "entries": [
                            { "content": "Inspect", "status": "in_progress" },
                            { "content": "Test", "status": "pending" }
                        ]
                    }
                }
            }),
        );
        handle_acp_event(
            &watcher,
            json!({
                "params": {
                    "_meta": { "promptId": "turn-1" },
                    "update": {
                        "sessionUpdate": "tool_call",
                        "toolCallId": "tool-1",
                        "kind": "read",
                        "title": "Read files",
                        "rawInput": { "path": "src/main.rs", "command": "secret" }
                    }
                }
            }),
        );
        let workflow = service.workflow(&task.id, &conversation.id).unwrap();
        assert_eq!(workflow.turns[0].plan.len(), 2);
        assert_eq!(workflow.turns[0].activities.len(), 1);
        assert!(!workflow.turns[0].activities[0].detail.contains("secret"));
        let _ = std::fs::remove_dir_all(root);
    }
}
