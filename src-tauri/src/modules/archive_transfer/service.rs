use super::models::{
    ArchiveTransferRecord, ArchiveTransferStatus, ArchiveTransferTarget, StartArchiveTransferInput,
};
use super::package::{ArchivePackage, build_package};
use crate::foundation::{AppError, AppPaths, AppResult, Database};
use crate::modules::game_archives::{ArchiveCatalogService, TransferableArchive};
use crate::modules::game_connections::{
    ARCHIVE_TRANSFER_CAPABILITY, ARCHIVE_TRANSFER_CHUNK_BYTES, ARCHIVE_TRANSFER_DATA_TIMEOUT_MS,
    ARCHIVE_TRANSFER_FINALIZE_TIMEOUT_MS, ARCHIVE_TRANSFER_OFFER_TIMEOUT_MS,
    ARCHIVE_TRANSFER_WINDOW_CHUNKS, ArchiveTransferEvent, ArchiveTransferOffer,
    GameConnectionService,
};
use crate::modules::instances::InstanceService;
use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

const MAX_ACTIVE_TRANSFERS: usize = 2;

#[derive(Clone)]
pub struct ArchiveTransferService {
    database: Database,
    paths: AppPaths,
    archives: ArchiveCatalogService,
    instances: InstanceService,
    connections: GameConnectionService,
    active: Arc<Mutex<HashMap<String, ActiveTransfer>>>,
}

struct ActiveTransfer {
    instance_id: String,
    cancel: watch::Sender<bool>,
}

enum TransferAbort {
    Cancelled,
    Rejected(String),
    Failed(String, String),
}

impl ArchiveTransferService {
    pub fn new(
        database: Database,
        paths: AppPaths,
        archives: ArchiveCatalogService,
        instances: InstanceService,
        connections: GameConnectionService,
    ) -> Self {
        let service = Self {
            database,
            paths,
            archives,
            instances,
            connections,
            active: Default::default(),
        };
        service.cleanup_packages();
        service
    }

    pub fn list_targets(&self) -> AppResult<Vec<ArchiveTransferTarget>> {
        let mut targets = Vec::new();
        for session in self.connections.sessions() {
            if !session
                .capabilities
                .iter()
                .any(|value| value == ARCHIVE_TRANSFER_CAPABILITY)
            {
                continue;
            }
            let instance = self.instances.read(&session.instance_id)?;
            targets.push(ArchiveTransferTarget {
                instance_id: session.instance_id,
                name: instance.name,
                origin: session.origin,
                game_version: session.game_version,
                platform: session.platform,
                remote_address: session.remote_address,
            });
        }
        targets.sort_by_key(|target| target.name.to_lowercase());
        Ok(targets)
    }

    pub fn list_sources(&self) -> AppResult<Vec<TransferableArchive>> {
        self.archives.list_transferable()
    }

    pub fn inspect_source(&self, main_archive_path: &str) -> AppResult<TransferableArchive> {
        self.archives
            .inspect_main(Path::new(main_archive_path.trim()))
    }

