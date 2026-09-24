use super::{
    GameInstance, InstanceOrigin, InstanceRuntimeInfo, InstanceStopResult, LaunchInstanceInput,
    LaunchMode, LaunchReportResult, ProcessState, WindowMode, WindowVisibilityMode,
    build_launch_contract, validate_executable,
};
use crate::foundation::{AppError, AppPaths, AppResult, Database};
use crate::modules::game_connections::{ConnectionRegistration, GameConnectionService, GameHello};
use chrono::Utc;
use parking_lot::Mutex;
use process_wrap::std::{ChildWrapper, CommandWrap, JobObject};
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::window::{InstanceWindowController, capture_foreground_window};

const MAX_INSTANCE_LOG_BYTES: u64 = 64 * 1024 * 1024;

struct LiveInstance {
    child: Arc<Mutex<Box<dyn ChildWrapper>>>,
    window: InstanceWindowController,
}

#[derive(Clone)]
pub struct InstanceService {
    database: Database,
    paths: AppPaths,
    connections: GameConnectionService,
    live: Arc<Mutex<HashMap<String, LiveInstance>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceLogRepairReport {
    pub files_truncated: u64,
    pub bytes_reclaimed: u64,
}

impl InstanceService {
    pub fn new(database: Database, paths: AppPaths, connections: GameConnectionService) -> Self {
        Self {
            database,
            paths,
            connections,
            live: Default::default(),
        }
    }

