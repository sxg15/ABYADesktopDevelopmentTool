use crate::foundation::{AppError, AppResult, Database};
use crate::modules::game_connections::{GameConnectionService, LogBatch, LogBatchAck};
use chrono::Utc;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

const MAX_PERSISTED_EVENTS_PER_SESSION: i64 = 100_000;
const MAX_PERSISTED_EVENTS_GLOBAL: i64 = 100_000;
const MAX_RAW_EVENT_CHARS: usize = 16_384;
const MAX_MESSAGE_CHARS: usize = 4_096;
const MAX_FIELD_CHARS: usize = 512;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSession {
    pub id: String,
    pub instance_id: String,
    pub server_session_id: String,
    pub status: String,
    pub endpoint: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub latest_sequence: i64,
    pub dropped_events: i64,
    pub last_error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSource {
    pub instance_id: String,
    pub name: String,
    pub origin: String,
    pub connection_state: String,
    pub remote_address: String,
    pub game_version: String,
    pub platform: String,
    pub latest_session_id: String,
    pub latest_session_status: String,
    pub latest_sequence: i64,
}

#[derive(Debug, Clone)]
pub struct LogConnectionState {
    pub session_id: String,
    pub latest_sequence: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLogEvent {
    pub log_session_id: String,
    pub sequence: i64,
    pub utc: String,
    pub severity: String,
    pub provider: String,
    pub category: String,
    pub event_name: String,
    pub message: String,
    pub object_id: String,
    pub correlation_id: String,
    pub raw: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilter {
    pub session_id: String,
    pub severity: Option<String>,
    pub provider: Option<String>,
    pub event_name: Option<String>,
    pub contains: Option<String>,
    pub before_sequence: Option<i64>,
    pub limit: Option<u16>,
}

#[derive(Clone)]
pub struct LogService {
    database: Database,
    connections: GameConnectionService,
    legacy_overflow: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRepairReport {
    pub events_deleted: u64,
    pub sessions_closed: u64,
}

impl LogService {
    pub fn new(database: Database, connections: GameConnectionService) -> Self {
        let overflow = database
            .with_connection(|connection| {
                Ok(connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM log_events LIMIT 1 OFFSET ?1)",
                    [MAX_PERSISTED_EVENTS_GLOBAL],
                    |row| row.get::<_, bool>(0),
                )?)
            })
            .unwrap_or(true);
        Self {
            database,
            connections,
            legacy_overflow: Arc::new(AtomicBool::new(overflow)),
        }
    }

    pub fn list_sources(&self) -> AppResult<Vec<LogSource>> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT
                    instance.id,instance.name,instance.origin,instance.connection_state,
                    COALESCE(instance.remote_address,''),instance.game_version,instance.platform,
                    COALESCE(session.id,''),COALESCE(session.status,''),COALESCE(session.latest_sequence,0)
                 FROM game_instances instance
                 LEFT JOIN log_sessions session ON session.id=(
                    SELECT latest.id FROM log_sessions latest
                    WHERE latest.instance_id=instance.id
                    ORDER BY latest.started_at DESC LIMIT 1
                 )
                 ORDER BY CASE instance.origin WHEN 'managed' THEN 0 ELSE 1 END,
                    COALESCE(instance.connected_at,instance.started_at) DESC",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(LogSource {
                    instance_id: row.get(0)?,
                    name: row.get(1)?,
                    origin: row.get(2)?,
                    connection_state: row.get(3)?,
                    remote_address: row.get(4)?,
                    game_version: row.get(5)?,
                    platform: row.get(6)?,
                    latest_session_id: row.get(7)?,
                    latest_session_status: row.get(8)?,
                    latest_sequence: row.get(9)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn list_sessions(&self, instance_id: Option<&str>) -> AppResult<Vec<LogSession>> {
        self.database.with_connection(|connection| {
            let sql = if instance_id.is_some() {
                format!("{SELECT_SESSION} WHERE instance_id=?1 ORDER BY started_at DESC")
            } else {
                format!("{SELECT_SESSION} ORDER BY started_at DESC")
            };
            let mut statement = connection.prepare(&sql)?;
            if let Some(instance_id) = instance_id {
                let rows = statement.query_map([instance_id], map_session)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            } else {
                let rows = statement.query_map([], map_session)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            }
        })
    }

    pub fn connection_started(
        &self,
        instance_id: &str,
        server_session_id: &str,
        endpoint: &str,
    ) -> AppResult<LogConnectionState> {
        let server_session_id = if server_session_id.trim().is_empty() {
            format!("connection:{instance_id}")
        } else {
            server_session_id.trim().to_string()
        };
        if let Some(session) = self.database.with_connection(|connection| {
            connection
                .query_row(
                    &format!(
                        "{SELECT_SESSION} WHERE instance_id=?1 AND server_session_id=?2
                         ORDER BY started_at DESC LIMIT 1"
                    ),
                    rusqlite::params![instance_id, server_session_id],
                    map_session,
                )
                .optional()
                .map_err(Into::into)
        })? {
            let enabled = session.status != "paused";
            let status = if enabled { "streaming" } else { "paused" };
            self.database.with_connection(|connection| {
                connection.execute(
                    "UPDATE log_sessions SET status=?2,endpoint=?3,ended_at=NULL,last_error=''
                     WHERE id=?1",
                    rusqlite::params![session.id, status, endpoint],
                )?;
                Ok(())
            })?;
            return Ok(LogConnectionState {
                session_id: session.id,
                latest_sequence: session.latest_sequence,
                enabled,
            });
        }
        let session = LogSession {
            id: Uuid::new_v4().to_string(),
            instance_id: instance_id.to_string(),
            server_session_id,
            status: "streaming".into(),
            endpoint: endpoint.to_string(),
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            latest_sequence: 0,
            dropped_events: 0,
            last_error: String::new(),
        };
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO log_sessions(
                    id,instance_id,server_session_id,status,endpoint,started_at,
                    latest_sequence,dropped_events,last_error)
                 VALUES(?1,?2,?3,?4,?5,?6,0,0,'')",
                rusqlite::params![
                    session.id,
                    session.instance_id,
                    session.server_session_id,
                    session.status,
                    session.endpoint,
                    session.started_at
                ],
            )?;
            Ok(())
        })?;
        Ok(LogConnectionState {
            session_id: session.id,
            latest_sequence: 0,
            enabled: true,
        })
    }

    pub fn connection_stopped(&self, instance_id: &str) {
        let _ = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE log_sessions SET status='disconnected',ended_at=?2
                 WHERE instance_id=?1 AND status IN ('connecting','streaming','reconnecting')",
                rusqlite::params![instance_id, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        });
    }

    pub fn start(&self, instance_id: &str) -> AppResult<LogSession> {
        self.connections.set_logs_enabled(instance_id, true)?;
        self.database.with_connection(|connection| {
            let count = connection.execute(
                "UPDATE log_sessions SET status='streaming',ended_at=NULL,last_error=''
                 WHERE id=(SELECT id FROM log_sessions WHERE instance_id=?1
                    ORDER BY started_at DESC LIMIT 1)",
                [instance_id],
            )?;
            if count == 0 {
                return Err(AppError::not_found("Log session"));
            }
            Ok(())
        })?;
        self.latest_session(instance_id)?
            .ok_or_else(|| AppError::not_found("Log session"))
    }

    pub fn stop(&self, instance_id: &str) -> AppResult<()> {
        self.connections.set_logs_enabled(instance_id, false)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE log_sessions SET status='paused'
                 WHERE id=(SELECT id FROM log_sessions WHERE instance_id=?1
                    ORDER BY started_at DESC LIMIT 1)
                   AND status IN ('connecting','streaming','reconnecting','disconnected')",
                [instance_id],
            )?;
            Ok(())
        })
    }

    pub fn ingest_batch(&self, instance_id: &str, batch: &LogBatch) -> AppResult<LogBatchAck> {
        if self.legacy_overflow.load(Ordering::Acquire) {
            return Err(AppError::new(
                "storageRepairRequired",
                "Historical log storage exceeds the global limit. Use Settings > Repair storage before collecting more logs.",
                "",
            ));
        }
        let session = self
            .latest_session(instance_id)?
            .ok_or_else(|| AppError::not_found("Log session"))?;
        if session.status == "paused" {
            return Err(AppError::validation(
                "Log collection is paused for this instance.",
            ));
        }
        if !batch.session_id.is_empty() && batch.session_id != session.server_session_id {
            return Err(AppError::new(
                "logSessionMismatch",
                "The log batch belongs to a different runtime session.",
                &batch.session_id,
            ));
        }
        let mut accepted_events = 0;
        let mut latest_sequence = session.latest_sequence;
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let (status, cursor): (String, i64) = transaction.query_row(
                "SELECT status,latest_sequence FROM log_sessions WHERE id=?1",
                [&session.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if status == "paused" {
                return Err(AppError::validation("Log collection is paused."));
            }
            latest_sequence = latest_sequence.max(cursor);
            for value in &batch.events {
                if let Some(sequence) = event_sequence(value) {
                    if sequence <= cursor {
                        continue;
                    }
                    let inserted = persist_event(&transaction, &session.id, value)?;
                    accepted_events += inserted;
                    latest_sequence = latest_sequence.max(sequence);
                }
            }
            transaction.execute(
                "UPDATE log_sessions SET latest_sequence=MAX(latest_sequence,?2),
                    dropped_events=dropped_events+?3,status='streaming',last_error=''
                 WHERE id=?1",
                rusqlite::params![session.id, latest_sequence, batch.dropped_events.max(0)],
            )?;
            prune_session_events(
                &transaction,
                &session.id,
                latest_sequence,
                MAX_PERSISTED_EVENTS_PER_SESSION,
            )?;
            prune_global_events(&transaction, MAX_PERSISTED_EVENTS_GLOBAL)?;
            transaction.commit()?;
            Ok(())
        })?;
        Ok(LogBatchAck {
            latest_sequence,
            accepted_events,
        })
    }

    pub fn delete_session(&self, id: &str) -> AppResult<()> {
        self.database.with_connection(|connection| {
            let status = connection
                .query_row("SELECT status FROM log_sessions WHERE id=?1", [id], |row| {
                    row.get::<_, String>(0)
                })
                .optional()?
                .ok_or_else(|| AppError::not_found("Log session"))?;
            if matches!(status.as_str(), "connecting" | "streaming" | "reconnecting") {
                return Err(AppError::validation(
                    "Pause or disconnect log collection before deleting the session.",
                ));
            }
            connection.execute("DELETE FROM log_sessions WHERE id=?1", [id])?;
            Ok(())
        })
    }

    pub fn repair_storage(&self) -> AppResult<LogRepairReport> {
        self.database.with_connection(|connection| {
            let sessions_closed = connection.execute(
                "UPDATE log_sessions SET status='paused', ended_at=COALESCE(ended_at, datetime('now'))
                 WHERE status<>'paused'", [])? as u64;
            let mut events_deleted = 0u64;
            loop {
                let deleted = connection.execute(
                    "DELETE FROM log_events WHERE rowid IN (SELECT rowid FROM log_events LIMIT 500)", [])?;
                events_deleted += deleted as u64;
                connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
                if deleted == 0 { break; }
            }
            self.legacy_overflow.store(false, Ordering::Release);
            Ok(LogRepairReport { events_deleted, sessions_closed })
        })
    }

    pub fn query(&self, filter: LogFilter) -> AppResult<Vec<RuntimeLogEvent>> {
        if filter.session_id.trim().is_empty() {
            return Err(AppError::validation("Select a log session."));
        }
        let limit = filter.limit.unwrap_or(200).clamp(1, 1000);
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT log_session_id,sequence,utc,severity,provider,category,event_name,
                        message,object_id,correlation_id,raw_json
                 FROM log_events
                 WHERE log_session_id=?1
                   AND (?2 IS NULL OR severity=?2)
                   AND (?3 IS NULL OR provider=?3)
                   AND (?4 IS NULL OR event_name=?4)
                   AND (?5 IS NULL OR message LIKE '%' || ?5 || '%')
                   AND (?6 IS NULL OR sequence<?6)
                 ORDER BY sequence DESC LIMIT ?7",
            )?;
            let rows = statement.query_map(
                rusqlite::params![
                    filter.session_id,
                    empty_to_none(filter.severity),
                    empty_to_none(filter.provider),
                    empty_to_none(filter.event_name),
                    empty_to_none(filter.contains),
                    filter.before_sequence,
                    limit
                ],
                map_event,
            )?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    fn latest_session(&self, instance_id: &str) -> AppResult<Option<LogSession>> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    &format!(
                        "{SELECT_SESSION} WHERE instance_id=?1 ORDER BY started_at DESC LIMIT 1"
                    ),
                    [instance_id],
                    map_session,
                )
                .optional()
                .map_err(Into::into)
        })
    }
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.filter(|item| !item.trim().is_empty())
}

