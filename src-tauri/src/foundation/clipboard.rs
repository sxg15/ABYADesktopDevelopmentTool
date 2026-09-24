use super::{AppError, AppResult};
use serde::Serialize;
use windows_sys::Win32::System::{DataExchange::*, Memory::*};

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
