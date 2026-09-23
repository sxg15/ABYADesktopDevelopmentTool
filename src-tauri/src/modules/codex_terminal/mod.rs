mod app_server;

use crate::foundation::{AppError, AppResult};
use crate::modules::development_terminal::transcript::read_transcript_replay;
pub use crate::modules::development_terminal::workflow::{
    CodexActivityKind, CodexActivityStatus, CodexObservabilityStatus, CodexPlanStepStatus,
    CodexWorkflowSnapshot, CodexWorkflowTurnStatus,
};
use crate::modules::development_terminal::workflow::{
    load_workflow, sanitize_detail, save_workflow, validate_conversation_title,
};
use crate::modules::tasks::TaskService;
use app_server::{AppServerHandle, supports_app_server};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
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
const CONVERSATIONS_DIRECTORY: &str = "conversations";
const CONVERSATION_METADATA_FILE: &str = "conversation.json";
const CONVERSATION_TRANSCRIPT_FILE: &str = "transcript.log";
const NATIVE_SESSION_MATCH_SECONDS: i64 = 10 * 60;
const NATIVE_SESSION_DISCOVERY_ATTEMPTS: usize = 20;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const PROCESS_TREE_STOP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTerminalAvailability {
    pub available: bool,
    pub working_directory: String,
    pub reason: String,
    pub observability_available: bool,
    pub observability_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexTerminalStatus {
    Starting,
    Running,
    Stopping,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexTerminalState {
    pub task_id: String,
    pub conversation_id: String,
    pub status: CodexTerminalStatus,
    pub pid: Option<u32>,
    pub working_directory: String,
    pub exit_code: Option<u32>,
    pub last_error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CodexTerminalEvent {
    Output { data: String },
    State { state: CodexTerminalState },
    Workflow { workflow: CodexWorkflowSnapshot },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversation {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_session_id: Option<String>,
}

#[derive(Clone)]
struct LiveTerminal {
    task_id: String,
    session_id: String,
    state: Arc<Mutex<CodexTerminalState>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    transcript: Arc<Mutex<File>>,
    killer: Arc<Mutex<Box<dyn ChildKiller + Send + Sync>>>,
    subscriber: Arc<Mutex<Option<Channel<CodexTerminalEvent>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexLaunchKind {
    Executable,
    CommandScript,
}

#[derive(Debug, Clone)]
struct ResolvedCodex {
    path: PathBuf,
    kind: CodexLaunchKind,
}

type CodexResolver = Arc<dyn Fn() -> Option<ResolvedCodex> + Send + Sync>;

#[derive(Clone)]
pub struct CodexTerminalService {
    tasks: TaskService,
    sessions: Arc<Mutex<HashMap<String, LiveTerminal>>>,
    resolver: CodexResolver,
    app_server: Arc<Mutex<Option<AppServerHandle>>>,
    thread_conversations: Arc<Mutex<HashMap<String, (String, String)>>>,
    workflow_write_lock: Arc<Mutex<()>>,
}

impl CodexTerminalService {
    pub fn new(tasks: TaskService) -> AppResult<Self> {
        Ok(Self {
            tasks,
            sessions: Default::default(),
            resolver: Arc::new(resolve_codex),
            app_server: Default::default(),
            thread_conversations: Default::default(),
            workflow_write_lock: Default::default(),
        })
    }

    pub fn availability(&self) -> CodexTerminalAvailability {
        let codex = (self.resolver)();
        let available = codex.is_some();
        let observability_available = codex.as_ref().is_some_and(supports_app_server);
        CodexTerminalAvailability {
            available,
            working_directory: display_path(&discover_working_directory().unwrap_or_default()),
            reason: if available {
                String::new()
            } else {
                "Codex CLI was not found in PATH.".into()
            },
            observability_available,
            observability_reason: if observability_available {
                String::new()
            } else if available {
                "This Codex version does not provide the native app-server event stream.".into()
            } else {
                "Codex CLI is unavailable.".into()
            },
        }
    }

    pub fn list_conversations(&self, task_id: &str) -> AppResult<Vec<CodexConversation>> {
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
            let conversation: CodexConversation =
                serde_json::from_str(&std::fs::read_to_string(metadata)?)?;
            if conversation.task_id == task_id {
                conversations.push(conversation);
            }
        }
        conversations.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        backfill_native_session_ids(Path::new(&task.workspace_path), &mut conversations)?;
        Ok(conversations)
    }

    pub fn create_conversation(
        &self,
        task_id: &str,
        title: Option<String>,
    ) -> AppResult<CodexConversation> {
        let task = self.tasks.get(task_id)?;
        let root = conversations_root(Path::new(&task.workspace_path));
        std::fs::create_dir_all(&root)?;
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let title = title.unwrap_or_default();
        let title = if title.trim().is_empty() {
            format!(
                "Conversation {}",
                self.list_conversations(task_id)?.len() + 1
            )
        } else {
            validate_conversation_title(&title)?
        };
        let conversation = CodexConversation {
            id: id.clone(),
            task_id: task_id.to_string(),
            title,
            created_at: now.clone(),
            updated_at: now,
            native_session_id: None,
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
    ) -> AppResult<CodexConversation> {
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
    ) -> AppResult<CodexConversation> {
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
            "desktopMcpReport",
        )
    }

    pub fn record_desktop_tool_activity(
        &self,
        task_id: &str,
        conversation_id: &str,
        activity_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
        status: CodexActivityStatus,
    ) -> AppResult<CodexWorkflowSnapshot> {
        let workflow = self.workflow(task_id, conversation_id)?;
        let turn_id = workflow
            .current_turn_id
            .unwrap_or_else(|| format!("desktop-mcp-{}", Uuid::new_v4()));
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
            "desktopMcp",
        )
    }

    pub fn delete_conversation(&self, task_id: &str, conversation_id: &str) -> AppResult<()> {
        let conversation = self.conversation(task_id, conversation_id)?;
        self.stop(conversation_id)?;
        self.thread_conversations
            .lock()
            .retain(|_, binding| binding != &(task_id.to_string(), conversation_id.to_string()));
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
        subscriber: Channel<CodexTerminalEvent>,
    ) -> AppResult<CodexTerminalState> {
        validate_size(columns, rows)?;
        if let Some(existing) = self.sessions.lock().get(conversation_id).cloned() {
            // Keep live output behind the transcript snapshot, then atomically
            // attach the new subscriber so replay and live bytes cannot race.
            let transcript_guard = existing.transcript.lock();
            send_transcript(&existing.task_id, conversation_id, &self.tasks, &subscriber)?;
            *existing.subscriber.lock() = Some(subscriber.clone());
            if let Ok(workflow) = self.workflow(&existing.task_id, conversation_id) {
                let _ = subscriber.send(CodexTerminalEvent::Workflow { workflow });
            }
            let state = existing.state.lock().clone();
            let _ = subscriber.send(CodexTerminalEvent::State {
                state: state.clone(),
            });
            drop(transcript_guard);
            return Ok(state);
        }

        let (codex, task) = self.validate_open_request(task_id, conversation_id)?;
        let working_directory = PathBuf::from(&task.workspace_path);
        let mut conversation = self.conversation(task_id, conversation_id)?;
        let mut native_session_id = conversation
            .native_session_id
            .clone()
            .or_else(|| find_native_session_id(&working_directory, &conversation.created_at));
        if conversation.native_session_id.is_none()
            && let Some(session_id) = native_session_id.as_deref()
        {
            conversation.native_session_id = Some(session_id.to_string());
            conversation.updated_at = Utc::now().to_rfc3339();
            write_conversation(
                &conversation_directory(&working_directory, conversation_id),
                &conversation,
            )?;
        }
        let mut remote = None;
        let observability = if supports_app_server(&codex) {
            match self.ensure_app_server(&codex) {
                Ok(server) => {
                    let instructions = managed_developer_instructions(task_id, conversation_id);
                    let prepared = if let Some(thread_id) = native_session_id.as_deref() {
                        server
                            .resume_thread(
                                thread_id,
                                &display_path(&working_directory),
                                &instructions,
                            )
                            .map(|()| thread_id.to_string())
                    } else {
                        server
                            .start_thread(&display_path(&working_directory), &instructions)
                            .and_then(|thread_id| {
                                server
                                    .persist_developer_instructions(&thread_id, &instructions)
                                    .map(|()| thread_id)
                            })
                    };
                    match prepared {
                        Ok(thread_id) => {
                            if conversation.native_session_id.as_deref() != Some(&thread_id) {
                                conversation.native_session_id = Some(thread_id.clone());
                                conversation.updated_at = Utc::now().to_rfc3339();
                                write_conversation(
                                    &conversation_directory(&working_directory, conversation_id),
                                    &conversation,
                                )?;
                            }
                            native_session_id = Some(thread_id.clone());
                            self.thread_conversations.lock().insert(
                                thread_id,
                                (task_id.to_string(), conversation_id.to_string()),
                            );
                            remote = Some(server);
                            (CodexObservabilityStatus::Native, String::new())
                        }
                        Err(error) => (
                            CodexObservabilityStatus::Compatibility,
                            format!(
                                "Native Codex activity stream is unavailable: {}",
                                error.message
                            ),
                        ),
                    }
                }
                Err(error) => (
                    CodexObservabilityStatus::Compatibility,
                    format!(
                        "Native Codex activity stream is unavailable: {}",
                        error.message
                    ),
                ),
            }
        } else {
            (
                CodexObservabilityStatus::Compatibility,
                "This Codex version does not provide the native app-server event stream.".into(),
            )
        };
        let conversation_directory = conversation_directory(&working_directory, conversation_id);
        std::fs::create_dir_all(&conversation_directory)?;
        let _workflow_guard = self.workflow_write_lock.lock();
        let mut workflow = load_workflow(&conversation_directory, task_id, conversation_id)?;
        workflow.observability = observability.0;
        workflow.warning = observability.1;
        save_workflow(&conversation_directory, &workflow, "observability")?;
        let transcript = OpenOptions::new()
            .create(true)
            .append(true)
            .open(conversation_directory.join(CONVERSATION_TRANSCRIPT_FILE))?;
        let starting = CodexTerminalState {
            task_id: task_id.to_string(),
            conversation_id: conversation_id.to_string(),
            status: CodexTerminalStatus::Starting,
            pid: None,
            working_directory: display_path(&working_directory),
            exit_code: None,
            last_error: String::new(),
        };
        let _ = subscriber.send(CodexTerminalEvent::State { state: starting });
        let _ = subscriber.send(CodexTerminalEvent::Workflow {
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
            &codex,
            &working_directory,
            task_id,
            conversation_id,
            native_session_id.as_deref(),
            remote.as_ref(),
        );
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(AppError::internal)?;
        drop(pair.slave);

        let pid = child.process_id();
        let killer = child.clone_killer();
        let state = CodexTerminalState {
            task_id: task_id.to_string(),
            conversation_id: conversation_id.to_string(),
            status: CodexTerminalStatus::Running,
            pid,
            working_directory: display_path(&working_directory),
            exit_code: None,
            last_error: String::new(),
        };
        let state_ref = Arc::new(Mutex::new(state.clone()));
        let subscriber_ref = Arc::new(Mutex::new(Some(subscriber.clone())));
        let session_id = Uuid::new_v4().to_string();
        let live = LiveTerminal {
            task_id: task_id.to_string(),
            session_id: session_id.clone(),
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
        let _ = subscriber.send(CodexTerminalEvent::State {
            state: state.clone(),
        });

        spawn_reader(reader, subscriber_ref.clone(), transcript_ref);
        self.spawn_monitor(
            conversation_id.to_string(),
            session_id,
            child,
            state_ref,
            subscriber_ref,
        );
        if native_session_id.is_none() {
            spawn_native_session_discovery(
                self.tasks.clone(),
                task_id.to_string(),
                conversation_id.to_string(),
                working_directory,
                conversation.created_at,
            );
        }
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
        if let Some(server) = self.app_server.lock().take() {
            server.stop();
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

    fn session(&self, conversation_id: &str) -> AppResult<LiveTerminal> {
        self.sessions
            .lock()
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| AppError::validation("The Codex terminal is not running."))
    }

    fn validate_open_request(
        &self,
        task_id: &str,
        conversation_id: &str,
    ) -> AppResult<(ResolvedCodex, crate::modules::tasks::DevelopmentTask)> {
        let task = self.tasks.get(task_id)?;
        self.conversation(task_id, conversation_id)?;
        let codex = (self.resolver)().ok_or_else(|| {
            AppError::validation("Codex CLI is not installed or is unavailable in PATH.")
        })?;
        Ok((codex, task))
    }

    fn ensure_app_server(&self, codex: &ResolvedCodex) -> AppResult<AppServerHandle> {
        if let Some(server) = self.app_server.lock().clone() {
            return Ok(server);
        }
        let tasks = self.tasks.clone();
        let sessions = self.sessions.clone();
        let thread_conversations = self.thread_conversations.clone();
        let workflow_write_lock = self.workflow_write_lock.clone();
        let server = AppServerHandle::start(
            codex,
            Arc::new(move |notification| {
                handle_app_server_notification(
                    &tasks,
                    &sessions,
                    &thread_conversations,
                    &workflow_write_lock,
                    notification,
                );
            }),
        )?;
        *self.app_server.lock() = Some(server.clone());
        Ok(server)
    }

    fn conversation(&self, task_id: &str, conversation_id: &str) -> AppResult<CodexConversation> {
        let task = self.tasks.get(task_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        if !directory.is_dir() {
            return Err(AppError::not_found("Codex conversation"));
        }
        let conversation: CodexConversation = serde_json::from_str(&std::fs::read_to_string(
            directory.join(CONVERSATION_METADATA_FILE),
        )?)?;
        if conversation.task_id != task_id || conversation.id != conversation_id {
            return Err(AppError::not_found("Codex conversation"));
        }
        Ok(conversation)
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
        state: Arc<Mutex<CodexTerminalState>>,
        subscriber: Arc<Mutex<Option<Channel<CodexTerminalEvent>>>>,
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
            }
            match result {
                Ok(status) => update_state(
                    &state,
                    &subscriber,
                    CodexTerminalStatus::Exited,
                    Some(status.exit_code()),
                    "",
                ),
                Err(error) => update_state(
                    &state,
                    &subscriber,
                    CodexTerminalStatus::Failed,
                    None,
                    &error.to_string(),
                ),
            }
        });
    }

    #[cfg(test)]
    fn for_test(tasks: TaskService, _working_directory: PathBuf, codex: ResolvedCodex) -> Self {
        Self {
            tasks,
            sessions: Default::default(),
            resolver: Arc::new(move || Some(codex.clone())),
            app_server: Default::default(),
            thread_conversations: Default::default(),
            workflow_write_lock: Default::default(),
        }
    }
}

fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    subscriber: Arc<Mutex<Option<Channel<CodexTerminalEvent>>>>,
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
                        format!("\r\n[Codex terminal read error: {error}]\r\n"),
                    );
                    break;
                }
            }
        }
    });
}

