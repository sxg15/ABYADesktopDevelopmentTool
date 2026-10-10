use super::{AppError, AppResult};
use serde::Serialize;
use windows_sys::Win32::System::{DataExchange::*, Memory::*};

/// 仅在用户点击复制时写入；调用方传入桌面窗口句柄，不记录文本。
pub fn write_text(text: &str, owner: isize) -> AppResult<()> {
    use windows_sys::Win32::Foundation::GlobalFree;
    if owner == 0 || text.contains('\0') {
        return Err(AppError::validation("Clipboard text or window is invalid."));
    }
    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }
    struct MemoryGuard(windows_sys::Win32::Foundation::HGLOBAL);
    impl Drop for MemoryGuard {
        fn drop(&mut self) {
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
    unsafe {
        let size = text
            .encode_utf16()
            .count()
            .checked_add(1)
            .and_then(|units| units.checked_mul(2))
            .ok_or_else(|| AppError::validation("Clipboard text is too large."))?;
        let memory = GlobalAlloc(GMEM_MOVEABLE, size);
        if memory.is_null() {
            return Err(AppError::validation("Cannot allocate clipboard text."));
        }
        let memory = MemoryGuard(memory);
        let pointer = GlobalLock(memory.0) as *mut u16;
        if pointer.is_null() {
            return Err(AppError::validation("Cannot prepare clipboard text."));
        }
        for (index, unit) in text.encode_utf16().chain(std::iter::once(0)).enumerate() {
            pointer.add(index).write(unit);
        }
        GlobalUnlock(memory.0);
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(owner as _) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            return Err(AppError::validation(
                "Clipboard is busy. Try copying again.",
            ));
        }
        let _clipboard = ClipboardGuard;
        if EmptyClipboard() == 0 || SetClipboardData(13, memory.0).is_null() {
            return Err(AppError::validation("Cannot write clipboard text."));
        }
        // 成功后 Windows 接管内存；失败路径由 MemoryGuard 释放。
        std::mem::forget(memory);
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClipboardContent {
    Text { text: String },
    Image,
    Empty,
}

/// Called only by an explicit terminal paste gesture. Never logs clipboard data.
pub fn read() -> AppResult<ClipboardContent> {
    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }
    unsafe {
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            return Err(AppError::validation(
                "Clipboard is busy. Try pasting again.",
            ));
        }
        let _guard = ClipboardGuard;
        // CF_UNICODETEXT; mixed HTML/image clipboard formats prefer plain text.
        if IsClipboardFormatAvailable(13) != 0 {
            let handle = GetClipboardData(13);
            if handle.is_null() {
                return Err(AppError::validation("Clipboard text is unavailable."));
            }
            let size = GlobalSize(handle);
            if size > 2 * 1024 * 1024 + 2 {
                return Err(AppError::validation("Clipboard text exceeds 1 MiB."));
            }
            let pointer = GlobalLock(handle) as *const u16;
            if pointer.is_null() {
                return Err(AppError::validation("Clipboard text is unavailable."));
            }
            let units = std::slice::from_raw_parts(pointer, size / 2);
            let end = units.iter().position(|c| *c == 0).unwrap_or(units.len());
            let text = String::from_utf16_lossy(&units[..end]);
            GlobalUnlock(handle);
            if text.len() > 1024 * 1024 {
                return Err(AppError::validation("Clipboard text exceeds 1 MiB."));
            }
            return Ok(ClipboardContent::Text { text });
        }
        // CF_BITMAP, CF_DIB, CF_DIBV5, or PNG: let the native provider attach the image.
        let png = RegisterClipboardFormatW("PNG\0".encode_utf16().collect::<Vec<_>>().as_ptr());
        if [2, 8, 17, png]
            .iter()
            .any(|format| *format != 0 && IsClipboardFormatAvailable(*format) != 0)
        {
            return Ok(ClipboardContent::Image);
        }
        Ok(ClipboardContent::Empty)
    }
}
