use crate::foundation::{AppError, AppResult};
use std::io::Write;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

#[derive(Clone)]
pub(super) struct TerminalInput {
    sender: mpsc::SyncSender<(Vec<u8>, mpsc::SyncSender<bool>)>,
    busy: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
}
impl TerminalInput {
    pub fn new(mut writer: Box<dyn Write + Send>) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<(Vec<u8>, mpsc::SyncSender<bool>)>(1);
        let failed = Arc::new(AtomicBool::new(false));
        let busy = Arc::new(AtomicBool::new(false));
        let worker_failed = failed.clone();
        std::thread::spawn(move || {
            while let Ok((bytes, response)) = receiver.recv() {
                let ok = !worker_failed.load(Ordering::SeqCst)
                    && writer
                        .write_all(&bytes)
                        .and_then(|_| writer.flush())
                        .is_ok();
                if !ok {
                    worker_failed.store(true, Ordering::SeqCst);
                }
                let _ = response.send(ok);
            }
        });
        Self {
            sender,
            busy,
            failed,
        }
    }
    pub fn write(&self, data: &[u8]) -> AppResult<()> {
        self.write_with_timeout(data, Duration::from_secs(2))
    }
    fn write_with_timeout(&self, data: &[u8], timeout: Duration) -> AppResult<()> {
        if self.failed.load(Ordering::SeqCst) {
            return Err(input_error());
        }
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(AppError::new(
                "codex_input_busy",
                "上一条终端输入尚未返回，未发送本条输入。",
                "not_executed",
            ));
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        if self.sender.try_send((data.to_vec(), sender)).is_err() {
            self.failed.store(true, Ordering::SeqCst);
            return Err(input_error());
        }
        if receiver.recv_timeout(timeout) != Ok(true) {
            self.failed.store(true, Ordering::SeqCst);
            return Err(input_error());
        }
        // Release before the caller submits the next chunk; the worker never accepts a second in-flight write.
        self.busy.store(false, Ordering::SeqCst);
        Ok(())
    }
}
fn input_error() -> AppError {
    AppError::new(
        "codex_input_stalled",
        "终端输入未确认，已停止后续输入且不会自动重发。请核对已完成操作，待执行结束或暂停后重建终端连接。",
        "outcome_unknown",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Slow(mpsc::Receiver<()>, Arc<std::sync::Mutex<Vec<u8>>>);
    impl Write for Slow {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let _ = self.0.recv();
            self.1.lock().unwrap().extend(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn blocked_writer_times_out_and_late_success_cannot_replay_following_input() {
        let (release, wait) = mpsc::channel();
        let bytes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let input = TerminalInput::new(Box::new(Slow(wait, bytes.clone())));
        assert!(
            input
                .write_with_timeout(b"first", Duration::from_millis(30))
                .is_err()
        );
        assert!(input.write(b"must-not-send").is_err());
        release.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(&*bytes.lock().unwrap(), b"first");
        assert!(input.write(b"still-blocked").is_err());
    }
}
