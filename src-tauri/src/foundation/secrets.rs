//! Current-user encrypted storage. Callers provide a distinct purpose string.
use super::{AppError, AppResult};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use windows_dpapi::{Scope, decrypt_data, encrypt_data};

pub fn protect(value: &str, purpose: &[u8]) -> AppResult<String> {
    encrypt_data(value.as_bytes(), Scope::User, Some(purpose))
        .map(|bytes| STANDARD.encode(bytes))
        .map_err(|_| AppError::new("secretStorage", "Windows credential protection failed.", ""))
}

pub fn unprotect(value: &str, purpose: &[u8]) -> AppResult<String> {
    let error = || {
        AppError::new(
            "secretStorage",
            "Sign in again on this Windows account.",
            "",
        )
    };
    let bytes = STANDARD.decode(value).map_err(|_| error())?;
    let plain = decrypt_data(&bytes, Scope::User, Some(purpose)).map_err(|_| error())?;
    String::from_utf8(plain).map_err(|_| error())
}
