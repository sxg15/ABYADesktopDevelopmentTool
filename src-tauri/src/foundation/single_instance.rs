use super::*;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::{Duration, Instant};
#[cfg(windows)]
use windows_sys::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};

pub struct InstanceGuard {
    #[cfg(windows)]
    handle: HANDLE,
}
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub fn acquire(directory: &Path) -> AppResult<Option<InstanceGuard>> {
    #[cfg(windows)]
    unsafe {
        let identity = format!(
            "{}\\{}:{}",
            std::env::var("USERDOMAIN").unwrap_or_default(),
            std::env::var("USERNAME").unwrap_or_default(),
            directory.canonicalize()?.to_string_lossy().to_lowercase()
        );
        let name = format!(
            "Global\\ABYA.Desktop.{:x}",
            Sha256::digest(identity.as_bytes())
        );
        let wide = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let handle = CreateMutexW(std::ptr::null(), 0, wide.as_ptr());
        if handle.is_null() {
            return Err(AppError::internal(std::io::Error::last_os_error()));
        }
        let existing = GetLastError() == ERROR_ALREADY_EXISTS;
        let guard = InstanceGuard { handle };
        let record = directory.join("desktop-instance.json");
        if existing {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                if let Ok(bytes) = std::fs::read(&record)
                    && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
                    && let Some(pid) = v["pid"].as_u64()
                    && creation_time(pid as u32) == v["created"].as_u64()
                    && v["created"].is_u64()
                {
                    let mut target = WindowTarget {
                        pid: pid as u32,
                        found: false,
                    };
                    EnumWindows(Some(focus_window), &mut target as *mut _ as isize);
                    if target.found {
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            drop(guard);
            return Ok(None);
        }
        std::fs::write(
            record,
            serde_json::to_vec(
                &serde_json::json!({"pid":std::process::id(),"created":creation_time(std::process::id()),"release":env!("ABYA_RELEASE_ID")}),
            )?,
        )?;
        Ok(Some(guard))
    }
    #[cfg(not(windows))]
    {
        let _ = directory;
        Ok(Some(InstanceGuard {}))
    }
}
#[cfg(windows)]
struct WindowTarget {
    pid: u32,
    found: bool,
}
#[cfg(windows)]
fn creation_time(pid: u32) -> Option<u64> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        let ok = GetProcessTimes(handle, &mut created, &mut exit, &mut kernel, &mut user) != 0;
        CloseHandle(handle);
        ok.then_some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }
}
#[cfg(windows)]
unsafe extern "system" fn focus_window(window: HWND, param: LPARAM) -> i32 {
    unsafe {
        let target = &mut *(param as *mut WindowTarget);
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        if pid == target.pid && IsWindowVisible(window) != 0 {
            ShowWindow(window, SW_RESTORE);
            SetForegroundWindow(window);
            target.found = true;
            return 0;
        }
    }
    1
}

pub fn redirect_packaged_launch() -> AppResult<bool> {
    if std::env::var("ABYA_TEST_MODE").as_deref() == Ok("1") {
        return Ok(false);
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
        let mut length = 0;
        let code = GetCurrentPackageFullName(&mut length, std::ptr::null_mut());
        if code == ERROR_INSUFFICIENT_BUFFER && length > 0 {
            let mut name = vec![0u16; length as usize];
            if GetCurrentPackageFullName(&mut length, name.as_mut_ptr()) == 0
                && String::from_utf16_lossy(&name).starts_with("OpenAI.Codex_")
            {
                use std::os::windows::process::CommandExt;
                std::process::Command::new("explorer.exe")
                    .arg(std::env::current_exe()?)
                    .creation_flags(0x08000000)
                    .spawn()?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}
