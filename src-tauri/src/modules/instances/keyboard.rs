use super::{InstanceService, WindowVisibilityMode};
use crate::foundation::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Write, time::Duration};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeKeyInput {
    pub instance_id: String,
    pub key: String,
    pub hold_ms: u64,
}

/// 仅允许单个游戏按键，不能发送系统快捷键或任意文本。
fn scan_code(key: &str) -> AppResult<(u16, bool)> {
    let code = match key {
        "Escape" => (0x01, false),
        "Enter" => (0x1c, false),
        "Space" => (0x39, false),
        "W" => (0x11, false),
        "A" => (0x1e, false),
        "S" => (0x1f, false),
        "D" => (0x20, false),
        "ArrowUp" => (0x48, true),
        "ArrowDown" => (0x50, true),
        "ArrowLeft" => (0x4b, true),
        "ArrowRight" => (0x4d, true),
        _ => {
            return Err(AppError::validation(
                "仅支持 Escape/Enter/Space/W/A/S/D 和方向键。",
            ));
        }
    };
    Ok(code)
}

impl InstanceService {
    /// 仅返回属于所选实例的原始回执；started 表示结果未知，不能据此重放。
    pub fn native_key_result(&self, instance: &str, request: &str) -> AppResult<Value> {
        self.read(instance)?;
        let id = uuid::Uuid::parse_str(request).map_err(AppError::internal)?;
        let path = self
            .paths
            .reports_dir
            .join("KeyboardInputs")
            .join(format!("{id}.json"));
        let receipt: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        if receipt["instanceId"] != instance {
            return Err(AppError::validation("输入回执不属于所选实例。"));
        }
        Ok(receipt)
    }
    /// 原生输入限定托管进程，持久记录请求占位，任何失败都不自动重放。
    pub fn native_key(&self, input: NativeKeyInput, request: &str) -> AppResult<Value> {
        let (scan, extended) = scan_code(&input.key)?;
        if !(40..=1000).contains(&input.hold_ms) {
            return Err(AppError::validation("holdMs 必须在 40–1000 毫秒之间。"));
        }
        let request = uuid::Uuid::parse_str(request).map_err(AppError::internal)?;
        let pid = self.runtime_pid(&input.instance_id)?;
        let instance = self.read(&input.instance_id)?;
        let visibility = instance
            .profile
            .as_ref()
            .ok_or_else(|| AppError::validation("缺少启动配置。"))?
            .visibility_mode;
        let root = self.paths.reports_dir.join("KeyboardInputs");
        std::fs::create_dir_all(&root)?;
        let path = root.join(format!("{request}.json"));
        let mut receipt = json!({"requestId":request,"instanceId":input.instance_id,"taskId":instance.task_id,
            "pid":pid,"key":input.key,"holdMs":input.hold_ms,"source":"windows-sendinput-scan-code",
            "status":"started","startedAt":chrono::Utc::now().to_rfc3339(),"gameplayVerified":false});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(serde_json::to_string_pretty(&receipt)?.as_bytes())?;
        file.sync_all()?;
        let result = (|| {
            self.set_window_visibility(&input.instance_id, WindowVisibilityMode::Visible)?;
            std::thread::sleep(Duration::from_millis(150));
            native_stroke(pid, scan, extended, input.hold_ms, || {
                self.runtime_pid(&input.instance_id).is_ok_and(|p| p == pid)
            })
        })();
        // 成功后留在前台供状态/画面对照；立即恢复焦点会再次触发游戏失焦暂停。
        if result.is_err() {
            let _ = self.set_window_visibility(&input.instance_id, visibility);
        }
        receipt["completedAt"] = json!(chrono::Utc::now().to_rfc3339());
        receipt["receiptPath"] = json!(path);
        match result {
            Ok(value) => {
                receipt["status"] = if value["completed"] == true {
                    json!("sent")
                } else {
                    json!("incomplete")
                };
                receipt["input"] = value;
                receipt["windowLeftVisible"] = json!(true);
            }
            Err(error) => {
                receipt["status"] = json!("failed");
                receipt["error"] = json!(error.to_string());
            }
        }
        std::fs::write(&path, serde_json::to_vec_pretty(&receipt)?)?;
        Ok(receipt)
    }
}

#[cfg(not(windows))]
fn native_stroke(_: u32, _: u16, _: bool, _: u64, _: impl Fn() -> bool) -> AppResult<Value> {
    Err(AppError::validation("原生键盘工具仅支持 Windows。"))
}

