use super::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopConnection {
    pub version: u32,
    pub endpoint: String,
    pub encrypted_token: String,
    #[serde(default)]
    route_proof: String,
}

pub fn descriptor_path() -> AppResult<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| AppError::validation("Windows user storage is unavailable."))?;
    Ok(PathBuf::from(root).join("ABYA Desktop Development Tool/Data/cli-connection.json"))
}

pub fn publish(endpoint: &str, token: &str) -> AppResult<()> {
    let file = descriptor_path()?;
    std::fs::create_dir_all(file.parent().unwrap())?;
    let encrypted_token = super::settings::encrypt_token(token)?;
    let value = DesktopConnection {
        version: 1,
        endpoint: endpoint.into(),
        route_proof: super::settings::encrypt_token(&route_identity(endpoint, &encrypted_token))?,
        encrypted_token,
    };
    // Only DPAPI ciphertext is persisted; the CLI never prints this descriptor.
    let temporary = file.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, serde_json::to_vec(&value)?)?;
    std::fs::rename(temporary, file)?;
    Ok(())
}

fn route_identity(endpoint: &str, ciphertext: &str) -> String {
    format!("{endpoint}\n{:x}", Sha256::digest(ciphertext.as_bytes()))
}

#[derive(Serialize, Deserialize)]
struct TaskLocator {
    task: String,
    workspace: PathBuf,
    descriptor: PathBuf,
    #[serde(default)]
    connection: Option<DesktopConnection>,
}

pub fn task_locator(task: &str, workspace: &Path) -> AppResult<String> {
    let record = TaskLocator {
        task: task.to_owned(),
        workspace: workspace.canonicalize()?,
        descriptor: descriptor_path()?,
        connection: Some(serde_json::from_slice(&std::fs::read(descriptor_path()?)?)?),
    };
    super::settings::encrypt_token(&serde_json::to_string(&record)?)
}

pub fn locate_task_connection(locator: &str, task: &str, workspace: &Path) -> AppResult<PathBuf> {
    if locator.len() > 64 * 1024 {
        return Err(AppError::validation("任务连接信息过长。"));
    }
    let raw = super::settings::decrypt_token(locator).map_err(|_| {
        AppError::validation(
            "当前 Windows 用户无法读取任务连接。请用相同账户重新打开开发 APP 的任务对话。",
        )
    })?;
    let record: TaskLocator = serde_json::from_str(&raw)?;
    if record.task != task
        || !record
            .workspace
            .to_string_lossy()
            .eq_ignore_ascii_case(&workspace.canonicalize()?.to_string_lossy())
        || !record.descriptor.is_absolute()
        || !record
            .descriptor
            .ends_with("ABYA Desktop Development Tool/Data/cli-connection.json")
    {
        return Err(AppError::validation(
            "任务、工作区与 APP 保存的连接不匹配。请重新打开任务对话。",
        ));
    }
    Ok(record.descriptor)
}

pub fn connection(file: Option<&Path>) -> AppResult<(String, String)> {
    let default_path;
    let path = if let Some(path) = file {
        path
    } else {
        default_path = descriptor_path()?;
        &default_path
    };
    let value: DesktopConnection = serde_json::from_slice(&std::fs::read(path).map_err(|_| {
        AppError::validation("找不到该任务对应的开发 APP 连接，请重新打开开发 APP。")
    })?)?;
    verified_connection(value, file.is_some())
}

pub fn task_connection(locator: &str, task: &str, workspace: &Path) -> AppResult<(String, String)> {
    let descriptor = locate_task_connection(locator, task, workspace)?;
    let raw = super::settings::decrypt_token(locator)?;
    let record: TaskLocator = serde_json::from_str(&raw)?;
    match record.connection {
        Some(value) => verified_connection(value, true),
        None => connection(Some(&descriptor)),
    }
}

fn verified_connection(
    value: DesktopConnection,
    require_proof: bool,
) -> AppResult<(String, String)> {
    if require_proof || !value.route_proof.is_empty() {
        let proof = super::settings::decrypt_token(&value.route_proof).map_err(|_| {
            AppError::validation("当前用户无法验证开发 APP 连接，请重新打开任务对话。")
        })?;
        if proof != route_identity(&value.endpoint, &value.encrypted_token) {
            return Err(AppError::validation(
                "开发 APP 连接已变化，请重新打开任务对话。",
            ));
        }
    }
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
        super::settings::decrypt_token(&value.encrypted_token).map_err(|_| {
            AppError::new(
                "desktop_identity_mismatch",
                "当前 Windows 用户无法使用该开发 APP 的连接，请在同一账户下重新打开任务对话。",
                "",
            )
        })?,
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

#[cfg(test)]
mod task_locator_tests {
    use super::*;
    #[test]
    fn protected_task_route_survives_a_missing_or_redirected_local_descriptor() {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let endpoint = "http://127.0.0.1:12345/api/v1/command";
        let token = super::super::settings::encrypt_token("fixture-only").unwrap();
        let mut record = TaskLocator {
            task: "route-fixture".into(),
            workspace: root.clone(),
            descriptor: root
                .join(uuid::Uuid::new_v4().to_string())
                .join("ABYA Desktop Development Tool/Data/cli-connection.json"),
            connection: Some(DesktopConnection {
                version: 1,
                endpoint: endpoint.into(),
                route_proof: super::super::settings::encrypt_token(&route_identity(
                    endpoint, &token,
                ))
                .unwrap(),
                encrypted_token: token,
            }),
        };
        let protect = |r: &TaskLocator| {
            super::super::settings::encrypt_token(&serde_json::to_string(r).unwrap()).unwrap()
        };
        let locator = protect(&record);
        assert_eq!(
            task_connection(&locator, "route-fixture", &root).unwrap(),
            (endpoint.into(), "fixture-only".into())
        );
        assert!(task_connection(&locator, "another-task", &root).is_err());
        record.connection.as_mut().unwrap().endpoint =
            "http://127.0.0.1:12346/api/v1/command".into();
        assert!(task_connection(&protect(&record), "route-fixture", &root).is_err());
    }
    #[test]
    fn a_copied_marker_cannot_select_another_tasks_connection() {
        let root = std::env::temp_dir();
        let locator = task_locator("a", &root).unwrap();
        assert_eq!(
            locate_task_connection(&locator, "a", &root).unwrap(),
            descriptor_path().unwrap()
        );
        assert!(locate_task_connection(&locator, "b", &root).is_err());
        assert!(locate_task_connection(&locator, "a", root.parent().unwrap()).is_err());
    }

    #[test]
    fn descriptor_cannot_redirect_the_original_windows_credential() {
        let temp =
            std::env::temp_dir().join(format!("abya-connection-{}.json", uuid::Uuid::new_v4()));
        let encrypted_token = super::super::settings::encrypt_token("fixture-token").unwrap();
        let endpoint = "http://127.0.0.1:12345/api/v1/command";
        let mut descriptor = DesktopConnection {
            version: 1,
            endpoint: endpoint.into(),
            route_proof: super::super::settings::encrypt_token(&route_identity(
                endpoint,
                &encrypted_token,
            ))
            .unwrap(),
            encrypted_token,
        };
        std::fs::write(&temp, serde_json::to_vec(&descriptor).unwrap()).unwrap();
        assert_eq!(connection(Some(&temp)).unwrap().0, endpoint);
        descriptor.endpoint = "http://127.0.0.1:12346/api/v1/command".into();
        std::fs::write(&temp, serde_json::to_vec(&descriptor).unwrap()).unwrap();
        assert!(connection(Some(&temp)).is_err());
        std::fs::remove_file(temp).unwrap();
    }
}
