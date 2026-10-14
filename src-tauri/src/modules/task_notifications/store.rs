use super::model::AttentionTarget;
use crate::foundation::{AppResult, Database};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};

#[derive(Clone)]
pub struct NotificationStore(pub Database);

impl NotificationStore {
    pub fn enabled(&self) -> AppResult<bool> {
        self.0.with_connection(|c| {
            let value: Option<String> = c
                .query_row(
                    "SELECT value_json FROM settings WHERE key='taskNotifications'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(value
                .map(|v| serde_json::from_str::<serde_json::Value>(&v))
                .transpose()?
                .and_then(|v| v["enabled"].as_bool())
                .unwrap_or(true))
        })
    }

    pub fn set_enabled(&self, enabled: bool) -> AppResult<()> {
        self.0.with_connection(|c| {
            c.execute(
                "INSERT INTO settings(key,value_json,updated_at) VALUES('taskNotifications',?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",
                params![serde_json::json!({"enabled":enabled}).to_string(),Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    /// Reserve before calling Windows: an unknown delivery outcome must not be replayed.
    pub fn observe(
        &self,
        key: &str,
        target: &AttentionTarget,
        disposition: &str,
    ) -> AppResult<bool> {
        self.0.with_connection(|c| {
            Ok(c.execute(
                "INSERT OR IGNORE INTO task_notification_receipts(event_key,task_id,target_json,disposition,observed_at)
                 VALUES(?1,?2,?3,?4,?5)",
                params![key,target.task_id,serde_json::to_string(target)?,disposition,Utc::now().to_rfc3339()],
            )? == 1)
        })
    }

    pub fn delivered(&self, key: &str, shown: bool) -> AppResult<()> {
        self.0.with_connection(|c| {
            c.execute(
                "UPDATE task_notification_receipts SET active=?2,disposition=?3 WHERE event_key=?1",
                params![key, shown, if shown { "notified" } else { "foreground" }],
            )?;
            Ok(())
        })
    }

    pub fn active(&self) -> AppResult<Vec<(String, String)>> {
        self.0.with_connection(|c| {
            let mut query = c.prepare(
                "SELECT event_key,task_id FROM task_notification_receipts WHERE active=1",
            )?;
            Ok(query
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?)
        })
    }

    pub fn retire(&self, key: &str) -> AppResult<()> {
        self.0.with_connection(|c| {
            c.execute(
                "UPDATE task_notification_receipts SET active=0 WHERE event_key=?1",
                [key],
            )?;
            Ok(())
        })
    }

    pub fn reset_active(&self) -> AppResult<()> {
        self.0.with_connection(|c| {
            c.execute(
                "UPDATE task_notification_receipts SET active=0 WHERE active=1",
                [],
            )?;
            Ok(())
        })
    }

    pub fn target(&self, key: &str) -> AppResult<Option<AttentionTarget>> {
        self.0.with_connection(|c| {
            let json: Option<String> = c
                .query_row(
                    "SELECT target_json FROM task_notification_receipts WHERE event_key=?1",
                    [key],
                    |r| r.get(0),
                )
                .optional()?;
            json.map(|v| serde_json::from_str(&v).map_err(Into::into))
                .transpose()
        })
    }
}
