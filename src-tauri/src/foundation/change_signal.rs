//! A content-free wakeup for background projections. Domain data stays in its owner.
use parking_lot::{Condvar, Mutex};
use std::sync::LazyLock;
use std::time::Duration;

static SIGNAL: LazyLock<(Mutex<u64>, Condvar)> = LazyLock::new(|| (Mutex::new(0), Condvar::new()));

pub fn notify() {
    let mut revision = SIGNAL.0.lock();
    *revision = revision.wrapping_add(1);
    SIGNAL.1.notify_all();
}

pub fn revision() -> u64 {
    *SIGNAL.0.lock()
}

pub fn wait(previous: u64, timeout: Duration) -> u64 {
    let mut revision = SIGNAL.0.lock();
    if *revision == previous {
        SIGNAL.1.wait_for(&mut revision, timeout);
    }
    *revision
}