fn send_output(
    subscriber: &Arc<Mutex<Option<Channel<CodexTerminalEvent>>>>,
    transcript: &Arc<Mutex<File>>,
    data: String,
) {
    {
        let mut file = transcript.lock();
        let _ = file.write_all(data.as_bytes());
        let _ = file.flush();
    }
    if let Some(channel) = subscriber.lock().clone() {
        let _ = channel.send(CodexTerminalEvent::Output { data });
    }
}

fn send_transcript(
    task_id: &str,
    conversation_id: &str,
    tasks: &TaskService,
    subscriber: &Channel<CodexTerminalEvent>,
) -> AppResult<()> {
    let task = tasks.get(task_id)?;
    let path = conversation_directory(Path::new(&task.workspace_path), conversation_id)
        .join(CONVERSATION_TRANSCRIPT_FILE);
    for data in read_transcript_replay(&path)? {
        let _ = subscriber.send(CodexTerminalEvent::Output { data });
    }
    Ok(())
}

fn conversations_root(workspace: &Path) -> PathBuf {
    workspace.join(CONVERSATIONS_DIRECTORY)
}

fn conversation_directory(workspace: &Path, conversation_id: &str) -> PathBuf {
    conversations_root(workspace).join(conversation_id)
}

fn write_conversation(directory: &Path, conversation: &CodexConversation) -> AppResult<()> {
    let json = serde_json::to_string_pretty(conversation)?;
    std::fs::write(directory.join(CONVERSATION_METADATA_FILE), json)?;
    Ok(())
}