    pub fn list(&self, task_id: Option<&str>) -> AppResult<Vec<GameInstance>> {
        self.database.with_connection(|connection| {
            let sql = if task_id.is_some() {
                format!("{SELECT_INSTANCE} WHERE task_id=?1 ORDER BY started_at DESC")
            } else {
                format!("{SELECT_INSTANCE} ORDER BY COALESCE(connected_at,started_at) DESC")
            };
            let mut statement = connection.prepare(&sql)?;
            if let Some(task_id) = task_id {
                let rows = statement.query_map([task_id], map_instance)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            } else {
                let rows = statement.query_map([], map_instance)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
        })
    }

    pub fn get(&self, id: &str) -> AppResult<InstanceRuntimeInfo> {
        let instance = self.read(id)?;
        let report_path = (instance.origin == InstanceOrigin::Managed)
            .then(|| AppPaths::display(&self.paths.instance_report_path(id)));
        Ok(InstanceRuntimeInfo {
            gateway_endpoint: self.connections.state().preferred_endpoint,
            report_path,
            process_alive: self.live.lock().contains_key(id),
            instance,
        })
    }

    pub fn launch(&self, mut input: LaunchInstanceInput) -> AppResult<GameInstance> {
        let task_id = input.task_id.trim();
        if task_id.is_empty() {
            return Err(AppError::validation("Select a development task."));
        }
        let name = input.name.trim();
        if name.is_empty() {
            return Err(AppError::validation("Instance name is required."));
        }
        validate_executable(input.executable_path.trim())?;
        self.ensure_active_task(task_id)?;

        let inherited = if input.profile.mode == LaunchMode::LanClient {
            let host_id = input
                .profile
                .host_instance_id
                .as_deref()
                .ok_or_else(|| AppError::validation("Select a running Host instance."))?;
            let host = self.read(host_id)?;
            if host.task_id.as_deref() != Some(task_id)
                || host.mode != Some(LaunchMode::LanHost)
                || host.process_state != ProcessState::Running
                || !self.live.lock().contains_key(host_id)
            {
                return Err(AppError::validation(
                    "ClientOnly requires a running Host from the same task.",
                ));
            }
            input.profile.archive = host
                .profile
                .as_ref()
                .and_then(|profile| profile.archive.clone());
            host.host_port
        } else {
            None
        };

        let id = Uuid::new_v4().to_string();
        let gateway_endpoint = self.connections.preferred_endpoint()?;
        let contract = build_launch_contract(
            &id,
            name,
            &input.profile,
            &self.paths,
            inherited,
            &gateway_endpoint,
        )?;
        let profile_json = serde_json::to_string(&input.profile)?;
        let args_json = serde_json::to_string(&contract.sanitized_args)?;
        let now = Utc::now().to_rfc3339();
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO game_instances(
                    id,task_id,origin,name,mode,host_instance_id,executable_path,
                    launch_profile_json,sanitized_args_json,process_state,connection_state,
                    started_at,host_port,log_file_path)
                 VALUES(?1,?2,'managed',?3,?4,?5,?6,?7,?8,'launching','waiting',?9,?10,?11)",
                rusqlite::params![
                    id,
                    task_id,
                    name,
                    input.profile.mode.as_db(),
                    input.profile.host_instance_id,
                    input.executable_path.trim(),
                    profile_json,
                    args_json,
                    now,
                    contract.host_port,
                    contract.log_path
                ],
            )?;
            Ok(())
        })?;

        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&contract.log_path)?;
        let stderr = stdout.try_clone()?;
        let executable = input.executable_path.trim().to_string();
        let working_directory = Path::new(&executable).parent().map(Path::to_path_buf);
        let log_path = PathBuf::from(&contract.log_path);
        let args = contract.args;
        #[cfg(test)]
        let args = {
            let mut isolated = args;
            isolated.push(format!(
                "--abya-data-root={}",
                self.paths.data_dir.join("IsolatedGameData").display()
            ));
            isolated
        };
        let restore_foreground = capture_foreground_window();
        let mut command = CommandWrap::with_new(&executable, |command| {
            command
                .env("ABYA_CLI_DEVELOPMENT", "1")
                .args(&args)
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            if let Some(directory) = &working_directory {
                command.current_dir(directory);
            }
        });
        command.wrap(JobObject);
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.mark_launch_failed(&id, &error.to_string())?;
                return Err(error.into());
            }
        };
        let pid = child.id();
        let child = Arc::new(Mutex::new(child));
        let window = match InstanceWindowController::start(
            pid,
            input.profile.visibility_mode,
            restore_foreground,
        ) {
            Ok(window) => window,
            Err(error) => {
                let _ = child.lock().kill();
                self.mark_launch_failed(&id, &error.to_string())?;
                return Err(error.into());
            }
        };
        self.live.lock().insert(
            id.clone(),
            LiveInstance {
                child: child.clone(),
                window,
            },
        );
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_instances SET process_state='running',pid=?2 WHERE id=?1",
                rusqlite::params![id, pid],
            )?;
            Ok(())
        })?;
        self.monitor(id.clone(), child, log_path);
        self.read(&id)
    }

    pub fn stop(&self, id: &str) -> AppResult<InstanceStopResult> {
        let instance = self.read(id)?;
        if instance.origin != InstanceOrigin::Managed {
            return Err(AppError::validation(
                "External game instances cannot be stopped by the desktop tool.",
            ));
        }
        let child = self
            .live
            .lock()
            .get(id)
            .map(|entry| entry.child.clone())
            .ok_or_else(|| AppError::validation("The instance is not running."))?;
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_instances SET process_state='stopping' WHERE id=?1",
                [id],
            )?;
            Ok(())
        })?;
        let graceful_exit_requested = request_graceful_exit(instance.pid);
        let graceful_exit_observed =
            graceful_exit_requested && self.wait_until_not_live(id, Duration::from_secs(5));
        let forced_termination = !graceful_exit_observed;
        if forced_termination {
            child.lock().kill()?;
        }
        let stopped =
            graceful_exit_observed || self.wait_until_not_live(id, Duration::from_secs(10));
        let instance = self.read(id)?;
        Ok(InstanceStopResult {
            instance,
            graceful_exit_requested,
            graceful_exit_observed,
            forced_termination,
            timed_out: !stopped,
            process_alive: self.live.lock().contains_key(id),
        })
    }

    pub fn set_window_visibility(
        &self,
        id: &str,
        visibility: WindowVisibilityMode,
    ) -> AppResult<GameInstance> {
        let mut instance = self.read(id)?;
        if instance.origin != InstanceOrigin::Managed {
            return Err(AppError::validation(
                "External game instances do not expose desktop window control.",
            ));
        }
        let profile = instance
            .profile
            .as_mut()
            .ok_or_else(|| AppError::validation("The instance has no launch profile."))?;
        if visibility == WindowVisibilityMode::Background
            && !matches!(profile.window_mode, WindowMode::Windowed)
        {
            return Err(AppError::validation(
                "Only windowed instances can enter background mode.",
            ));
        }

        let live = self.live.lock();
        let entry = live
            .get(id)
            .ok_or_else(|| AppError::validation("The instance is not running."))?;
        entry.window.set_visibility(visibility);
        drop(live);

        profile.visibility_mode = visibility;
        let profile_json = serde_json::to_string(profile)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_instances SET launch_profile_json=?2 WHERE id=?1",
                rusqlite::params![id, profile_json],
            )?;
            Ok(())
        })?;
        self.read(id)
    }

    pub fn wait_for_state(
        &self,
        id: &str,
        target: ProcessState,
        timeout: Duration,
    ) -> AppResult<GameInstance> {
        let started = Instant::now();
        loop {
            let instance = self.read(id)?;
            if instance.process_state == target {
                return Ok(instance);
            }
            if started.elapsed() >= timeout {
                return Err(AppError::new(
                    "instanceStateTimeout",
                    "The game instance did not reach the requested process state before timeout.",
                    format!(
                        "instanceId={id}, target={target:?}, current={:?}",
                        instance.process_state
                    ),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn read_launch_report(&self, id: &str) -> AppResult<LaunchReportResult> {
        let instance = self.read(id)?;
        if instance.origin != InstanceOrigin::Managed {
            return Err(AppError::validation(
                "External instances do not have desktop launch reports.",
            ));
        }
        let path = self.paths.instance_report_path(id);
        if !path.is_file() {
            return Ok(LaunchReportResult {
                instance_id: id.to_string(),
                path: AppPaths::display(&path),
                exists: false,
                report: serde_json::Value::Null,
            });
        }
        let metadata = std::fs::metadata(&path)?;
        if metadata.len() > 1024 * 1024 {
            return Err(AppError::validation(
                "The launch report exceeds the 1 MiB safety limit.",
            ));
        }
        let report = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        Ok(LaunchReportResult {
            instance_id: id.to_string(),
            path: AppPaths::display(&path),
            exists: true,
            report,
        })
    }

    pub(crate) fn runtime_pid(&self, id: &str) -> AppResult<u32> {
        let instance = self.read(id)?;
        if instance.origin != InstanceOrigin::Managed {
            return Err(AppError::validation(
                "External instances have no runtime control.",
            ));
        }
        self.live
            .lock()
            .get(id)
            .map(|entry| entry.child.lock().id())
            .ok_or_else(|| AppError::validation("The managed instance is not running."))
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        if self.live.lock().contains_key(id) {
            return Err(AppError::validation(
                "Stop the instance before deleting it.",
            ));
        }
        self.database.with_connection(|connection| {
            let states = connection
                .query_row(
                    "SELECT process_state,connection_state FROM game_instances WHERE id=?1",
                    [id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(|| AppError::not_found("Game instance"))?;
            if matches!(states.0.as_str(), "launching" | "running" | "stopping")
                || states.1 == "connected"
            {
                return Err(AppError::validation(
                    "Stop or disconnect the instance before deleting it.",
                ));
            }
            connection.execute("DELETE FROM game_instances WHERE id=?1", [id])?;
            Ok(())
        })
    }

    pub fn register_connection(
        &self,
        hello: &GameHello,
        remote_address: SocketAddr,
    ) -> AppResult<ConnectionRegistration> {
        let now = Utc::now().to_rfc3339();
        let capabilities = serde_json::to_string(&hello.capabilities)?;
        let supplied_id = hello
            .instance_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (instance_id, origin) = if let Some(instance_id) = supplied_id {
            let instance = self.read(instance_id)?;
            if instance.origin != InstanceOrigin::Managed {
                return Err(AppError::validation(
                    "Only desktop-managed instances may claim an instance ID.",
                ));
            }
            (instance_id.to_string(), InstanceOrigin::Managed)
        } else {
            let existing = self.database.with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT id FROM game_instances
                         WHERE origin='external' AND runtime_instance_id=?1
                         ORDER BY connected_at DESC LIMIT 1",
                        [&hello.runtime_instance_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(Into::into)
            })?;
            (
                existing.unwrap_or_else(|| Uuid::new_v4().to_string()),
                InstanceOrigin::External,
            )
        };
        self.database.with_connection(|connection| {
            if origin == InstanceOrigin::External {
                connection.execute(
                    "INSERT INTO game_instances(
                        id,task_id,origin,name,process_state,connection_state,runtime_instance_id,
                        remote_address,game_version,platform,runtime_capabilities_json,
                        connected_at,last_seen_at)
                     VALUES(?1,NULL,'external',?2,'unmanaged','connected',?3,?4,?5,?6,?7,?8,?8)
                     ON CONFLICT(id) DO UPDATE SET
                        name=excluded.name,connection_state='connected',
                        remote_address=excluded.remote_address,game_version=excluded.game_version,
                        platform=excluded.platform,
                        runtime_capabilities_json=excluded.runtime_capabilities_json,
                        connected_at=excluded.connected_at,disconnected_at=NULL,
                        last_seen_at=excluded.last_seen_at",
                    rusqlite::params![
                        instance_id,
                        external_name(hello),
                        hello.runtime_instance_id,
                        remote_address.to_string(),
                        hello.game_version,
                        hello.platform,
                        capabilities,
                        now
                    ],
                )?;
            } else {
                connection.execute(
                    "UPDATE game_instances SET
                        connection_state='connected',runtime_instance_id=?2,remote_address=?3,
                        game_version=?4,platform=?5,runtime_capabilities_json=?6,
                        connected_at=?7,disconnected_at=NULL,last_seen_at=?7
                     WHERE id=?1",
                    rusqlite::params![
                        instance_id,
                        hello.runtime_instance_id,
                        remote_address.to_string(),
                        hello.game_version,
                        hello.platform,
                        capabilities,
                        now
                    ],
                )?;
            }
            Ok(())
        })?;
        Ok(ConnectionRegistration {
            instance_id,
            origin: origin.as_db().to_string(),
            log_session_id: String::new(),
            resume_after_sequence: 0,
            logs_enabled: true,
        })
    }

    pub fn mark_disconnected(&self, instance_id: &str) {
        let _ = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_instances SET connection_state='disconnected',
                    disconnected_at=?2,last_seen_at=?2 WHERE id=?1",
                rusqlite::params![instance_id, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        });
    }

    pub fn stop_all(&self) {
        let children = self
            .live
            .lock()
            .values()
            .map(|entry| entry.child.clone())
            .collect::<Vec<_>>();
        for child in children {
            let _ = child.lock().kill();
        }
    }

    pub fn repair_log_files(&self) -> AppResult<InstanceLogRepairReport> {
        let mut report = InstanceLogRepairReport {
            files_truncated: 0,
            bytes_reclaimed: 0,
        };
        if !self.paths.bootstrap_logs_dir.exists() {
            return Ok(report);
        }
        for entry in std::fs::read_dir(&self.paths.bootstrap_logs_dir)? {
            let path = entry?.path();
            if !path.is_file() {
                continue;
            }
            let size = std::fs::metadata(&path)?.len();
            if size > MAX_INSTANCE_LOG_BYTES {
                truncate_oversized_instance_log(&path)?;
                report.files_truncated += 1;
                report.bytes_reclaimed += size.saturating_sub(MAX_INSTANCE_LOG_BYTES);
            }
        }
        Ok(report)
    }

    fn wait_until_not_live(&self, id: &str, timeout: Duration) -> bool {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if !self.live.lock().contains_key(id) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        !self.live.lock().contains_key(id)
    }

    pub(crate) fn read(&self, id: &str) -> AppResult<GameInstance> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    &format!("{SELECT_INSTANCE} WHERE id=?1"),
                    [id],
                    map_instance,
                )
                .optional()?
                .ok_or_else(|| AppError::not_found("Game instance"))
        })
    }

    fn ensure_active_task(&self, id: &str) -> AppResult<()> {
        let status = self.database.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT status FROM development_tasks WHERE id=?1",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(Into::into)
        })?;
        match status.as_deref() {
            Some("active") => Ok(()),
            Some(_) => Err(AppError::validation(
                "Only active development tasks can launch instances.",
            )),
            None => Err(AppError::not_found("Task")),
        }
    }

    fn mark_launch_failed(&self, id: &str, reason: &str) -> AppResult<()> {
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE game_instances SET process_state='failed',connection_state='disconnected',
                    ended_at=?2,failure_reason=?3 WHERE id=?1",
                rusqlite::params![id, Utc::now().to_rfc3339(), reason],
            )?;
            Ok(())
        })
    }

    fn monitor(&self, id: String, child: Arc<Mutex<Box<dyn ChildWrapper>>>, log_path: PathBuf) {
        let database = self.database.clone();
        let live = self.live.clone();
        std::thread::spawn(move || {
            loop {
                let _ = truncate_oversized_instance_log(&log_path);
                match child.lock().try_wait() {
                    Ok(Some(status)) => {
                        if let Some(instance) = live.lock().remove(&id) {
                            instance.window.stop();
                        }
                        let _ = database.with_connection(|connection| {
                            connection.execute(
                                "UPDATE game_instances SET process_state='exited',
                                ended_at=?2,exit_code=?3 WHERE id=?1",
                                rusqlite::params![id, Utc::now().to_rfc3339(), status.code()],
                            )?;
                            Ok(())
                        });
                        break;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(500)),
                    Err(error) => {
                        if let Some(instance) = live.lock().remove(&id) {
                            instance.window.stop();
                        }
                        let _ = database.with_connection(|connection| {
                            connection.execute(
                                "UPDATE game_instances SET process_state='failed',
                                ended_at=?2,failure_reason=?3 WHERE id=?1",
                                rusqlite::params![id, Utc::now().to_rfc3339(), error.to_string()],
                            )?;
                            Ok(())
                        });
                        break;
                    }
                }
            }
        });
    }
}

