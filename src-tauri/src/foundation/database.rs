use super::{AppPaths, AppResult};
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Arc;

const WAL_AUTOCHECKPOINT_PAGES: i64 = 1_000;
const WAL_JOURNAL_SIZE_LIMIT_BYTES: i64 = 64 * 1024 * 1024;
const SMALL_DATABASE_BYTES: u64 = 64 * 1024 * 1024;
const VACUUM_MIN_FREE_BYTES: u64 = 256 * 1024 * 1024;
const VACUUM_MIN_FREE_PERCENT: u64 = 25;

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRepairReport {
    pub database_bytes_before: u64,
    pub database_bytes_after: u64,
    pub wal_bytes_after: u64,
    pub free_pages_after: u64,
    pub quick_check: String,
}

impl Database {
    pub fn open(paths: &AppPaths) -> AppResult<Self> {
        let connection = Connection::open(&paths.database_path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let page_count = pragma_i64(&connection, "page_count")?;
        if page_count == 0 {
            connection.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
        }
        configure_connection(&connection)?;
        maintain_connection_storage(&connection)?;
        let database = Self {
            connection: Arc::new(Mutex::new(connection)),
        };
        database.migrate()?;
        database.mark_interrupted_instances()?;
        database.maintain_storage()?;
        Ok(database)
    }

    pub fn with_connection<T>(
        &self,
        action: impl FnOnce(&Connection) -> AppResult<T>,
    ) -> AppResult<T> {
        let connection = self.connection.lock();
        action(&connection)
    }

    fn migrate(&self) -> AppResult<()> {
        self.with_connection(|connection| {
            connection.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY,
                    value_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS development_tasks (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL CHECK(status IN ('active','completed','archived')),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    completed_at TEXT,
                    archived_at TEXT,
                    workspace_path TEXT NOT NULL DEFAULT ''
                );
                "#,
            )?;
            if !table_has_column(connection, "development_tasks", "workspace_path")? {
                connection.execute(
                    "ALTER TABLE development_tasks ADD COLUMN workspace_path TEXT NOT NULL DEFAULT ''",
                    [],
                )?;
            }
            if table_exists(connection, "game_instances")?
                && !table_has_column(connection, "game_instances", "origin")?
            {
                migrate_legacy_runtime_tables(connection)?;
            }
            create_runtime_tables(connection)?;
            Ok(())
        })
    }

    fn mark_interrupted_instances(&self) -> AppResult<()> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE game_instances
                 SET process_state='interrupted', ended_at=datetime('now'),
                     failure_reason='Application ended while the instance was running.'
                 WHERE origin='managed' AND process_state IN ('launching','running','stopping')",
                [],
            )?;
            connection.execute(
                "UPDATE game_instances
                 SET connection_state='disconnected',disconnected_at=datetime('now')
                 WHERE connection_state IN ('connected','waiting')",
                [],
            )?;
            connection.execute(
                "UPDATE log_sessions SET status='interrupted', ended_at=datetime('now')
                 WHERE status IN ('connecting','streaming','reconnecting')",
                [],
            )?;
            connection.execute(
                "UPDATE archive_transfers
                 SET status='interrupted',error_code='applicationRestarted',
                     error_message='The desktop tool stopped during the archive transfer.',
                     updated_at=datetime('now'),completed_at=datetime('now')
                 WHERE status IN ('preparing','waitingAcceptance','transferring','finalizing')",
                [],
            )?;
            Ok(())
        })
    }

    fn maintain_storage(&self) -> AppResult<()> {
        self.with_connection(maintain_connection_storage)
    }

    pub fn repair_storage(&self, paths: &AppPaths) -> AppResult<DatabaseRepairReport> {
        let before = std::fs::metadata(&paths.database_path)
            .map(|m| m.len())
            .unwrap_or(0);
        self.with_connection(|connection| {
            checkpoint_and_truncate_wal(connection)?;
            if pragma_i64(connection, "auto_vacuum")? == 0 {
                convert_to_incremental_vacuum(connection)?;
            } else {
                // An explicit repair may rebuild the compact database in one pass.
                // Incremental vacuum can reclaim only a page at a time on heavily
                // fragmented legacy files and would make the UI appear hung.
                connection.execute_batch("VACUUM;")?;
            }
            connection.execute_batch("PRAGMA optimize;")?;
            checkpoint_and_truncate_wal(connection)?;
            let free_pages = pragma_i64(connection, "freelist_count")?.max(0) as u64;
            let quick_check: String =
                connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            if quick_check != "ok" {
                return Err(crate::foundation::AppError::new(
                    "storageCorrupt",
                    "Storage integrity check failed.",
                    quick_check,
                ));
            }
            Ok(DatabaseRepairReport {
                database_bytes_before: before,
                database_bytes_after: std::fs::metadata(&paths.database_path)
                    .map(|m| m.len())
                    .unwrap_or(0),
                wal_bytes_after: std::fs::metadata(paths.database_path.with_extension("db-wal"))
                    .map(|m| m.len())
                    .unwrap_or(0),
                free_pages_after: free_pages,
                quick_check,
            })
        })
    }
}