    pub fn start(&self, input: StartArchiveTransferInput) -> AppResult<ArchiveTransferRecord> {
        let instance_id = input.instance_id.trim();
        if instance_id.is_empty() {
            return Err(AppError::validation("Select a connected game instance."));
        }
        let session = self
            .connections
            .session(instance_id)
            .ok_or_else(|| AppError::validation("The selected game instance is not connected."))?;
        if !session
            .capabilities
            .iter()
            .any(|value| value == ARCHIVE_TRANSFER_CAPABILITY)
        {
            return Err(AppError::validation(
                "The selected game instance does not support archive transfer.",
            ));
        }
        let snapshot = self
            .archives
            .snapshot(Path::new(input.main_archive_path.trim()))?;
        let id = Uuid::new_v4().to_string();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        {
            let mut active = self.active.lock();
            if active.len() >= MAX_ACTIVE_TRANSFERS {
                return Err(AppError::validation(
                    "At most two archive transfers may be active.",
                ));
            }
            if active
                .values()
                .any(|transfer| transfer.instance_id == instance_id)
            {
                return Err(AppError::validation(
                    "The selected game instance already has an active archive transfer.",
                ));
            }
            active.insert(
                id.clone(),
                ActiveTransfer {
                    instance_id: instance_id.to_string(),
                    cancel: cancel_tx,
                },
            );
        }
        let now = Utc::now().to_rfc3339();
        let insert_result = self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO archive_transfers(
                    id,instance_id,source_main_path,archive_path,archive_guid,archive_name,
                    author,file_count,uncompressed_bytes,status,phase,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'preparing',?11,?11)",
                rusqlite::params![
                    id,
                    instance_id,
                    snapshot.archive.main_archive_path,
                    snapshot.archive.archive_path,
                    snapshot.archive.archive_guid,
                    snapshot.archive.archive_name,
                    snapshot.archive.author,
                    snapshot.archive.file_count as i64,
                    snapshot.archive.uncompressed_bytes as i64,
                    ArchiveTransferStatus::Preparing.as_db(),
                    now
                ],
            )?;
            Ok(())
        });
        if let Err(error) = insert_result {
            self.active.lock().remove(&id);
            return Err(error);
        }
        let service = self.clone();
        let transfer_id = id.clone();
        let target_id = instance_id.to_string();
        tauri::async_runtime::spawn(async move {
            service
                .run(transfer_id, target_id, snapshot, cancel_rx)
                .await;
        });
        self.get(&id)
    }

    pub fn list(&self) -> AppResult<Vec<ArchiveTransferRecord>> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{SELECT_TRANSFER} ORDER BY created_at DESC LIMIT 50"
            ))?;
            let rows = statement.query_map([], map_transfer)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
    }

    pub fn get(&self, id: &str) -> AppResult<ArchiveTransferRecord> {
        self.database.with_connection(|connection| {
            connection
                .query_row(
                    &format!("{SELECT_TRANSFER} WHERE id=?1"),
                    [id],
                    map_transfer,
                )
                .optional()?
                .ok_or_else(|| AppError::not_found("Archive transfer"))
        })
    }

    pub fn cancel(&self, id: &str) -> AppResult<ArchiveTransferRecord> {
        let record = self.get(id)?;
        if !record.status.active() {
            return Err(AppError::validation(
                "Only an active archive transfer can be cancelled.",
            ));
        }
        let active = self.active.lock();
        let transfer = active
            .get(id)
            .ok_or_else(|| AppError::validation("Archive transfer is no longer active."))?;
        let _ = transfer.cancel.send(true);
        Ok(record)
    }

    pub fn stop_all(&self) {
        let transfers = self.active.lock();
        for transfer in transfers.values() {
            let _ = transfer.cancel.send(true);
        }
        let now = Utc::now().to_rfc3339();
        let _ = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE archive_transfers
                 SET status='interrupted',phase='interrupted',
                     error_code='applicationShutdown',
                     error_message='The desktop tool stopped during the archive transfer.',
                     updated_at=?1,completed_at=?1
                 WHERE status IN ('preparing','waitingAcceptance','transferring','finalizing')",
                [&now],
            )?;
            Ok(())
        });
        self.cleanup_packages();
    }

    async fn run(
        &self,
        id: String,
        instance_id: String,
        snapshot: crate::modules::game_archives::ArchiveSnapshot,
        mut cancel: watch::Receiver<bool>,
    ) {
        let package_path = self.paths.archive_transfers_dir.join(format!("{id}.zip"));
        let package_result = tokio::task::spawn_blocking({
            let package_path = package_path.clone();
            move || build_package(snapshot, package_path)
        })
        .await
        .map_err(AppError::internal)
        .and_then(|result| result);
        let outcome = match package_result {
            Ok(_package) if *cancel.borrow() => Err(TransferAbort::Cancelled),
            Ok(package) => self.execute(&id, &instance_id, package, &mut cancel).await,
            Err(error) => Err(abort_from_error(error)),
        };
        match outcome {
            Ok(()) => {}
            Err(TransferAbort::Cancelled) => {
                self.connections.cancel_archive_transfer(
                    &instance_id,
                    &id,
                    "The desktop user cancelled the archive transfer.",
                );
                self.finish_terminal(
                    &id,
                    ArchiveTransferStatus::Cancelled,
                    "cancelled",
                    "",
                    "",
                    "",
                );
            }
            Err(TransferAbort::Rejected(message)) => {
                self.finish_terminal(
                    &id,
                    ArchiveTransferStatus::Rejected,
                    "rejected",
                    "",
                    "gameRejected",
                    &message,
                );
            }
            Err(TransferAbort::Failed(code, message)) => {
                self.finish_terminal(
                    &id,
                    ArchiveTransferStatus::Failed,
                    "failed",
                    "",
                    &code,
                    &message,
                );
            }
        }
        self.connections.finish_archive_transfer(&id);
        self.active.lock().remove(&id);
        let _ = tokio::fs::remove_file(package_path).await;
    }

    async fn execute(
        &self,
        id: &str,
        instance_id: &str,
        package: ArchivePackage,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<(), TransferAbort> {
        let record = self.get(id).map_err(abort_from_error)?;
        self.update_package(id, &package);
        let display_name = std::env::var("COMPUTERNAME")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "ABYA Desktop Development Tool".into());
        let offer = ArchiveTransferOffer {
            transfer_id: id.to_string(),
            source_display_name: display_name,
            archive_guid: record.archive_guid,
            archive_name: record.archive_name,
            author: record.author,
            file_count: record.file_count,
            uncompressed_bytes: record.uncompressed_bytes,
            package_bytes: package.bytes,
            sha256: package.sha256.clone(),
            format: "zip".into(),
            conflict_policy: "replace".into(),
            chunk_size: ARCHIVE_TRANSFER_CHUNK_BYTES,
        };
        let mut events = self
            .connections
            .begin_archive_transfer(instance_id, offer)
            .map_err(abort_from_error)?;
        self.update_status(
            id,
            ArchiveTransferStatus::WaitingAcceptance,
            "waitingAcceptance",
            0.0,
        );
        loop {
            match wait_event(
                &mut events,
                cancel,
                Duration::from_millis(ARCHIVE_TRANSFER_OFFER_TIMEOUT_MS),
                "archiveTransferOfferTimeout",
            )
            .await?
            {
                ArchiveTransferEvent::Decision { accepted: true, .. } => break,
                ArchiveTransferEvent::Decision {
                    accepted: false,
                    reason,
                } => return Err(TransferAbort::Rejected(reason)),
                ArchiveTransferEvent::Disconnected { message } => {
                    return Err(TransferAbort::Failed(
                        "gameConnectionClosed".into(),
                        message,
                    ));
                }
                _ => {}
            }
        }
        self.update_status(id, ArchiveTransferStatus::Transferring, "transferring", 0.0);
        let mut file = tokio::fs::File::open(&package.path)
            .await
            .map_err(|error| {
                TransferAbort::Failed("archivePackageRead".into(), error.to_string())
            })?;
        let mut sent = 0_u64;
        let mut acknowledged = 0_u64;
        let window_bytes = (ARCHIVE_TRANSFER_CHUNK_BYTES * ARCHIVE_TRANSFER_WINDOW_CHUNKS) as u64;
        while acknowledged < package.bytes {
            while sent < package.bytes && sent.saturating_sub(acknowledged) < window_bytes {
                if *cancel.borrow() {
                    return Err(TransferAbort::Cancelled);
                }
                let remaining = usize::try_from(
                    (package.bytes - sent).min(ARCHIVE_TRANSFER_CHUNK_BYTES as u64),
                )
                .unwrap_or(ARCHIVE_TRANSFER_CHUNK_BYTES);
                let mut buffer = vec![0_u8; remaining];
                file.read_exact(&mut buffer).await.map_err(|error| {
                    TransferAbort::Failed("archivePackageRead".into(), error.to_string())
                })?;
                self.connections
                    .send_archive_chunk(instance_id, id, sent, &buffer)
                    .map_err(abort_from_error)?;
                sent += buffer.len() as u64;
            }
            match wait_event(
                &mut events,
                cancel,
                Duration::from_millis(ARCHIVE_TRANSFER_DATA_TIMEOUT_MS),
                "archiveTransferDataTimeout",
            )
            .await?
            {
                ArchiveTransferEvent::Ack { received_bytes } => {
                    if received_bytes < acknowledged || received_bytes > sent {
                        return Err(TransferAbort::Failed(
                            "archiveTransferInvalidAck".into(),
                            "The game returned an invalid archive transfer acknowledgement.".into(),
                        ));
                    }
                    acknowledged = received_bytes;
                    let progress = if package.bytes == 0 {
                        100.0
                    } else {
                        acknowledged as f64 * 100.0 / package.bytes as f64
                    };
                    self.update_progress(id, acknowledged, "transferring", progress);
                }
                ArchiveTransferEvent::Progress {
                    phase,
                    percent,
                    message,
                } => {
                    let phase = display_phase(&phase, &message);
                    self.update_status(
                        id,
                        ArchiveTransferStatus::Transferring,
                        phase,
                        percent.clamp(0.0, 100.0),
                    );
                }
                ArchiveTransferEvent::Result {
                    status,
                    error_code,
                    message,
                    ..
                } => {
                    return Err(TransferAbort::Failed(
                        if error_code.is_empty() {
                            "archiveTransferEndedEarly".into()
                        } else {
                            error_code
                        },
                        if message.is_empty() { status } else { message },
                    ));
                }
                ArchiveTransferEvent::Disconnected { message } => {
                    return Err(TransferAbort::Failed(
                        "gameConnectionClosed".into(),
                        message,
                    ));
                }
                ArchiveTransferEvent::Decision { .. } => {}
            }
        }
        self.connections
            .complete_archive_transfer(instance_id, id)
            .map_err(abort_from_error)?;
        self.update_status(id, ArchiveTransferStatus::Finalizing, "verifying", 100.0);
        loop {
            match wait_event(
                &mut events,
                cancel,
                Duration::from_millis(ARCHIVE_TRANSFER_FINALIZE_TIMEOUT_MS),
                "archiveTransferFinalizeTimeout",
            )
            .await?
            {
                ArchiveTransferEvent::Progress {
                    phase,
                    percent,
                    message,
                } => {
                    let phase = display_phase(&phase, &message);
                    self.update_status(
                        id,
                        ArchiveTransferStatus::Finalizing,
                        phase,
                        percent.clamp(0.0, 100.0),
                    );
                }
                ArchiveTransferEvent::Result {
                    status,
                    installed_path,
                    error_code,
                    message,
                } => {
                    if status.eq_ignore_ascii_case("completed") {
                        self.finish_terminal(
                            id,
                            ArchiveTransferStatus::Completed,
                            "completed",
                            &installed_path,
                            "",
                            "",
                        );
                        return Ok(());
                    }
                    return Err(TransferAbort::Failed(
                        if error_code.is_empty() {
                            "archiveTransferInstallFailed".into()
                        } else {
                            error_code
                        },
                        if message.is_empty() { status } else { message },
                    ));
                }
                ArchiveTransferEvent::Disconnected { message } => {
                    return Err(TransferAbort::Failed(
                        "gameConnectionClosed".into(),
                        message,
                    ));
                }
                _ => {}
            }
        }
    }

    fn update_package(&self, id: &str, package: &ArchivePackage) {
        let now = Utc::now().to_rfc3339();
        let _ = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE archive_transfers
                 SET package_bytes=?2,sha256=?3,updated_at=?4 WHERE id=?1",
                rusqlite::params![id, package.bytes as i64, package.sha256, now],
            )?;
            Ok(())
        });
    }

    fn update_progress(&self, id: &str, acknowledged: u64, phase: &str, percent: f64) {
        let now = Utc::now().to_rfc3339();
        let _ = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE archive_transfers
                 SET acknowledged_bytes=?2,phase=?3,progress_percent=?4,updated_at=?5
                 WHERE id=?1",
                rusqlite::params![id, acknowledged as i64, phase, percent, now],
            )?;
            Ok(())
        });
    }

    fn update_status(&self, id: &str, status: ArchiveTransferStatus, phase: &str, percent: f64) {
        let now = Utc::now().to_rfc3339();
        let _ = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE archive_transfers
                 SET status=?2,phase=?3,progress_percent=?4,updated_at=?5 WHERE id=?1",
                rusqlite::params![id, status.as_db(), phase, percent, now],
            )?;
            Ok(())
        });
    }

    fn finish_terminal(
        &self,
        id: &str,
        status: ArchiveTransferStatus,
        phase: &str,
        installed_path: &str,
        error_code: &str,
        error_message: &str,
    ) {
        let now = Utc::now().to_rfc3339();
        let progress = if status == ArchiveTransferStatus::Completed {
            100.0
        } else {
            self.get(id)
                .map(|record| record.progress_percent)
                .unwrap_or_default()
        };
        let _ = self.database.with_connection(|connection| {
            connection.execute(
                "UPDATE archive_transfers
                 SET status=?2,phase=?3,progress_percent=?4,installed_path=?5,
                     error_code=?6,error_message=?7,updated_at=?8,completed_at=?8
                 WHERE id=?1",
                rusqlite::params![
                    id,
                    status.as_db(),
                    phase,
                    progress,
                    installed_path,
                    error_code,
                    error_message,
                    now
                ],
            )?;
            Ok(())
        });
    }

    fn cleanup_packages(&self) {
        let Ok(entries) = std::fs::read_dir(&self.paths.archive_transfers_dir) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

async fn wait_event(
    events: &mut mpsc::UnboundedReceiver<ArchiveTransferEvent>,
    cancel: &mut watch::Receiver<bool>,
    timeout: Duration,
    timeout_code: &str,
) -> Result<ArchiveTransferEvent, TransferAbort> {
    if *cancel.borrow() {
        return Err(TransferAbort::Cancelled);
    }
    tokio::select! {
        changed = cancel.changed() => {
            match changed {
                Ok(()) if *cancel.borrow() => Err(TransferAbort::Cancelled),
                _ => Err(TransferAbort::Failed(
                    "archiveTransferCancelled".into(),
                    "Archive transfer cancellation state closed.".into(),
                )),
            }
        }
        result = tokio::time::timeout(timeout, events.recv()) => {
            match result {
                Ok(Some(event)) => Ok(event),
                Ok(None) => Err(TransferAbort::Failed(
                    "gameConnectionClosed".into(),
                    "The archive transfer event channel closed.".into(),
                )),
                Err(_) => Err(TransferAbort::Failed(
                    timeout_code.into(),
                    "The archive transfer timed out waiting for the game.".into(),
                )),
            }
        }
    }
}

const SELECT_TRANSFER: &str = "SELECT
    id,instance_id,source_main_path,archive_path,archive_guid,archive_name,author,
    file_count,uncompressed_bytes,package_bytes,acknowledged_bytes,sha256,status,
    phase,progress_percent,installed_path,error_code,error_message,created_at,
    updated_at,completed_at
    FROM archive_transfers";

fn map_transfer(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArchiveTransferRecord> {
    let status: String = row.get(12)?;
    Ok(ArchiveTransferRecord {
        id: row.get(0)?,
        instance_id: row.get(1)?,
        source_main_path: row.get(2)?,
        archive_path: row.get(3)?,
        archive_guid: row.get(4)?,
        archive_name: row.get(5)?,
        author: row.get(6)?,
        file_count: row.get::<_, i64>(7)?.max(0) as u64,
        uncompressed_bytes: row.get::<_, i64>(8)?.max(0) as u64,
        package_bytes: row.get::<_, i64>(9)?.max(0) as u64,
        acknowledged_bytes: row.get::<_, i64>(10)?.max(0) as u64,
        sha256: row.get(11)?,
        status: ArchiveTransferStatus::from_db(&status),
        phase: row.get(13)?,
        progress_percent: row.get(14)?,
        installed_path: row.get(15)?,
        error_code: row.get(16)?,
        error_message: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
        completed_at: row.get(20)?,
    })
}

fn abort_from_error(error: AppError) -> TransferAbort {
    let message = error.to_string();
    TransferAbort::Failed(error.code, message)
}

fn display_phase<'a>(phase: &'a str, message: &'a str) -> &'a str {
    if phase.trim().is_empty() && !message.trim().is_empty() {
        message
    } else {
        phase
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_status_round_trips() {
        for status in [
            ArchiveTransferStatus::Preparing,
            ArchiveTransferStatus::WaitingAcceptance,
            ArchiveTransferStatus::Transferring,
            ArchiveTransferStatus::Finalizing,
            ArchiveTransferStatus::Completed,
            ArchiveTransferStatus::Rejected,
            ArchiveTransferStatus::Cancelled,
            ArchiveTransferStatus::Failed,
            ArchiveTransferStatus::Interrupted,
        ] {
            assert_eq!(ArchiveTransferStatus::from_db(status.as_db()), status);
        }
    }
}