fn truncate_oversized_instance_log(path: &Path) -> AppResult<bool> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() <= MAX_INSTANCE_LOG_BYTES {
        return Ok(false);
    }
    let mut log = OpenOptions::new().write(true).truncate(true).open(path)?;
    writeln!(
        log,
        "[ABYA Desktop Development Tool truncated an oversized instance log at {} bytes.]",
        metadata.len()
    )?;
    log.flush()?;
    Ok(true)
}

#[cfg(windows)]
fn request_graceful_exit(pid: Option<u32>) -> bool {
    use std::os::windows::process::CommandExt;

    let Some(pid) = pid else {
        return false;
    };
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .creation_flags(0x0800_0000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(windows))]
fn request_graceful_exit(_pid: Option<u32>) -> bool {
    false
}

fn external_name(hello: &GameHello) -> String {
    let name = hello.display_name.trim();
    if name.is_empty() {
        format!(
            "External {}",
            hello
                .runtime_instance_id
                .chars()
                .take(8)
                .collect::<String>()
        )
    } else {
        name.chars().take(80).collect()
    }
}

const SELECT_INSTANCE: &str = "SELECT
    id,task_id,origin,name,mode,host_instance_id,executable_path,launch_profile_json,
    sanitized_args_json,pid,process_state,connection_state,started_at,ended_at,
    exit_code,failure_reason,host_port,log_file_path,runtime_instance_id,remote_address,
    game_version,platform,runtime_capabilities_json,connected_at,disconnected_at,last_seen_at
    FROM game_instances";