fn drain_incremental_vacuum(connection: &Connection, pages: u32) -> AppResult<()> {
    connection.execute_batch(&format!("PRAGMA incremental_vacuum({pages});"))?;
    Ok(())
}

fn maintain_connection_storage(connection: &Connection) -> AppResult<()> {
    checkpoint_and_truncate_wal(connection)?;
    connection.execute_batch("PRAGMA optimize;")?;

    let page_count = pragma_i64(connection, "page_count")?.max(0) as u64;
    let free_pages = pragma_i64(connection, "freelist_count")?.max(0) as u64;
    let page_size = pragma_i64(connection, "page_size")?.max(0) as u64;
    let auto_vacuum = pragma_i64(connection, "auto_vacuum")?;
    if auto_vacuum == 0
        && page_count.saturating_mul(page_size) <= SMALL_DATABASE_BYTES
        && should_convert_to_incremental_vacuum(page_count, free_pages, page_size)
    {
        convert_to_incremental_vacuum(connection)?;
    } else if auto_vacuum == 2 && free_pages > 0 {
        drain_incremental_vacuum(connection, 1024)?;
    }

    checkpoint_and_truncate_wal(connection)
}

fn configure_connection(connection: &Connection) -> AppResult<()> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "wal_autocheckpoint", WAL_AUTOCHECKPOINT_PAGES)?;
    connection.pragma_update(None, "journal_size_limit", WAL_JOURNAL_SIZE_LIMIT_BYTES)?;
    Ok(())
}

fn pragma_i64(connection: &Connection, name: &str) -> AppResult<i64> {
    Ok(connection.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?)
}

fn checkpoint_and_truncate_wal(connection: &Connection) -> AppResult<()> {
    let busy: i64 =
        connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
    if busy != 0 {
        return Err(crate::foundation::AppError::validation(
            "The database is busy in another tool copy. Close it and retry storage repair.",
        ));
    }
    Ok(())
}

fn should_convert_to_incremental_vacuum(page_count: u64, free_pages: u64, page_size: u64) -> bool {
    if page_count == 0 || page_size == 0 {
        return true;
    }
    let database_bytes = page_count.saturating_mul(page_size);
    let free_bytes = free_pages.saturating_mul(page_size);
    database_bytes <= SMALL_DATABASE_BYTES
        || (free_bytes >= VACUUM_MIN_FREE_BYTES
            && free_pages.saturating_mul(100) / page_count >= VACUUM_MIN_FREE_PERCENT)
}

fn convert_to_incremental_vacuum(connection: &Connection) -> AppResult<()> {
    checkpoint_and_truncate_wal(connection)?;
    connection.pragma_update(None, "journal_mode", "DELETE")?;
    let vacuum_result = (|| {
        connection.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
        connection.execute_batch("VACUUM;")
    })();
    let wal_result = connection.pragma_update(None, "journal_mode", "WAL");
    if let Err(error) = vacuum_result {
        return Err(error.into());
    }
    wal_result?;
    configure_connection(connection)
}

