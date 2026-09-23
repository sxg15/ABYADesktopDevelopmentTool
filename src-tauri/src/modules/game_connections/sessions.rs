use super::models::{
    ARCHIVE_TRANSFER_CAPABILITY, ArchiveTransferEvent, ArchiveTransferOffer, SessionSnapshot,
};
use super::protocol::ServerMessage;
use crate::foundation::{AppError, AppResult};
use axum::extract::ws::Message;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<String, LiveSession>>>,
    pending_transfers: Arc<Mutex<HashMap<String, PendingTransfer>>>,
}

struct PendingTransfer {
    instance_id: String,
    sender: tokio::sync::mpsc::UnboundedSender<ArchiveTransferEvent>,
}

pub(crate) struct LiveSession {
    pub snapshot: SessionSnapshot,
    pub sender: UnboundedSender<Message>,
    pub last_seen: Instant,
}

impl SessionRegistry {
    pub(crate) fn new() -> Self {
        Self {
            sessions: Default::default(),
            pending_transfers: Default::default(),
        }
    }

    pub(crate) fn insert(&self, snapshot: SessionSnapshot, sender: UnboundedSender<Message>) {
        if let Some(previous) = self.sessions.lock().insert(
            snapshot.instance_id.clone(),
            LiveSession {
                snapshot,
                sender,
                last_seen: Instant::now(),
            },
        ) {
            send_json(
                &previous.sender,
                &ServerMessage::Disconnect {
                    reason: "A newer connection replaced this session.".into(),
                },
            );
        }
    }

    pub(crate) fn touch(&self, instance_id: &str, connection_id: &str) {
        if let Some(session) = self.sessions.lock().get_mut(instance_id)
            && session.snapshot.connection_id == connection_id
        {
            session.last_seen = Instant::now();
        }
    }

    pub(crate) fn is_timed_out(&self, instance_id: &str, connection_id: &str) -> bool {
        self.sessions
            .lock()
            .get(instance_id)
            .filter(|session| session.snapshot.connection_id == connection_id)
            .is_none_or(|session| {
                session.last_seen.elapsed()
                    > Duration::from_millis(super::models::DISCONNECT_TIMEOUT_MS)
            })
    }

    pub(crate) fn remove_if_current(&self, instance_id: &str, connection_id: &str) -> bool {
        let removed = {
            let mut sessions = self.sessions.lock();
            let current = sessions
                .get(instance_id)
                .is_some_and(|session| session.snapshot.connection_id == connection_id);
            if current {
                sessions.remove(instance_id);
            }
            current
        };
        if removed {
            self.fail_transfers(instance_id, "The game connection closed.");
        }
        removed
    }

    pub(crate) fn count(&self) -> usize {
        self.sessions.lock().len()
    }

    pub(crate) fn instance_ids(&self) -> Vec<String> {
        self.sessions.lock().keys().cloned().collect()
    }

    pub(crate) fn snapshots(&self) -> Vec<SessionSnapshot> {
        self.sessions
            .lock()
            .values()
            .map(|session| session.snapshot.clone())
            .collect()
    }

    pub(crate) fn snapshot(&self, instance_id: &str) -> Option<SessionSnapshot> {
        self.sessions
            .lock()
            .get(instance_id)
            .map(|session| session.snapshot.clone())
    }

    pub(crate) fn send_logs_control(&self, instance_id: &str, enabled: bool) -> AppResult<()> {
        let sender = self.sender(instance_id)?;
        send_json(&sender, &ServerMessage::LogsControl { enabled });
        Ok(())
    }

    pub(crate) fn begin_archive_transfer(
        &self,
        instance_id: &str,
        offer: ArchiveTransferOffer,
    ) -> AppResult<tokio::sync::mpsc::UnboundedReceiver<ArchiveTransferEvent>> {
        let sender = self.sender_with_capability(instance_id, ARCHIVE_TRANSFER_CAPABILITY)?;
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        self.pending_transfers.lock().insert(
            offer.transfer_id.clone(),
            PendingTransfer {
                instance_id: instance_id.to_string(),
                sender: event_tx,
            },
        );
        send_json(&sender, &ServerMessage::ArchiveTransferOffer(offer));
        Ok(event_rx)
    }

    pub(crate) fn send_archive_chunk(
        &self,
        instance_id: &str,
        transfer_id: &str,
        offset: u64,
        payload: &[u8],
    ) -> AppResult<()> {
        self.ensure_transfer_owner(instance_id, transfer_id)?;
        let sender = self.sender_with_capability(instance_id, ARCHIVE_TRANSFER_CAPABILITY)?;
        let transfer_uuid = Uuid::parse_str(transfer_id)
            .map_err(|_| AppError::validation("Archive transfer ID is invalid."))?;
        let payload_len = u32::try_from(payload.len())
            .map_err(|_| AppError::validation("Archive transfer chunk is too large."))?;
        let mut frame = Vec::with_capacity(34 + payload.len());
        frame.extend_from_slice(b"ABAT");
        frame.push(1);
        frame.push(1);
        frame.extend_from_slice(transfer_uuid.as_bytes());
        frame.extend_from_slice(&offset.to_le_bytes());
        frame.extend_from_slice(&payload_len.to_le_bytes());
        frame.extend_from_slice(payload);
        sender
            .send(Message::Binary(frame.into()))
            .map_err(|_| AppError::validation("The game connection closed."))
    }

