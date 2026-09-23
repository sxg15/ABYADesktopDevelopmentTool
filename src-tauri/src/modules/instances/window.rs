use super::WindowVisibilityMode;
#[cfg(windows)]
use std::collections::{HashMap, hash_map::Entry};
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::time::Duration;

const BACKGROUND: u8 = 0;
const VISIBLE: u8 = 1;

pub(crate) struct InstanceWindowController {
    desired: Arc<AtomicU8>,
    stopped: Arc<AtomicBool>,
}

impl InstanceWindowController {
    pub(crate) fn start(
        pid: u32,
        initial: WindowVisibilityMode,
        restore_foreground: isize,
    ) -> io::Result<Self> {
        let desired = Arc::new(AtomicU8::new(encode(initial)));
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_desired = desired.clone();
        let worker_stopped = stopped.clone();

        std::thread::Builder::new()
            .name(format!("abya-instance-window-{pid}"))
            .spawn(move || {
                let mut previous = initial;
                let mut elapsed = Duration::ZERO;
                let mut windows = WindowRegistry::new(restore_foreground);
                while !worker_stopped.load(Ordering::Relaxed) {
                    let current = decode(worker_desired.load(Ordering::Relaxed));
                    if current == WindowVisibilityMode::Background || previous != current {
                        apply_window_visibility(pid, current, &mut windows);
                    }
                    previous = current;
                    let delay = if elapsed < Duration::from_secs(15) {
                        Duration::from_millis(10)
                    } else {
                        Duration::from_millis(100)
                    };
                    std::thread::sleep(delay);
                    elapsed += delay;
                }
            })?;

        Ok(Self { desired, stopped })
    }

    pub(crate) fn set_visibility(&self, visibility: WindowVisibilityMode) {
        self.desired.store(encode(visibility), Ordering::Relaxed);
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

impl Drop for InstanceWindowController {
    fn drop(&mut self) {
        self.stop();
    }
}

fn encode(visibility: WindowVisibilityMode) -> u8 {
    match visibility {
        WindowVisibilityMode::Background => BACKGROUND,
        WindowVisibilityMode::Visible => VISIBLE,
    }
}

fn decode(value: u8) -> WindowVisibilityMode {
    if value == BACKGROUND {
        WindowVisibilityMode::Background
    } else {
        WindowVisibilityMode::Visible
    }
}

#[cfg(windows)]
struct WindowSnapshot {
    rect: windows_sys::Win32::Foundation::RECT,
    extended_style: isize,
}

#[cfg(windows)]
struct WindowRegistry {
    snapshots: HashMap<isize, WindowSnapshot>,
    restore_foreground: isize,
}

#[cfg(windows)]
impl WindowRegistry {
    fn new(restore_foreground: isize) -> Self {
        Self {
            snapshots: HashMap::new(),
            restore_foreground,
        }
    }
}

#[cfg(not(windows))]
struct WindowRegistry;

#[cfg(not(windows))]
impl WindowRegistry {
    fn new(_restore_foreground: isize) -> Self {
        Self
    }
}

#[cfg(windows)]
fn apply_window_visibility(
    pid: u32,
    visibility: WindowVisibilityMode,
    registry: &mut WindowRegistry,
) {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
        GetWindowThreadProcessId, IsWindow, IsWindowVisible, SW_SHOWNOACTIVATE, SWP_ASYNCWINDOWPOS,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER,
        SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindowAsync, WS_EX_APPWINDOW,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    use windows_sys::core::BOOL;

    const BACKGROUND_X: i32 = -50_000;
    const BACKGROUND_Y: i32 = -50_000;

    struct EnumContext {
        pid: u32,
        visibility: WindowVisibilityMode,
        registry: *mut WindowRegistry,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let context = unsafe { &mut *(lparam as *mut EnumContext) };
        let mut window_pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut window_pid);
        }
        if window_pid != context.pid {
            return 1;
        }

        let registry = unsafe { &mut *context.registry };
        match context.visibility {
            WindowVisibilityMode::Background => {
                let key = hwnd as isize;
                if let Entry::Vacant(entry) = registry.snapshots.entry(key) {
                    if unsafe { IsWindowVisible(hwnd) } == 0 {
                        return 1;
                    }
                    let mut rect = RECT::default();
                    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
                        return 1;
                    }
                    entry.insert(WindowSnapshot {
                        rect,
                        extended_style: unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) },
                    });
                }

                let snapshot = &registry.snapshots[&key];
                let background_style = (snapshot.extended_style & !(WS_EX_APPWINDOW as isize))
                    | WS_EX_TOOLWINDOW as isize
                    | WS_EX_NOACTIVATE as isize;
                unsafe {
                    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, background_style);
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        BACKGROUND_X,
                        BACKGROUND_Y,
                        0,
                        0,
                        SWP_ASYNCWINDOWPOS
                            | SWP_FRAMECHANGED
                            | SWP_NOACTIVATE
                            | SWP_NOOWNERZORDER
                            | SWP_NOSIZE
                            | SWP_NOZORDER,
                    );
                    ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
                }
            }
            WindowVisibilityMode::Visible => {
                let key = hwnd as isize;
                if let Some(snapshot) = registry.snapshots.remove(&key) {
                    let width = (snapshot.rect.right - snapshot.rect.left).max(1);
                    let height = (snapshot.rect.bottom - snapshot.rect.top).max(1);
                    unsafe {
                        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, snapshot.extended_style);
                        SetWindowPos(
                            hwnd,
                            std::ptr::null_mut(),
                            snapshot.rect.left,
                            snapshot.rect.top,
                            width,
                            height,
                            SWP_ASYNCWINDOWPOS
                                | SWP_FRAMECHANGED
                                | SWP_NOACTIVATE
                                | SWP_NOOWNERZORDER
                                | SWP_NOZORDER,
                        );
                        ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
                    }
                }
            }
        }
        1
    }

    let foreground = unsafe { GetForegroundWindow() };
    if !foreground.is_null() {
        let mut foreground_pid = 0;
        unsafe {
            GetWindowThreadProcessId(foreground, &mut foreground_pid);
        }
        if foreground_pid != pid {
            registry.restore_foreground = foreground as isize;
        }
    }

    let mut context = EnumContext {
        pid,
        visibility,
        registry,
    };
    unsafe {
        EnumWindows(
            Some(visit),
            (&mut context as *mut EnumContext).cast::<c_void>() as LPARAM,
        );
    }
    registry
        .snapshots
        .retain(|hwnd, _| unsafe { IsWindow(*hwnd as HWND) != 0 });

    if visibility == WindowVisibilityMode::Background && registry.restore_foreground != 0 {
        let current = unsafe { GetForegroundWindow() };
        if !current.is_null() {
            let mut current_pid = 0;
            unsafe {
                GetWindowThreadProcessId(current, &mut current_pid);
            }
            let previous = registry.restore_foreground as HWND;
            if current_pid == pid && unsafe { IsWindow(previous) } != 0 {
                unsafe {
                    SetForegroundWindow(previous);
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn apply_window_visibility(
    _pid: u32,
    _visibility: WindowVisibilityMode,
    _registry: &mut WindowRegistry,
) {
}

#[cfg(windows)]
pub(crate) fn capture_foreground_window() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe { GetForegroundWindow() as isize }
}

#[cfg(not(windows))]
pub(crate) fn capture_foreground_window() -> isize {
    0
}
