use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(super) struct ConversationGate {
    pub normal: Mutex<()>,
    // Only short handle publication / generation checks; never RPC or filesystem I/O.
    pub publication: Mutex<()>,
    pub generation: AtomicU64,
}
impl ConversationGate {
    pub fn epoch(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    pub fn invalidate(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }
    pub fn check(&self, expected: u64) -> AppResult<()> {
        if self.epoch() != expected {
            Err(AppError::new(
                "codex_generation_changed",
                "执行器已停止或替换，旧操作已取消。",
                "not_executed",
            ))
        } else {
            Ok(())
        }
    }
}
impl CodexTerminalService {
    pub(super) fn gate(&self, conversation: &str) -> Arc<ConversationGate> {
        self.gates
            .lock()
            .entry(conversation.into())
            .or_default()
            .clone()
    }
}