/// 使用扫描码进入 Windows 输入通道；发送成功与游戏收到/正确处理分开记录。
#[cfg(windows)]
fn native_stroke(
    pid: u32,
    scan: u16,
    extended: bool,
    hold: u64,
    alive: impl Fn() -> bool,
) -> AppResult<Value> {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    };
    static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _lock = GATE
        .try_lock()
        .map_err(|_| AppError::validation("另一次原生输入仍在执行。"))?;
    struct Target {
        pid: u32,
        hwnd: HWND,
    }
    unsafe extern "system" fn find(hwnd: HWND, param: LPARAM) -> i32 {
        let target = unsafe { &mut *(param as *mut Target) };
        let mut owner = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut owner) };
        if owner == target.pid
            && unsafe { IsWindowVisible(hwnd) } != 0
            && unsafe { GetWindow(hwnd, GW_OWNER) }.is_null()
        {
            target.hwnd = hwnd;
            return 0;
        }
        1
    }
    fn owns(hwnd: HWND, pid: u32) -> bool {
        let mut owner = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut owner) };
        owner == pid && !hwnd.is_null()
    }
    fn send(scan: u16, flags: u32) -> u32 {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: 0,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) }
    }
    let mut target = Target {
        pid,
        hwnd: std::ptr::null_mut(),
    };
    unsafe { EnumWindows(Some(find), &mut target as *mut _ as LPARAM) };
    if target.hwnd.is_null() || !alive() {
        return Err(AppError::validation("托管进程没有可用窗口。"));
    }
    let vk = unsafe { MapVirtualKeyW(u32::from(scan), MAPVK_VSC_TO_VK) };
    if [
        VK_SHIFT as i32,
        VK_CONTROL as i32,
        VK_MENU as i32,
        VK_LWIN as i32,
        VK_RWIN as i32,
        vk as i32,
    ]
    .iter()
    .any(|k| unsafe { GetAsyncKeyState(*k) } < 0)
    {
        return Err(AppError::validation("检测到用户按键尚未松开，未发送输入。"));
    }
    unsafe {
        ShowWindow(target.hwnd, SW_RESTORE);
        SetForegroundWindow(target.hwnd);
    }
    if unsafe { GetForegroundWindow() } != target.hwnd {
        // 后台工作线程临时连接输入队列，只激活已验证的目标，随即解除连接。
        use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        let caller = unsafe { GetCurrentThreadId() };
        let foreground =
            unsafe { GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut()) };
        let target_thread = unsafe { GetWindowThreadProcessId(target.hwnd, std::ptr::null_mut()) };
        let mut message: MSG = unsafe { std::mem::zeroed() };
        unsafe {
            PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        }
        let mut attached = Vec::new();
        for thread in [foreground, target_thread] {
            if thread != 0
                && thread != caller
                && !attached.contains(&thread)
                && unsafe { AttachThreadInput(caller, thread, 1) } != 0
            {
                attached.push(thread);
            }
        }
        unsafe {
            BringWindowToTop(target.hwnd);
            SetForegroundWindow(target.hwnd);
            SetFocus(target.hwnd);
        }
        for thread in attached {
            unsafe {
                AttachThreadInput(caller, thread, 0);
            }
        }
    }
    std::thread::sleep(Duration::from_millis(120));
    if unsafe { GetForegroundWindow() } != target.hwnd || !owns(target.hwnd, pid) || !alive() {
        return Err(AppError::validation("目标窗口未获得前台焦点，未发送输入。"));
    }
    let flags = KEYEVENTF_SCANCODE | if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
    // 一旦按下，作用域退出必须松键；不再向其他窗口发送新的按下事件。
    struct Release {
        scan: u16,
        flags: u32,
    }
    impl Drop for Release {
        fn drop(&mut self) {
            let _ = send(self.scan, self.flags | KEYEVENTF_KEYUP);
        }
    }
    let down = send(scan, flags);
    if down != 1 {
        return Err(AppError::validation(
            "Windows 拒绝按键输入，可能存在权限级别差异。",
        ));
    }
    let release = Release { scan, flags };
    let mut focus_lost = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(hold) {
        if unsafe { GetForegroundWindow() } != target.hwnd || !alive() {
            focus_lost = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let up = send(scan, flags | KEYEVENTF_KEYUP);
    if up == 1 {
        std::mem::forget(release);
    } else {
        drop(release);
    }
    Ok(
        json!({"downSent":down==1,"upSent":up==1,"focusLost":focus_lost,
        "completed":up==1&&!focus_lost,"scanCode":scan,"extended":extended,
        "note":"Windows 系统注入输入，不是物理硬件来源证明；需核对游戏状态和画面。"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_keyboard_limits_game_keys() {
        assert_eq!(scan_code("Escape").unwrap(), (1, false));
        assert_eq!(scan_code("ArrowUp").unwrap(), (0x48, true));
        for key in ["Alt", "Win", "Ctrl+C", "text", "F4", ""] {
            assert!(scan_code(key).is_err());
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "显式运行：短暂激活自有测试窗口并发送一次 Escape"]
    fn native_keyboard_roundtrip_receives_down_and_up() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
        static DOWN: AtomicUsize = AtomicUsize::new(0);
        static UP: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
            if w == 27 && msg == WM_KEYDOWN {
                DOWN.fetch_add(1, Ordering::SeqCst);
            }
            if w == 27 && msg == WM_KEYUP {
                UP.fetch_add(1, Ordering::SeqCst);
            }
            if msg == WM_DESTROY {
                unsafe { PostQuitMessage(0) };
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, w, l) }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let title: Vec<u16> = "ABYA native keyboard verification\0"
                .encode_utf16()
                .collect();
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    80,
                    80,
                    420,
                    180,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            assert!(!hwnd.is_null());
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc as *const () as isize);
            }
            tx.send(hwnd as usize).unwrap();
            let mut message: MSG = unsafe { std::mem::zeroed() };
            while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        });
        let hwnd = rx.recv_timeout(Duration::from_secs(5)).unwrap() as HWND;
        let result = native_stroke(std::process::id(), 1, false, 100, || true);
        std::thread::sleep(Duration::from_millis(150));
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        worker.join().unwrap();
        let receipt = result.unwrap();
        assert_eq!(receipt["completed"], true);
        assert_eq!(DOWN.load(Ordering::SeqCst), 1);
        assert_eq!(UP.load(Ordering::SeqCst), 1);
        println!("Native keyboard fixture: {receipt}; down=1 up=1");
    }
}
