use super::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopConnection {
    pub version: u32,
    pub endpoint: String,
    pub encrypted_token: String,
}

pub fn descriptor_path() -> AppResult<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| AppError::validation("Windows user storage is unavailable."))?;
    Ok(PathBuf::from(root).join("ABYA Desktop Development Tool/Data/cli-connection.json"))
}

pub fn publish(endpoint: &str, token: &str) -> AppResult<()> {
    let file = descriptor_path()?;
    std::fs::create_dir_all(file.parent().unwrap())?;
    let value = DesktopConnection {
        version: 1,
        endpoint: endpoint.into(),
        encrypted_token: super::settings::encrypt_token(token)?,
    };
    // Only DPAPI ciphertext is persisted; the CLI never prints this descriptor.
    let temporary = file.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, serde_json::to_vec(&value)?)?;
    std::fs::rename(temporary, file)?;
    Ok(())
}

pub fn connection() -> AppResult<(String, String)> {
    let value: DesktopConnection = serde_json::from_slice(&std::fs::read(descriptor_path()?)?)?;
    let url = reqwest::Url::parse(&value.endpoint).map_err(AppError::internal)?;
    if value.version != 1
        || url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.path() != "/api/v1/command"
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AppError::validation(
            "Invalid desktop CLI connection descriptor.",
        ));
    }
    Ok((
        value.endpoint,
        super::settings::decrypt_token(&value.encrypted_token).map_err(|_| AppError::new(
            "desktop_identity_mismatch",
            "The current Windows identity cannot decrypt this Desktop connection. Reopen the task terminal to obtain its managed CLI session.", ""))?,
    ))
}

pub fn desktop_cli_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let sibling = exe.with_file_name("abya-desktop.exe");
    if sibling.is_file() || !cfg!(debug_assertions) {
        return sibling;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/abya-desktop.exe")
}

pub fn runtime_paths() -> AppResult<(PathBuf, PathBuf)> {
    let exe = std::env::current_exe()?;
    let root = exe
        .parent()
        .ok_or_else(|| AppError::validation("Missing application directory."))?;
    let script = root.join("tools/abya/bin/abya.mjs");
    let script = if script.is_file() {
        script
    } else if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tools/abya/bin/abya.mjs")
    } else {
        return Err(AppError::validation("Bundled Abya CLI is missing."));
    };
    if !script.is_file() {
        return Err(AppError::validation(
            "Bundled Abya CLI is missing. Reinstall the complete portable package.",
        ));
    }
    let node = root.join("runtime/node.exe");
    let node = if node.is_file() {
        node
    } else if cfg!(debug_assertions) {
        PathBuf::from("node.exe")
    } else {
        return Err(AppError::validation("Bundled Node runtime is missing."));
    };
    Ok((node, script))
}