fn update_state(
    state: &Arc<Mutex<CodexTerminalState>>,
    subscriber: &Arc<Mutex<Option<Channel<CodexTerminalEvent>>>>,
    status: CodexTerminalStatus,
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
        let _ = channel.send(CodexTerminalEvent::State { state: snapshot });
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

fn build_command(
    codex: &ResolvedCodex,
    working_directory: &Path,
    task_id: &str,
    conversation_id: &str,
    native_session_id: Option<&str>,
    remote: Option<&AppServerHandle>,
) -> CommandBuilder {
    let mut command = match codex.kind {
        CodexLaunchKind::Executable => CommandBuilder::new(&codex.path),
        CodexLaunchKind::CommandScript => {
            let comspec = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
            let mut command = CommandBuilder::new(comspec);
            command.args(["/D", "/S", "/C", "call"]);
            command.arg(&codex.path);
            command
        }
    };
    if let Some(session_id) = native_session_id {
        command.arg("resume");
        command.arg(session_id);
    }
    if let Some(server) = remote {
        command.arg("--remote");
        command.arg(&server.endpoint);
        command.arg("--remote-auth-token-env");
        command.arg("ABYA_CODEX_APP_SERVER_TOKEN");
        command.env("ABYA_CODEX_APP_SERVER_TOKEN", &server.token);
    }
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("ABYA_DEVELOPMENT_PROVIDER", "codex");
    command.env("ABYA_DEVELOPMENT_TASK_ID", task_id);
    command.env("ABYA_DEVELOPMENT_CONVERSATION_ID", conversation_id);
    command.env("ABYA_DEVELOPMENT_WORKSPACE", working_directory);
    command.arg("--cd");
    command.arg(working_directory);
    command.arg("--no-alt-screen");
    command.cwd(working_directory);
    command
}

fn managed_developer_instructions(task_id: &str, conversation_id: &str) -> String {
    let current_date = chrono::Local::now().format("%Y-%m-%d");
    format!(
        "You are working inside ABYA development task {task_id}, conversation \
         {conversation_id}. The current local date is {current_date}. For every new user \
         request that requires more than one operation, call update_plan before \
         any command, file edit, web call, MCP call, or game-instance action. Keep \
         exactly one plan step in progress, update the plan as work advances, and \
         complete or fail every step before the final response. If the ABYA desktop \
         MCP is used, first call \
         development_conversation_bind with provider codex, this task, and this conversation, then use \
         development_conversation_activity_report for meaningful design, editing, \
         testing, and blocker milestones. Do not include credentials or secrets in \
         activity details."
    )
}

fn handle_app_server_notification(
    tasks: &TaskService,
    sessions: &Arc<Mutex<HashMap<String, LiveTerminal>>>,
    thread_conversations: &Arc<Mutex<HashMap<String, (String, String)>>>,
    workflow_write_lock: &Arc<Mutex<()>>,
    notification: serde_json::Value,
) {
    let method = notification
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let params = notification
        .get("params")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let Some(thread_id) = params.get("threadId").and_then(serde_json::Value::as_str) else {
        return;
    };
    let Some((task_id, conversation_id)) = thread_conversations.lock().get(thread_id).cloned()
    else {
        return;
    };
    let Ok(task) = tasks.get(&task_id) else {
        return;
    };
    let _workflow_guard = workflow_write_lock.lock();
    let directory = conversation_directory(Path::new(&task.workspace_path), &conversation_id);
    let Ok(mut workflow) = load_workflow(&directory, &task_id, &conversation_id) else {
        return;
    };
    workflow.observability = CodexObservabilityStatus::Native;
    workflow.warning.clear();

    let event_type = match method {
        "turn/started" => {
            let Some(turn_id) = params
                .pointer("/turn/id")
                .and_then(serde_json::Value::as_str)
            else {
                return;
            };
            workflow.begin_turn(turn_id);
            "turnStarted"
        }
        "turn/plan/updated" => {
            let Some(turn_id) = params.get("turnId").and_then(serde_json::Value::as_str) else {
                return;
            };
            let steps = params
                .get("plan")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|step| {
                    Some((
                        step.get("step")?.as_str()?.to_string(),
                        match step.get("status")?.as_str()? {
                            "inProgress" => CodexPlanStepStatus::InProgress,
                            "completed" => CodexPlanStepStatus::Completed,
                            _ => CodexPlanStepStatus::Pending,
                        },
                    ))
                })
                .collect::<Vec<_>>();
            workflow.update_plan(
                turn_id,
                params
                    .get("explanation")
                    .and_then(serde_json::Value::as_str),
                &steps,
            );
            "planUpdated"
        }
        "item/started" | "item/completed" => {
            let Some(turn_id) = params.get("turnId").and_then(serde_json::Value::as_str) else {
                return;
            };
            let Some(item) = params.get("item") else {
                return;
            };
            let Some((activity_id, kind, summary, detail)) = describe_thread_item(item) else {
                return;
            };
            let status = if method == "item/started" {
                CodexActivityStatus::Started
            } else if item.get("status").and_then(serde_json::Value::as_str) == Some("failed") {
                CodexActivityStatus::Failed
            } else {
                CodexActivityStatus::Completed
            };
            workflow.record_activity(
                turn_id,
                Some(&activity_id),
                kind,
                status,
                &summary,
                &detail,
                "codexNative",
            );
            if method == "item/started" {
                "activityStarted"
            } else {
                "activityCompleted"
            }
        }
        "turn/completed" => {
            let Some(turn_id) = params
                .pointer("/turn/id")
                .and_then(serde_json::Value::as_str)
            else {
                return;
            };
            let status = match params
                .pointer("/turn/status")
                .and_then(serde_json::Value::as_str)
            {
                Some("failed") => CodexWorkflowTurnStatus::Failed,
                Some("interrupted") => CodexWorkflowTurnStatus::Interrupted,
                _ => CodexWorkflowTurnStatus::Completed,
            };
            workflow.complete_turn(turn_id, status);
            "turnCompleted"
        }
        _ => return,
    };

    if save_workflow(&directory, &workflow, event_type).is_ok() {
        send_workflow_to_live_session(sessions, &conversation_id, &workflow);
    }
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
        let _ = channel.send(CodexTerminalEvent::Workflow {
            workflow: workflow.clone(),
        });
    }
}