fn table_exists(connection: &Connection, table: &str) -> AppResult<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn table_has_column(connection: &Connection, table: &str, column: &str) -> AppResult<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for value in columns {
        if value? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn create_runtime_tables(connection: &Connection) -> AppResult<()> {
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS game_instances (
            id TEXT PRIMARY KEY,
            task_id TEXT REFERENCES development_tasks(id) ON DELETE CASCADE,
            origin TEXT NOT NULL CHECK(origin IN ('managed','external')),
            name TEXT NOT NULL,
            mode TEXT,
            host_instance_id TEXT,
            executable_path TEXT,
            launch_profile_json TEXT,
            sanitized_args_json TEXT,
            pid INTEGER,
            process_state TEXT NOT NULL,
            connection_state TEXT NOT NULL,
            started_at TEXT,
            ended_at TEXT,
            exit_code INTEGER,
            failure_reason TEXT NOT NULL DEFAULT '',
            host_port INTEGER,
            log_file_path TEXT,
            runtime_instance_id TEXT,
            remote_address TEXT,
            game_version TEXT NOT NULL DEFAULT '',
            platform TEXT NOT NULL DEFAULT '',
            runtime_capabilities_json TEXT NOT NULL DEFAULT '[]',
            connected_at TEXT,
            disconnected_at TEXT,
            last_seen_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_game_instances_task
            ON game_instances(task_id, started_at DESC);
        CREATE INDEX IF NOT EXISTS idx_game_instances_runtime
            ON game_instances(runtime_instance_id, origin);
        CREATE INDEX IF NOT EXISTS idx_game_instances_origin
            ON game_instances(origin, connected_at DESC);

        CREATE TABLE IF NOT EXISTS log_sessions (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL REFERENCES game_instances(id) ON DELETE CASCADE,
            server_session_id TEXT NOT NULL,
            status TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            latest_sequence INTEGER NOT NULL DEFAULT 0,
            dropped_events INTEGER NOT NULL DEFAULT 0,
            last_error TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_log_sessions_instance
            ON log_sessions(instance_id, started_at DESC);

        CREATE TABLE IF NOT EXISTS log_events (
            log_session_id TEXT NOT NULL REFERENCES log_sessions(id) ON DELETE CASCADE,
            sequence INTEGER NOT NULL,
            utc TEXT NOT NULL,
            severity TEXT NOT NULL,
            provider TEXT NOT NULL,
            category TEXT NOT NULL,
            event_name TEXT NOT NULL,
            message TEXT NOT NULL,
            object_id TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            raw_json TEXT NOT NULL,
            PRIMARY KEY(log_session_id, sequence)
        );
        CREATE INDEX IF NOT EXISTS idx_log_events_query
            ON log_events(log_session_id, sequence DESC, severity, provider, event_name);

        CREATE TABLE IF NOT EXISTS archive_transfers (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL REFERENCES game_instances(id) ON DELETE CASCADE,
            source_main_path TEXT NOT NULL,
            archive_path TEXT NOT NULL,
            archive_guid TEXT NOT NULL,
            archive_name TEXT NOT NULL,
            author TEXT NOT NULL DEFAULT '',
            file_count INTEGER NOT NULL,
            uncompressed_bytes INTEGER NOT NULL,
            package_bytes INTEGER NOT NULL DEFAULT 0,
            acknowledged_bytes INTEGER NOT NULL DEFAULT 0,
            sha256 TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL,
            phase TEXT NOT NULL DEFAULT '',
            progress_percent REAL NOT NULL DEFAULT 0,
            installed_path TEXT NOT NULL DEFAULT '',
            error_code TEXT NOT NULL DEFAULT '',
            error_message TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_archive_transfers_instance
            ON archive_transfers(instance_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_archive_transfers_status
            ON archive_transfers(status, updated_at DESC);
        "#,
    )?;
    Ok(())
}

fn migrate_legacy_runtime_tables(connection: &Connection) -> AppResult<()> {
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    let result = connection.execute_batch(
        r#"
        BEGIN IMMEDIATE;
        DROP INDEX IF EXISTS idx_log_events_query;
        DROP INDEX IF EXISTS idx_log_sessions_instance;
        DROP INDEX IF EXISTS idx_game_instances_task;
        ALTER TABLE log_events RENAME TO log_events_legacy;
        ALTER TABLE log_sessions RENAME TO log_sessions_legacy;
        ALTER TABLE game_instances RENAME TO game_instances_legacy;

        CREATE TABLE game_instances (
            id TEXT PRIMARY KEY,
            task_id TEXT REFERENCES development_tasks(id) ON DELETE CASCADE,
            origin TEXT NOT NULL CHECK(origin IN ('managed','external')),
            name TEXT NOT NULL,
            mode TEXT,
            host_instance_id TEXT,
            executable_path TEXT,
            launch_profile_json TEXT,
            sanitized_args_json TEXT,
            pid INTEGER,
            process_state TEXT NOT NULL,
            connection_state TEXT NOT NULL,
            started_at TEXT,
            ended_at TEXT,
            exit_code INTEGER,
            failure_reason TEXT NOT NULL DEFAULT '',
            host_port INTEGER,
            log_file_path TEXT,
            runtime_instance_id TEXT,
            remote_address TEXT,
            game_version TEXT NOT NULL DEFAULT '',
            platform TEXT NOT NULL DEFAULT '',
            runtime_capabilities_json TEXT NOT NULL DEFAULT '[]',
            connected_at TEXT,
            disconnected_at TEXT,
            last_seen_at TEXT
        );
        INSERT INTO game_instances(
            id,task_id,origin,name,mode,host_instance_id,executable_path,
            launch_profile_json,sanitized_args_json,pid,process_state,connection_state,
            started_at,ended_at,exit_code,failure_reason,host_port,log_file_path)
        SELECT
            id,task_id,'managed',name,mode,host_instance_id,executable_path,
            launch_profile_json,sanitized_args_json,pid,state,'disconnected',
            started_at,ended_at,exit_code,failure_reason,host_port,log_file_path
        FROM game_instances_legacy;

        CREATE TABLE log_sessions (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL REFERENCES game_instances(id) ON DELETE CASCADE,
            server_session_id TEXT NOT NULL,
            status TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            latest_sequence INTEGER NOT NULL DEFAULT 0,
            dropped_events INTEGER NOT NULL DEFAULT 0,
            last_error TEXT NOT NULL DEFAULT ''
        );
        INSERT INTO log_sessions
        SELECT * FROM log_sessions_legacy;

        CREATE TABLE log_events (
            log_session_id TEXT NOT NULL REFERENCES log_sessions(id) ON DELETE CASCADE,
            sequence INTEGER NOT NULL,
            utc TEXT NOT NULL,
            severity TEXT NOT NULL,
            provider TEXT NOT NULL,
            category TEXT NOT NULL,
            event_name TEXT NOT NULL,
            message TEXT NOT NULL,
            object_id TEXT NOT NULL,
            correlation_id TEXT NOT NULL,
            raw_json TEXT NOT NULL,
            PRIMARY KEY(log_session_id, sequence)
        );
        INSERT INTO log_events
        SELECT * FROM log_events_legacy;

        DROP TABLE log_events_legacy;
        DROP TABLE log_sessions_legacy;
        DROP TABLE game_instances_legacy;
        COMMIT;
        "#,
    );
    connection.pragma_update(None, "foreign_keys", "ON")?;
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::AppPaths;
    use uuid::Uuid;

    #[test]
    fn migrates_legacy_instances_without_changing_ids() {
        let root = std::env::temp_dir().join(format!("abya-db-migration-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let connection = Connection::open(&paths.database_path).unwrap();
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys=ON;
                CREATE TABLE development_tasks (
                    id TEXT PRIMARY KEY,title TEXT NOT NULL,description TEXT NOT NULL,
                    status TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,
                    completed_at TEXT,archived_at TEXT);
                INSERT INTO development_tasks VALUES('task','Task','','active','now','now',NULL,NULL);
                CREATE TABLE game_instances (
                    id TEXT PRIMARY KEY,task_id TEXT NOT NULL,name TEXT NOT NULL,mode TEXT NOT NULL,
                    host_instance_id TEXT,executable_path TEXT NOT NULL,launch_profile_json TEXT NOT NULL,
                    sanitized_args_json TEXT NOT NULL,pid INTEGER,state TEXT NOT NULL,started_at TEXT,
                    ended_at TEXT,exit_code INTEGER,failure_reason TEXT NOT NULL DEFAULT '',
                    mcp_port INTEGER NOT NULL,host_port INTEGER,log_file_path TEXT NOT NULL);
                INSERT INTO game_instances VALUES(
                    'instance','task','Game','normal',NULL,'game.exe','{}','[]',NULL,'exited',
                    'now',NULL,0,'',48100,NULL,'game.log');
                CREATE TABLE log_sessions (
                    id TEXT PRIMARY KEY,instance_id TEXT NOT NULL,server_session_id TEXT NOT NULL,
                    status TEXT NOT NULL,endpoint TEXT NOT NULL,started_at TEXT NOT NULL,ended_at TEXT,
                    latest_sequence INTEGER NOT NULL DEFAULT 0,dropped_events INTEGER NOT NULL DEFAULT 0,
                    last_error TEXT NOT NULL DEFAULT '');
                INSERT INTO log_sessions VALUES('session','instance','runtime','stopped','old','now',NULL,1,0,'');
                CREATE TABLE log_events (
                    log_session_id TEXT NOT NULL,sequence INTEGER NOT NULL,utc TEXT NOT NULL,
                    severity TEXT NOT NULL,provider TEXT NOT NULL,category TEXT NOT NULL,
                    event_name TEXT NOT NULL,message TEXT NOT NULL,object_id TEXT NOT NULL,
                    correlation_id TEXT NOT NULL,raw_json TEXT NOT NULL,
                    PRIMARY KEY(log_session_id,sequence));
                INSERT INTO log_events VALUES('session',1,'now','Info','test','test','event','message','','','{}');
                "#,
            )
            .unwrap();
        drop(connection);

        let database = Database::open(&paths).unwrap();
        database
            .with_connection(|connection| {
                let row: (String, String, String) = connection.query_row(
                    "SELECT id,origin,process_state FROM game_instances",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                assert_eq!(row, ("instance".into(), "managed".into(), "exited".into()));
                let event_count: i64 =
                    connection
                        .query_row("SELECT COUNT(*) FROM log_events", [], |row| row.get(0))?;
                assert_eq!(event_count, 1);
                Ok(())
            })
            .unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn new_databases_use_bounded_wal_and_incremental_vacuum() {
        let root = std::env::temp_dir().join(format!("abya-db-storage-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let database = Database::open(&paths).unwrap();
        database
            .with_connection(|connection| {
                assert_eq!(pragma_i64(connection, "auto_vacuum")?, 2);
                assert_eq!(
                    pragma_i64(connection, "journal_size_limit")?,
                    WAL_JOURNAL_SIZE_LIMIT_BYTES
                );
                assert_eq!(
                    pragma_i64(connection, "wal_autocheckpoint")?,
                    WAL_AUTOCHECKPOINT_PAGES
                );
                Ok(())
            })
            .unwrap();
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn vacuum_conversion_targets_small_or_severely_sparse_databases() {
        assert!(should_convert_to_incremental_vacuum(10, 0, 4_096));
        assert!(should_convert_to_incremental_vacuum(200_000, 70_000, 4_096));
        assert!(!should_convert_to_incremental_vacuum(
            200_000, 10_000, 4_096
        ));
    }
}