fn event_sequence(value: &Value) -> Option<i64> {
    value.get("sequence").and_then(Value::as_i64)
}

fn persist_event(
    connection: &rusqlite::Connection,
    session_id: &str,
    value: &Value,
) -> AppResult<usize> {
    let sequence = event_sequence(value)
        .ok_or_else(|| AppError::validation("Log events require an integer sequence."))?;
    let object_id = value
        .pointer("/object/id")
        .or_else(|| value.pointer("/object/instanceId"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let correlation_id = value
        .pointer("/correlation/id")
        .or_else(|| value.pointer("/correlation/correlationId"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let inserted = connection.execute(
        "INSERT OR IGNORE INTO log_events(
            log_session_id,sequence,utc,severity,provider,category,event_name,message,
            object_id,correlation_id,raw_json)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        rusqlite::params![
            session_id,
            sequence,
            bounded_field(value, "utc", MAX_FIELD_CHARS),
            bounded_field(value, "severity", MAX_FIELD_CHARS),
            bounded_field(value, "provider", MAX_FIELD_CHARS),
            bounded_field(value, "category", MAX_FIELD_CHARS),
            bounded_field(value, "eventName", MAX_FIELD_CHARS),
            bounded_field(value, "message", MAX_MESSAGE_CHARS),
            bounded(object_id, MAX_FIELD_CHARS),
            bounded(correlation_id, MAX_FIELD_CHARS),
            bounded_raw_event(value)
        ],
    )?;
    Ok(inserted)
}

fn prune_session_events(
    connection: &rusqlite::Connection,
    session_id: &str,
    latest_sequence: i64,
    max_events: i64,
) -> AppResult<usize> {
    let minimum_sequence = latest_sequence.saturating_sub(max_events.saturating_sub(1));
    Ok(connection.execute(
        "DELETE FROM log_events WHERE log_session_id=?1 AND sequence<?2",
        rusqlite::params![session_id, minimum_sequence],
    )?)
}

fn bounded_field(value: &Value, key: &str, max_chars: usize) -> String {
    bounded(
        value.get(key).and_then(Value::as_str).unwrap_or_default(),
        max_chars,
    )
}

fn bounded_raw_event(value: &Value) -> String {
    let encoded = value.to_string();
    if encoded.len() <= MAX_RAW_EVENT_CHARS {
        encoded
    } else {
        serde_json::json!({
            "truncated": true,
            "reason": "Runtime log event exceeded the persistence limit."
        })
        .to_string()
    }
}

fn bounded(value: &str, max_chars: usize) -> String {
    let mut end = value.len().min(max_chars);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn prune_global_events(connection: &rusqlite::Connection, limit: i64) -> AppResult<usize> {
    Ok(connection.execute(
        "DELETE FROM log_events WHERE rowid <= (SELECT COALESCE(MAX(rowid),0)-?1 FROM log_events)",
        [limit],
    )?)
}

const SELECT_SESSION: &str = "SELECT
    id,instance_id,server_session_id,status,endpoint,started_at,ended_at,
    latest_sequence,dropped_events,last_error FROM log_sessions";

fn map_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<LogSession> {
    Ok(LogSession {
        id: row.get(0)?,
        instance_id: row.get(1)?,
        server_session_id: row.get(2)?,
        status: row.get(3)?,
        endpoint: row.get(4)?,
        started_at: row.get(5)?,
        ended_at: row.get(6)?,
        latest_sequence: row.get(7)?,
        dropped_events: row.get(8)?,
        last_error: row.get(9)?,
    })
}

fn map_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<RuntimeLogEvent> {
    let raw: String = row.get(10)?;
    Ok(RuntimeLogEvent {
        log_session_id: row.get(0)?,
        sequence: row.get(1)?,
        utc: row.get(2)?,
        severity: row.get(3)?,
        provider: row.get(4)?,
        category: row.get(5)?,
        event_name: row.get(6)?,
        message: row.get(7)?,
        object_id: row.get(8)?,
        correlation_id: row.get(9)?,
        raw: serde_json::from_str(&raw).unwrap_or(Value::Null),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::AppPaths;
    use crate::modules::game_connections::GameConnectionService;
    use std::fs;

    #[test]
    fn persists_before_ack_and_deduplicates_replayed_events() {
        let (service, database) = test_service();
        seed_external(&database);
        service
            .connection_started("instance", "runtime-session", "ws://192.168.1.5")
            .unwrap();
        let batch = LogBatch {
            batch_id: "batch".into(),
            session_id: "runtime-session".into(),
            events: vec![serde_json::json!({
                "sequence": 1,
                "utc": "2026-08-26T00:00:00Z",
                "severity": "Info",
                "provider": "test",
                "category": "test",
                "eventName": "ready",
                "message": "ready"
            })],
            dropped_events: 0,
        };
        assert_eq!(
            service
                .ingest_batch("instance", &batch)
                .unwrap()
                .accepted_events,
            1
        );
        assert_eq!(
            service
                .ingest_batch("instance", &batch)
                .unwrap()
                .accepted_events,
            0
        );
        let count: i64 = database
            .with_connection(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM log_events", [], |row| row.get(0))
                    .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn prunes_events_older_than_the_per_session_window() {
        let (service, database) = test_service();
        seed_external(&database);
        let session = service
            .connection_started("instance", "runtime-session", "ws://192.168.1.5")
            .unwrap();
        database
            .with_connection(|connection| {
                for sequence in 1..=5 {
                    persist_event(
                        connection,
                        &session.session_id,
                        &serde_json::json!({ "sequence": sequence }),
                    )?;
                }
                assert_eq!(
                    prune_session_events(connection, &session.session_id, 5, 3)?,
                    2
                );
                let sequences = connection
                    .prepare(
                        "SELECT sequence FROM log_events WHERE log_session_id=?1 ORDER BY sequence",
                    )?
                    .query_map([&session.session_id], |row| row.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(sequences, vec![3, 4, 5]);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn bounds_large_runtime_log_fields_and_preserves_valid_json() {
        let (service, database) = test_service();
        seed_external(&database);
        let session = service
            .connection_started("instance", "runtime-session", "ws://192.168.1.5")
            .unwrap();
        database
            .with_connection(|connection| {
                persist_event(
                    connection,
                    &session.session_id,
                    &serde_json::json!({
                        "sequence": 1,
                        "message": "m".repeat(MAX_MESSAGE_CHARS + 10),
                        "payload": "x".repeat(MAX_RAW_EVENT_CHARS + 10)
                    }),
                )?;
                let (message, raw): (String, String) = connection.query_row(
                    "SELECT message,raw_json FROM log_events WHERE log_session_id=?1",
                    [&session.session_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(message.chars().count(), MAX_MESSAGE_CHARS);
                assert_eq!(
                    serde_json::from_str::<Value>(&raw)?["truncated"],
                    Value::Bool(true)
                );
                Ok(())
            })
            .unwrap();
    }

    fn test_service() -> (LogService, Database) {
        let root = std::env::temp_dir().join(format!("abya-logs-{}", Uuid::new_v4()));
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
            LogService::new(database.clone(), GameConnectionService::new()),
            database,
        )
    }

    fn seed_external(database: &Database) {
        database
            .with_connection(|connection| {
                connection.execute(
                    "INSERT INTO game_instances(
                        id,origin,name,process_state,connection_state)
                     VALUES('instance','external','External','unmanaged','connected')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
    }
}