fn describe_thread_item(
    item: &serde_json::Value,
) -> Option<(String, CodexActivityKind, String, String)> {
    let id = item.get("id")?.as_str()?.to_string();
    let item_type = item.get("type")?.as_str()?;
    let (kind, summary, detail) = match item_type {
        "reasoning" => (
            CodexActivityKind::Analysis,
            "Analyzing the task".into(),
            String::new(),
        ),
        "commandExecution" => {
            let action = item
                .get("commandActions")
                .and_then(serde_json::Value::as_array)
                .and_then(|actions| actions.first())
                .and_then(|action| action.get("type"))
                .and_then(serde_json::Value::as_str);
            let summary = match action {
                Some("read") => "Reading project files",
                Some("listFiles") => "Listing project files",
                Some("search") => "Searching the workspace",
                _ => "Running a command",
            };
            let actions = item
                .get("commandActions")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .map(|action| {
                    json!({
                        "type": action.get("type"),
                        "name": action.get("name"),
                        "path": action.get("path"),
                        "query": action.get("query")
                    })
                })
                .collect::<Vec<_>>();
            (
                CodexActivityKind::Command,
                summary.into(),
                sanitize_detail(&json!({ "actions": actions })),
            )
        }
        "fileChange" => {
            let changes = item
                .get("changes")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .map(|change| {
                    json!({
                        "path": change.get("path"),
                        "kind": change.pointer("/kind/type")
                    })
                })
                .collect::<Vec<_>>();
            (
                CodexActivityKind::FileChange,
                "Updating project files".into(),
                sanitize_detail(&json!({ "changes": changes })),
            )
        }
        "mcpToolCall" | "dynamicToolCall" => {
            let tool = item
                .get("tool")
                .or_else(|| item.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("MCP tool");
            let (kind, summary) = describe_desktop_tool(tool);
            (
                kind,
                summary,
                sanitize_detail(&json!({
                    "server": item.get("server"),
                    "tool": tool,
                    "arguments": item.get("arguments")
                })),
            )
        }
        "webSearch" => (
            CodexActivityKind::Web,
            "Researching information".into(),
            sanitize_detail(&json!({
                "query": item.get("query"),
                "action": item.pointer("/action/type")
            })),
        ),
        "collabAgentToolCall" => (
            CodexActivityKind::Agent,
            "Coordinating a sub-agent".into(),
            sanitize_detail(&json!({
                "tool": item.get("tool"),
                "status": item.get("status"),
                "receiverThreadIds": item.get("receiverThreadIds")
            })),
        ),
        "subAgentActivity" => (
            CodexActivityKind::Agent,
            "Coordinating a sub-agent".into(),
            sanitize_detail(&json!({
                "agentThreadId": item.get("agentThreadId"),
                "kind": item.get("kind")
            })),
        ),
        "imageView" => (
            CodexActivityKind::Other,
            "Inspecting an image".into(),
            sanitize_detail(&json!({ "path": item.get("path") })),
        ),
        "imageGeneration" => (
            CodexActivityKind::Other,
            "Generating an image".into(),
            sanitize_detail(&json!({
                "status": item.get("status"),
                "savedPath": item.get("savedPath")
            })),
        ),
        "contextCompaction" => (
            CodexActivityKind::Other,
            "Compacting conversation context".into(),
            String::new(),
        ),
        "enteredReviewMode" | "exitedReviewMode" => (
            CodexActivityKind::Analysis,
            "Reviewing project changes".into(),
            String::new(),
        ),
        "sleep" => (
            CodexActivityKind::Other,
            "Waiting before continuing".into(),
            sanitize_detail(&json!({ "durationMs": item.get("durationMs") })),
        ),
        "agentMessage" | "userMessage" | "hookPrompt" | "functionCallOutput" => {
            return None;
        }
        _ => return None,
    };
    Some((id, kind, summary, detail))
}

fn describe_desktop_tool(tool_name: &str) -> (CodexActivityKind, String) {
    let kind = if tool_name.contains("test") {
        CodexActivityKind::Test
    } else if tool_name.starts_with("game_") {
        CodexActivityKind::GameInstance
    } else {
        CodexActivityKind::Mcp
    };
    let summary = match tool_name {
        "game_instance_launch" => "Starting a game instance",
        "game_instance_wait_for_state" => "Waiting for the game process",
        "game_instance_wait_for_mcp" => "Waiting for the game Runtime MCP",
        "game_instance_set_window_visibility" => "Updating the game window",
        "game_instance_stop" => "Stopping a game instance",
        "game_runtime_call_tool" | "game_instance_mcp_call" => "Using a game runtime tool",
        "game_runtime_list_tools" | "game_instance_mcp_tools_list" => {
            "Inspecting game runtime capabilities"
        }
        "game_log_query" => "Inspecting game logs",
        other => return (kind, format!("Using {other}")),
    };
    (kind, summary.into())
}

#[derive(Debug, Clone)]
struct NativeSessionRecord {
    id: String,
    cwd: PathBuf,
    started_at: DateTime<Utc>,
}

fn spawn_native_session_discovery(
    tasks: TaskService,
    task_id: String,
    conversation_id: String,
    working_directory: PathBuf,
    created_at: String,
) {
    std::thread::spawn(move || {
        for _ in 0..NATIVE_SESSION_DISCOVERY_ATTEMPTS {
            if let Some(session_id) = find_native_session_id(&working_directory, &created_at) {
                let Ok(task) = tasks.get(&task_id) else {
                    return;
                };
                let directory =
                    conversation_directory(Path::new(&task.workspace_path), &conversation_id);
                let metadata = directory.join(CONVERSATION_METADATA_FILE);
                let Ok(mut conversation) = std::fs::read_to_string(&metadata).and_then(|value| {
                    serde_json::from_str::<CodexConversation>(&value).map_err(std::io::Error::other)
                }) else {
                    return;
                };
                if conversation.native_session_id.is_none() {
                    conversation.native_session_id = Some(session_id);
                    conversation.updated_at = Utc::now().to_rfc3339();
                    let _ = write_conversation(&directory, &conversation);
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}

fn find_native_session_id(working_directory: &Path, created_at: &str) -> Option<String> {
    let created_at = DateTime::parse_from_rfc3339(created_at)
        .ok()?
        .with_timezone(&Utc);
    let root = codex_sessions_root()?;
    find_native_session_id_in(&root, working_directory, created_at)
}

fn find_native_session_id_in(
    root: &Path,
    working_directory: &Path,
    created_at: DateTime<Utc>,
) -> Option<String> {
    native_session_records_in(root, working_directory)
        .into_iter()
        .filter(|record| paths_equal(&record.cwd, working_directory))
        .filter_map(|record| {
            let distance = (record.started_at - created_at).num_seconds().abs();
            (distance <= NATIVE_SESSION_MATCH_SECONDS).then_some((distance, record))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, record)| record.id)
}

fn backfill_native_session_ids(
    working_directory: &Path,
    conversations: &mut [CodexConversation],
) -> AppResult<()> {
    let Some(root) = codex_sessions_root() else {
        return Ok(());
    };
    let records = native_session_records_in(&root, working_directory);
    if records.is_empty() {
        return Ok(());
    }

    let assignments = assign_native_session_ids(conversations, records);
    for (conversation_index, _) in assignments {
        let directory =
            conversation_directory(working_directory, &conversations[conversation_index].id);
        write_conversation(&directory, &conversations[conversation_index])?;
    }
    Ok(())
}

fn assign_native_session_ids(
    conversations: &mut [CodexConversation],
    records: Vec<NativeSessionRecord>,
) -> Vec<(usize, String)> {
    let reserved_ids = conversations
        .iter()
        .filter_map(|conversation| conversation.native_session_id.as_deref())
        .collect::<HashSet<_>>();
    let mut available_records = records
        .into_iter()
        .filter(|record| !reserved_ids.contains(record.id.as_str()))
        .collect::<Vec<_>>();
    let mut pending_indices = conversations
        .iter()
        .enumerate()
        .filter_map(|(index, conversation)| {
            conversation.native_session_id.is_none().then_some(index)
        })
        .collect::<Vec<_>>();
    let mut assignments = Vec::new();
    while !pending_indices.is_empty() && !available_records.is_empty() {
        let best = pending_indices
            .iter()
            .flat_map(|conversation_index| {
                let conversation = &conversations[*conversation_index];
                let Some(created_at) = DateTime::parse_from_rfc3339(&conversation.created_at)
                    .ok()
                    .map(|value| value.with_timezone(&Utc))
                else {
                    return Vec::new();
                };
                available_records
                    .iter()
                    .enumerate()
                    .map(move |(record_index, record)| {
                        (
                            (record.started_at - created_at).num_seconds().abs(),
                            *conversation_index,
                            record_index,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .min_by_key(|candidate| *candidate);
        let Some((distance, conversation_index, record_index)) = best else {
            break;
        };

        let unique_legacy_match = pending_indices.len() == 1 && available_records.len() == 1;
        if distance > NATIVE_SESSION_MATCH_SECONDS && !unique_legacy_match {
            break;
        }

        let record = available_records.remove(record_index);
        pending_indices.retain(|index| *index != conversation_index);
        conversations[conversation_index].native_session_id = Some(record.id.clone());
        assignments.push((conversation_index, record.id));
    }
    assignments
}

fn native_session_records_in(root: &Path, working_directory: &Path) -> Vec<NativeSessionRecord> {
    let mut files = Vec::new();
    collect_native_session_files(root, 0, &mut files);
    files
        .into_iter()
        .filter_map(|path| read_native_session_record(&path))
        .filter(|record| paths_equal(&record.cwd, working_directory))
        .collect()
}

fn codex_sessions_root() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .map(|path| path.join(".codex"))
        })
        .map(|path| path.join("sessions"))
}

fn collect_native_session_files(root: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if depth < 3 {
                collect_native_session_files(&path, depth + 1, files);
            }
        } else if file_type.is_file() && path.extension() == Some(OsStr::new("jsonl")) {
            files.push(path);
        }
    }
}

fn read_native_session_record(path: &Path) -> Option<NativeSessionRecord> {
    let file = File::open(path).ok()?;
    let mut line = String::new();
    BufReader::new(file).read_line(&mut line).ok()?;
    let value: serde_json::Value = serde_json::from_str(&line).ok()?;
    if value.get("type")?.as_str()? != "session_meta" {
        return None;
    }
    let payload = value.get("payload")?;
    let id = payload
        .get("session_id")
        .or_else(|| payload.get("id"))?
        .as_str()?
        .to_string();
    let cwd = PathBuf::from(payload.get("cwd")?.as_str()?);
    let started_at = DateTime::parse_from_rfc3339(payload.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    Some(NativeSessionRecord {
        id,
        cwd,
        started_at,
    })
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

fn resolve_codex() -> Option<ResolvedCodex> {
    let mut directories = path_directories(std::env::var_os("PATH").as_deref());
    #[cfg(windows)]
    {
        directories.extend(windows_persisted_path_directories());
        directories.extend(standard_windows_codex_directories(
            std::env::var_os("APPDATA").as_deref(),
        ));
    }
    resolve_codex_in_directories(directories, std::env::var_os("PATHEXT").as_deref())
}

#[cfg(test)]
fn resolve_codex_in(path: Option<&OsStr>, path_ext: Option<&OsStr>) -> Option<ResolvedCodex> {
    resolve_codex_in_directories(path_directories(path), path_ext)
}

fn resolve_codex_in_directories(
    directories: impl IntoIterator<Item = PathBuf>,
    path_ext: Option<&OsStr>,
) -> Option<ResolvedCodex> {
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
            let candidate = directory.join(format!("codex{extension}"));
            if !candidate.is_file() {
                continue;
            }
            let kind = match extension.as_str() {
                ".cmd" | ".bat" => CodexLaunchKind::CommandScript,
                ".exe" | ".com" => CodexLaunchKind::Executable,
                _ => continue,
            };
            if kind == CodexLaunchKind::CommandScript
                && let Some(native) = resolve_npm_native_codex(&candidate)
            {
                return Some(ResolvedCodex {
                    path: native,
                    kind: CodexLaunchKind::Executable,
                });
            }
            return Some(ResolvedCodex {
                path: candidate,
                kind,
            });
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

#[cfg(windows)]
fn standard_windows_codex_directories(app_data: Option<&OsStr>) -> Vec<PathBuf> {
    app_data
        .map(PathBuf::from)
        .map(|directory| vec![directory.join("npm")])
        .unwrap_or_default()
}

fn resolve_npm_native_codex(shim: &Path) -> Option<PathBuf> {
    let package_scope = shim
        .parent()?
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("node_modules")
        .join("@openai");
    for package in std::fs::read_dir(package_scope).ok()?.flatten() {
        if !package
            .file_name()
            .to_string_lossy()
            .starts_with("codex-win32-")
        {
            continue;
        }
        let vendor = package.path().join("vendor");
        for target in std::fs::read_dir(vendor).ok()?.flatten() {
            let executable = target.path().join("bin").join("codex.exe");
            if executable.is_file() {
                return Some(executable);
            }
        }
    }
    None
}

fn disconnect_and_terminate(session: LiveTerminal) -> AppResult<()> {
    // Drop the UI channel before killing so PTY output cannot block on webview IPC.
    let subscriber = session.subscriber.lock().take();
    let pid = {
        let mut state = session.state.lock();
        state.status = CodexTerminalStatus::Stopping;
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
                        output.push(
                            // The prefix was validated by from_utf8.
                            unsafe { std::str::from_utf8_unchecked(&self.pending[..valid]) }
                                .to_string(),
                        );
                        self.pending.drain(..valid);
                    }
                    if let Some(length) = error.error_len() {
                        output.push("\u{fffd}".to_string());
                        self.pending.drain(..length.min(self.pending.len()));
                    } else {
                        break;
                    }
                }
            }
        }
        output
    }

    fn finish(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(std::mem::take(&mut self.pending).as_slice()).into_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{AppPaths, Database};
    use crate::modules::tasks::{TaskInput, TaskStatus};
    use std::fs;

    #[test]
    fn resolves_executable_before_command_script() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("codex.cmd"), "@echo off").unwrap();
        fs::write(root.join("codex.exe"), "").unwrap();
        let path = std::env::join_paths([&root]).unwrap();
        let resolved = resolve_codex_in(Some(&path), Some(OsStr::new(".CMD;.EXE"))).unwrap();
        assert_eq!(resolved.kind, CodexLaunchKind::Executable);
        assert_eq!(resolved.path, root.join("codex.exe"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_npm_command_script() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("codex.cmd"), "@echo off").unwrap();
        let path = std::env::join_paths([&root]).unwrap();
        let resolved = resolve_codex_in(Some(&path), Some(OsStr::new(".EXE;.CMD"))).unwrap();
        assert_eq!(resolved.kind, CodexLaunchKind::CommandScript);
        assert_eq!(resolved.path, root.join("codex.cmd"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preserves_split_utf8_sequences() {
        let mut decoder = Utf8ChunkDecoder::default();
        let bytes = "终端".as_bytes();
        assert!(decoder.push(&bytes[..2]).is_empty());
        let output = decoder.push(&bytes[2..]).join("");
        assert_eq!(output, "终端");
        assert!(decoder.finish().is_none());
    }

    #[test]
    fn validates_terminal_dimensions() {
        assert!(validate_size(80, 24).is_ok());
        assert!(validate_size(10, 24).is_err());
        assert!(validate_size(80, 300).is_err());
    }

    #[test]
    fn command_script_uses_comspec_and_fixed_working_directory() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let script = root.join("codex.cmd");
        fs::write(&script, "@echo off\r\n").unwrap();
        let command = build_command(
            &ResolvedCodex {
                path: script.clone(),
                kind: CodexLaunchKind::CommandScript,
            },
            &root,
            "task-123",
            "conversation-123",
            None,
            None,
        );
        let args = command.get_argv();
        assert_eq!(
            args.first().map(PathBuf::from),
            Some(PathBuf::from(
                std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into())
            ))
        );
        assert_eq!(
            &args[1..],
            [
                OsStr::new("/D"),
                OsStr::new("/S"),
                OsStr::new("/C"),
                OsStr::new("call"),
                script.as_os_str(),
                OsStr::new("--cd"),
                root.as_os_str(),
                OsStr::new("--no-alt-screen"),
            ]
        );
        assert_eq!(
            command.get_cwd().map(|value| value.as_os_str()),
            Some(root.as_os_str())
        );
        assert_eq!(command.get_env("TERM"), Some(OsStr::new("xterm-256color")));
        assert_eq!(command.get_env("COLORTERM"), Some(OsStr::new("truecolor")));
        assert_eq!(
            command.get_env("ABYA_DEVELOPMENT_TASK_ID"),
            Some(OsStr::new("task-123"))
        );
        assert_eq!(
            command.get_env("ABYA_DEVELOPMENT_CONVERSATION_ID"),
            Some(OsStr::new("conversation-123"))
        );
        assert_eq!(
            command.get_env("ABYA_DEVELOPMENT_WORKSPACE"),
            Some(root.as_os_str())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resumes_a_saved_native_session() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let script = root.join("codex.cmd");
        fs::write(&script, "@echo off\r\n").unwrap();
        let command = build_command(
            &ResolvedCodex {
                path: script.clone(),
                kind: CodexLaunchKind::CommandScript,
            },
            &root,
            "task-123",
            "conversation-123",
            Some("native-session-123"),
            None,
        );
        let args = command.get_argv();
        assert_eq!(
            &args[1..],
            [
                OsStr::new("/D"),
                OsStr::new("/S"),
                OsStr::new("/C"),
                OsStr::new("call"),
                script.as_os_str(),
                OsStr::new("resume"),
                OsStr::new("native-session-123"),
                OsStr::new("--cd"),
                root.as_os_str(),
                OsStr::new("--no-alt-screen"),
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn remote_tui_uses_the_app_server_endpoint_and_environment_token() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("codex.exe");
        fs::write(&executable, "").unwrap();
        let remote = AppServerHandle::for_test("ws://127.0.0.1:49999", "capability-token");
        let command = build_command(
            &ResolvedCodex {
                path: executable,
                kind: CodexLaunchKind::Executable,
            },
            &root,
            "task-123",
            "conversation-123",
            Some("native-session-123"),
            Some(&remote),
        );
        let args = command.get_argv();
        assert_eq!(
            &args[1..],
            [
                OsStr::new("resume"),
                OsStr::new("native-session-123"),
                OsStr::new("--remote"),
                OsStr::new("ws://127.0.0.1:49999"),
                OsStr::new("--remote-auth-token-env"),
                OsStr::new("ABYA_CODEX_APP_SERVER_TOKEN"),
                OsStr::new("--cd"),
                root.as_os_str(),
                OsStr::new("--no-alt-screen"),
            ]
        );
        assert_eq!(
            command.get_env("ABYA_CODEX_APP_SERVER_TOKEN"),
            Some(OsStr::new("capability-token"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn finds_native_session_by_workspace_and_creation_time() {
        let root = test_root();
        let workspace = root.join("workspace");
        let sessions = root.join("sessions").join("2026").join("08").join("31");
        fs::create_dir_all(&sessions).unwrap();
        let started_at = Utc::now();
        let session_id = "native-session-123";
        fs::write(
            sessions.join("rollout.jsonl"),
            serde_json::json!({
                "type": "session_meta",
                "payload": {
                    "session_id": session_id,
                    "cwd": workspace,
                    "timestamp": started_at.to_rfc3339(),
                }
            })
            .to_string(),
        )
        .unwrap();

        assert_eq!(
            find_native_session_id_in(&root.join("sessions"), &workspace, started_at),
            Some(session_id.to_string())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn assigns_a_unique_legacy_session_without_a_time_window() {
        let workspace = PathBuf::from(r"C:\Workspaces\legacy-task");
        let mut conversations = vec![CodexConversation {
            id: "conversation-1".into(),
            task_id: "task-1".into(),
            title: "Conversation 1".into(),
            created_at: "2026-08-01T12:00:00Z".into(),
            updated_at: "2026-08-01T12:00:00Z".into(),
            native_session_id: None,
        }];
        let records = vec![NativeSessionRecord {
            id: "native-session-1".into(),
            cwd: workspace,
            started_at: DateTime::parse_from_rfc3339("2026-08-01T12:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
        }];

        let assignments = assign_native_session_ids(&mut conversations, records);

        assert_eq!(assignments, vec![(0, "native-session-1".to_string())]);
        assert_eq!(
            conversations[0].native_session_id.as_deref(),
            Some("native-session-1")
        );
    }

    #[test]
    fn resolves_native_binary_from_official_npm_shim() {
        let root = test_root();
        let package = root
            .join("node_modules")
            .join("@openai")
            .join("codex")
            .join("node_modules")
            .join("@openai")
            .join("codex-win32-x64")
            .join("vendor")
            .join("x86_64-pc-windows-msvc")
            .join("bin");
        fs::create_dir_all(&package).unwrap();
        fs::write(root.join("codex.cmd"), "@echo off").unwrap();
        fs::write(package.join("codex.exe"), "").unwrap();
        let path = std::env::join_paths([&root]).unwrap();
        let resolved = resolve_codex_in(Some(&path), Some(OsStr::new(".CMD"))).unwrap();
        assert_eq!(resolved.kind, CodexLaunchKind::Executable);
        assert_eq!(resolved.path, package.join("codex.exe"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_from_additional_directory_when_process_path_is_stale() {
        let root = test_root();
        let stale = root.join("stale");
        let npm = root.join("npm");
        fs::create_dir_all(&stale).unwrap();
        fs::create_dir_all(&npm).unwrap();
        fs::write(npm.join("codex.cmd"), "@echo off").unwrap();

        let resolved =
            resolve_codex_in_directories([stale, npm.clone()], Some(OsStr::new(".CMD"))).unwrap();

        assert_eq!(resolved.kind, CodexLaunchKind::CommandScript);
        assert_eq!(resolved.path, npm.join("codex.cmd"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn standard_windows_codex_directory_uses_appdata_npm() {
        let app_data = PathBuf::from(r"C:\Users\Developer\AppData\Roaming");
        assert_eq!(
            standard_windows_codex_directories(Some(app_data.as_os_str())),
            [app_data.join("npm")]
        );
    }

    #[test]
    fn all_task_statuses_pass_terminal_open_validation() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        let database = Database::open(&paths).unwrap();
        let tasks = TaskService::new(database, root.join("workspaces"));
        let active = tasks
            .create(TaskInput {
                title: "Active".into(),
                description: String::new(),
            })
            .unwrap();
        let completed = tasks
            .create(TaskInput {
                title: "Completed".into(),
                description: String::new(),
            })
            .unwrap();
        tasks
            .set_status(&completed.id, TaskStatus::Completed)
            .unwrap();
        let archived = tasks
            .create(TaskInput {
                title: "Archived".into(),
                description: String::new(),
            })
            .unwrap();
        tasks
            .set_status(&archived.id, TaskStatus::Archived)
            .unwrap();

        let script = root.join("codex.cmd");
        fs::write(&script, "@echo off\r\n").unwrap();
        let service = CodexTerminalService::for_test(
            tasks,
            root.clone(),
            ResolvedCodex {
                path: script,
                kind: CodexLaunchKind::CommandScript,
            },
        );

        let first = service.create_conversation(&active.id, None).unwrap();
        let second = service
            .create_conversation(&active.id, Some("Review".into()))
            .unwrap();
        assert_ne!(first.id, second.id);
        assert!(
            Path::new(&active.workspace_path)
                .join("conversations")
                .join(&first.id)
                .join("conversation.json")
                .is_file()
        );
        assert_eq!(service.list_conversations(&active.id).unwrap().len(), 2);
        for task in [&active, &completed, &archived] {
            let conversation = service.create_conversation(&task.id, None).unwrap();
            assert!(
                service
                    .validate_open_request(&task.id, &conversation.id)
                    .is_ok()
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn maps_native_app_server_notifications_into_the_workflow() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        let database = Database::open(&paths).unwrap();
        let tasks = TaskService::new(database, root.join("workspaces"));
        let task = tasks
            .create(TaskInput {
                title: "Observable".into(),
                description: String::new(),
            })
            .unwrap();
        let service = CodexTerminalService::new(tasks.clone()).unwrap();
        let conversation = service.create_conversation(&task.id, None).unwrap();
        service.thread_conversations.lock().insert(
            "thread-1".into(),
            (task.id.clone(), conversation.id.clone()),
        );

        for notification in [
            serde_json::json!({
                "method": "turn/started",
                "params": {
                    "threadId": "thread-1",
                    "turn": { "id": "turn-1" }
                }
            }),
            serde_json::json!({
                "method": "turn/plan/updated",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "explanation": "Implement and verify",
                    "plan": [
                        { "step": "Inspect", "status": "completed" },
                        { "step": "Test", "status": "inProgress" }
                    ]
                }
            }),
            serde_json::json!({
                "method": "item/started",
                "params": {
                    "threadId": "thread-1",
                    "turnId": "turn-1",
                    "item": {
                        "id": "item-1",
                        "type": "commandExecution",
                        "commandActions": [
                            { "type": "read", "path": "src/App.tsx" }
                        ]
                    }
                }
            }),
        ] {
            handle_app_server_notification(
                &tasks,
                &service.sessions,
                &service.thread_conversations,
                &service.workflow_write_lock,
                notification,
            );
        }

        let workflow = service.workflow(&task.id, &conversation.id).unwrap();
        assert_eq!(workflow.observability, CodexObservabilityStatus::Native);
        assert_eq!(workflow.turns.len(), 1);
        assert_eq!(workflow.turns[0].plan.len(), 2);
        assert_eq!(
            workflow.turns[0].plan[1].status,
            CodexPlanStepStatus::InProgress
        );
        assert_eq!(workflow.turns[0].activities.len(), 1);
        assert_eq!(
            workflow.turns[0].activities[0].kind,
            CodexActivityKind::Command
        );
        assert!(!workflow.turns[0].activities[0].unplanned);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deleting_a_conversation_removes_its_native_thread_binding() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        let database = Database::open(&paths).unwrap();
        let tasks = TaskService::new(database, root.join("workspaces"));
        let task = tasks
            .create(TaskInput {
                title: "Delete binding".into(),
                description: String::new(),
            })
            .unwrap();
        let service = CodexTerminalService::new(tasks).unwrap();
        let conversation = service.create_conversation(&task.id, None).unwrap();
        service.thread_conversations.lock().insert(
            "thread-1".into(),
            (task.id.clone(), conversation.id.clone()),
        );

        service
            .delete_conversation(&task.id, &conversation.id)
            .unwrap();

        assert!(service.thread_conversations.lock().is_empty());
        assert!(
            !Path::new(&task.workspace_path)
                .join("conversations")
                .join(&conversation.id)
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_activity_details_omit_command_output_and_mcp_results() {
        let (_, _, _, command_detail) = describe_thread_item(&serde_json::json!({
            "id": "command",
            "type": "commandExecution",
            "status": "completed",
            "aggregatedOutput": "Bearer must-not-be-persisted",
            "commandActions": [
                {
                    "type": "read",
                    "name": "App",
                    "path": "src/App.tsx",
                    "command": "Get-Content src/App.tsx"
                }
            ]
        }))
        .unwrap();
        assert!(command_detail.contains("src/App.tsx"));
        assert!(!command_detail.contains("Bearer must-not-be-persisted"));
        assert!(!command_detail.contains("Get-Content"));

        let (_, _, _, mcp_detail) = describe_thread_item(&serde_json::json!({
            "id": "mcp",
            "type": "mcpToolCall",
            "server": "desktop",
            "tool": "game_instance_list",
            "arguments": {
                "taskId": "task",
                "authorization": "Bearer secret"
            },
            "result": {
                "content": [{ "type": "text", "text": "must-not-be-persisted" }]
            }
        }))
        .unwrap();
        assert!(mcp_detail.contains("game_instance_list"));
        assert!(mcp_detail.contains("[REDACTED]"));
        assert!(!mcp_detail.contains("must-not-be-persisted"));
        assert!(!mcp_detail.contains("Bearer secret"));
    }

    #[test]
    fn stop_task_does_not_send_ui_events_while_killing_the_process() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let paths = test_paths(&root);
        let database = Database::open(&paths).unwrap();
        let tasks = TaskService::new(database, root.join("workspaces"));
        let task = tasks
            .create(TaskInput {
                title: "Stop task".into(),
                description: String::new(),
            })
            .unwrap();
        let script = root.join("codex.cmd");
        fs::write(&script, "@echo off\r\n").unwrap();
        let service = CodexTerminalService::for_test(
            tasks,
            root.clone(),
            ResolvedCodex {
                path: script,
                kind: CodexLaunchKind::CommandScript,
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
        let _ = fs::remove_dir_all(root);
    }

    fn insert_live_session(
        service: &CodexTerminalService,
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
                state: Arc::new(Mutex::new(CodexTerminalState {
                    task_id: task_id.to_string(),
                    conversation_id: conversation_id.to_string(),
                    status: CodexTerminalStatus::Running,
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

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!("abya-codex-terminal-{}", Uuid::new_v4()))
    }

    fn test_paths(root: &Path) -> AppPaths {
        AppPaths {
            data_dir: root.to_path_buf(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        }
    }
}