fn map_instance(row: &rusqlite::Row<'_>) -> rusqlite::Result<GameInstance> {
    let origin: String = row.get(2)?;
    let mode = row
        .get::<_, Option<String>>(4)?
        .map(|value| LaunchMode::from_db(&value));
    let profile = row
        .get::<_, Option<String>>(7)?
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let sanitized_args = row
        .get::<_, Option<String>>(8)?
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?
        .unwrap_or_default();
    let process_state: String = row.get(10)?;
    let connection_state: String = row.get(11)?;
    let capabilities: String = row.get(22)?;
    Ok(GameInstance {
        id: row.get(0)?,
        task_id: row.get(1)?,
        origin: InstanceOrigin::from_db(&origin),
        name: row.get(3)?,
        mode,
        host_instance_id: row.get(5)?,
        executable_path: row.get(6)?,
        profile,
        sanitized_args,
        pid: row.get(9)?,
        process_state: ProcessState::from_db(&process_state),
        connection_state: super::ConnectionState::from_db(&connection_state),
        started_at: row.get(12)?,
        ended_at: row.get(13)?,
        exit_code: row.get(14)?,
        failure_reason: row.get(15)?,
        host_port: row.get(16)?,
        log_file_path: row.get(17)?,
        runtime_instance_id: row.get(18)?,
        remote_address: row.get(19)?,
        game_version: row.get(20)?,
        platform: row.get(21)?,
        runtime_capabilities: serde_json::from_str(&capabilities).unwrap_or_default(),
        connected_at: row.get(23)?,
        disconnected_at: row.get(24)?,
        last_seen_at: row.get(25)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn registers_and_reuses_external_log_source() {
        let (service, _) = test_service();
        let hello = external_hello();
        let first = service
            .register_connection(&hello, "192.168.1.50:50000".parse().unwrap())
            .unwrap();
        service.mark_disconnected(&first.instance_id);
        let second = service
            .register_connection(&hello, "192.168.1.50:50001".parse().unwrap())
            .unwrap();
        assert_eq!(first.instance_id, second.instance_id);
        assert_eq!(
            service.read(&first.instance_id).unwrap().origin,
            InstanceOrigin::External
        );
    }

    #[test]
    fn rejects_stopping_external_instance() {
        let (service, _) = test_service();
        let registration = service
            .register_connection(&external_hello(), "192.168.1.50:50000".parse().unwrap())
            .unwrap();
        let error = service.stop(&registration.instance_id).unwrap_err();
        assert_eq!(error.code, "validation");
    }

    #[test]
    fn truncates_an_oversized_instance_log_while_an_append_handle_is_open() {
        let root = std::env::temp_dir().join(format!("abya-instance-log-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("instance.log");
        fs::File::create(&path)
            .unwrap()
            .set_len(MAX_INSTANCE_LOG_BYTES + 1)
            .unwrap();
        let mut append = OpenOptions::new().append(true).open(&path).unwrap();

        assert!(truncate_oversized_instance_log(&path).unwrap());
        writeln!(append, "new output").unwrap();
        append.flush().unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("truncated an oversized instance log"));
        assert!(contents.contains("new output"));
        assert!(fs::metadata(&path).unwrap().len() < 1_024);
        let _ = fs::remove_dir_all(root);
    }

    fn test_service() -> (InstanceService, Database) {
        let root = std::env::temp_dir().join(format!("abya-instance-{}", Uuid::new_v4()));
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        fs::create_dir_all(&paths.data_dir).unwrap();
        let database = Database::open(&paths).unwrap();
        (
            InstanceService::new(database.clone(), paths, GameConnectionService::new()),
            database,
        )
    }

    fn external_hello() -> GameHello {
        GameHello {
            protocol_version: 1,
            runtime_instance_id: "runtime-external".into(),
            instance_id: None,
            display_name: "External Game".into(),
            game_version: "1.0".into(),
            platform: "Windows".into(),
            capabilities: vec!["logs".into(), "mcp".into()],
            log_session_id: "logs-1".into(),
        }
    }
}