    pub(crate) fn complete_archive_transfer(
        &self,
        instance_id: &str,
        transfer_id: &str,
    ) -> AppResult<()> {
        self.ensure_transfer_owner(instance_id, transfer_id)?;
        let sender = self.sender_with_capability(instance_id, ARCHIVE_TRANSFER_CAPABILITY)?;
        send_json(
            &sender,
            &ServerMessage::ArchiveTransferComplete {
                transfer_id: transfer_id.to_string(),
            },
        );
        Ok(())
    }

    pub(crate) fn cancel_archive_transfer(
        &self,
        instance_id: &str,
        transfer_id: &str,
        reason: &str,
    ) {
        if let Ok(sender) = self.sender_with_capability(instance_id, ARCHIVE_TRANSFER_CAPABILITY) {
            send_json(
                &sender,
                &ServerMessage::ArchiveTransferCancel {
                    transfer_id: transfer_id.to_string(),
                    reason: reason.to_string(),
                },
            );
        }
    }

    pub(crate) fn resolve_archive_transfer(
        &self,
        instance_id: &str,
        transfer_id: &str,
        event: ArchiveTransferEvent,
    ) {
        let pending = self.pending_transfers.lock();
        let Some(transfer) = pending.get(transfer_id) else {
            return;
        };
        if transfer.instance_id == instance_id {
            let _ = transfer.sender.send(event);
        }
    }

    pub(crate) fn finish_archive_transfer(&self, transfer_id: &str) {
        self.pending_transfers.lock().remove(transfer_id);
    }

    pub(crate) fn stop_all(&self) {
        let sessions = std::mem::take(&mut *self.sessions.lock());
        for (_, session) in sessions {
            send_json(
                &session.sender,
                &ServerMessage::Disconnect {
                    reason: "Desktop tool is shutting down.".into(),
                },
            );
        }
        let transfers = std::mem::take(&mut *self.pending_transfers.lock());
        for (_, transfer) in transfers {
            let _ = transfer.sender.send(ArchiveTransferEvent::Disconnected {
                message: "The desktop game gateway stopped.".into(),
            });
        }
    }

    fn sender(&self, instance_id: &str) -> AppResult<UnboundedSender<Message>> {
        let sessions = self.sessions.lock();
        let session = sessions.get(instance_id).ok_or_else(|| {
            AppError::validation("The game instance is not connected to the desktop tool.")
        })?;
        Ok(session.sender.clone())
    }

    fn sender_with_capability(
        &self,
        instance_id: &str,
        capability: &str,
    ) -> AppResult<UnboundedSender<Message>> {
        let sessions = self.sessions.lock();
        let session = sessions.get(instance_id).ok_or_else(|| {
            AppError::validation("The game instance is not connected to the desktop tool.")
        })?;
        if !session
            .snapshot
            .capabilities
            .iter()
            .any(|value| value == capability)
        {
            return Err(AppError::validation(format!(
                "The selected game instance does not support {capability}."
            )));
        }
        Ok(session.sender.clone())
    }

    fn ensure_transfer_owner(&self, instance_id: &str, transfer_id: &str) -> AppResult<()> {
        let pending = self.pending_transfers.lock();
        let transfer = pending
            .get(transfer_id)
            .ok_or_else(|| AppError::not_found("Archive transfer"))?;
        if transfer.instance_id != instance_id {
            return Err(AppError::validation(
                "Archive transfer does not belong to the selected game instance.",
            ));
        }
        Ok(())
    }

    fn fail_transfers(&self, instance_id: &str, message: &str) {
        let mut transfers = self.pending_transfers.lock();
        let ids = transfers
            .iter()
            .filter(|(_, transfer)| transfer.instance_id == instance_id)
            .map(|(transfer_id, _)| transfer_id.clone())
            .collect::<Vec<_>>();
        for transfer_id in ids {
            if let Some(transfer) = transfers.remove(&transfer_id) {
                let _ = transfer.sender.send(ArchiveTransferEvent::Disconnected {
                    message: message.to_string(),
                });
            }
        }
    }
}

pub(crate) fn send_json(sender: &UnboundedSender<Message>, message: &ServerMessage) {
    if let Ok(json) = serde_json::to_string(message) {
        let _ = sender.send(Message::Text(json.into()));
    }
}
