use super::{ArchiveSelection, LaunchMode, LaunchProfile, WindowMode, WindowVisibilityMode};
use crate::foundation::{AppError, AppPaths, AppResult};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use std::net::TcpListener;
use std::path::Path;

pub(crate) struct LaunchContract {
    pub args: Vec<String>,
    pub sanitized_args: Vec<String>,
    pub host_port: Option<u16>,
    pub log_path: String,
    pub mcp_endpoint: String,
    pub mcp_token: String,
}

pub(crate) fn validate_executable(path: &str) -> AppResult<()> {
    let path = Path::new(path);
    if !path.is_file() {
        return Err(AppError::validation("Select an existing game executable."));
    }
    if !path
        .extension()
        .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("exe"))
    {
        return Err(AppError::validation(
            "The game executable must be an .exe file.",
        ));
    }
    Ok(())
}

pub(crate) fn build_launch_contract(
    instance_id: &str,
    instance_name: &str,
    profile: &LaunchProfile,
    paths: &AppPaths,
    inherited_host_port: Option<u16>,
    gateway_endpoint: &str,
) -> AppResult<LaunchContract> {
    if !(640..=7680).contains(&profile.width) || !(480..=4320).contains(&profile.height) {
        return Err(AppError::validation(
            "Resolution must be between 640x480 and 7680x4320.",
        ));
    }
    if profile.visibility_mode == WindowVisibilityMode::Background
        && !matches!(profile.window_mode, WindowMode::Windowed)
    {
        return Err(AppError::validation(
            "Background instances require windowed rendering.",
        ));
    }
    validate_archive(profile.mode, profile.archive.as_ref())?;
    if !gateway_endpoint.starts_with("ws://") {
        return Err(AppError::validation(
            "The selected LAN adapter did not provide a WebSocket endpoint.",
        ));
    }
    let host_port = match profile.mode {
        LaunchMode::LanHost => Some(allocate_port()?),
        LaunchMode::LanClient => Some(
            inherited_host_port
                .ok_or_else(|| AppError::validation("Select a running Host instance."))?,
        ),
        _ => None,
    };
    let mcp_port = allocate_port_excluding(host_port)?;
    let mcp_token = generate_mcp_token();
    let short_id = instance_id.chars().take(8).collect::<String>();
    let user_id = format!("abya-desktop-{short_id}");
    let user_name = format!(
        "{} {}",
        instance_name.trim().chars().take(32).collect::<String>(),
        short_id
    );
    let report_path = AppPaths::display(&paths.instance_report_path(instance_id));
    let log_path = AppPaths::display(&paths.instance_log_path(instance_id));
    let mut args = vec![
        "-screen-width".to_string(),
        profile.width.to_string(),
        "-screen-height".to_string(),
        profile.height.to_string(),
    ];
    match profile.window_mode {
        WindowMode::Windowed => args.extend(["-screen-fullscreen".into(), "0".into()]),
        WindowMode::Borderless => args.push("-popupwindow".into()),
        WindowMode::Fullscreen => args.extend(["-screen-fullscreen".into(), "1".into()]),
    }
    args.extend([
        "-logFile".into(),
        log_path.clone(),
        format!("--abya-test-user-id={user_id}"),
        format!("--abya-test-user-name={user_name}"),
        format!("--abya-launch-mode={}", profile.mode.as_db()),
        format!("--abya-launch-report={report_path}"),
        format!("--abya-launch-exit-on-failure={}", profile.exit_on_failure),
        format!("--abya-mcp-port={mcp_port}"),
        format!("--abya-mcp-token={mcp_token}"),
        "--abya-mcp-autostart=true".into(),
        "--abya-mcp-auto-approve=true".into(),
        format!("--abya-devtool-endpoint={gateway_endpoint}"),
        format!("--abya-devtool-instance-id={instance_id}"),
        "--abya-devtool-protocol=1".into(),
        "--abya-devtool-autoconnect=true".into(),
    ]);
    if !profile.language.trim().is_empty() {
        args.push(format!("-language={}", profile.language.trim()));
    }
    if matches!(profile.mode, LaunchMode::LanHost | LaunchMode::LanClient) {
        args.push("--abya-launch-host=127.0.0.1".into());
        args.push(format!(
            "--abya-launch-host-port={}",
            host_port.unwrap_or(7777)
        ));
    }
    if let Some(archive) = &profile.archive {
        append_archive_args(&mut args, archive);
    }
    let sanitized_args = args
        .iter()
        .map(|argument| {
            if argument.starts_with("--abya-mcp-token=") {
                "--abya-mcp-token=[REDACTED]".to_string()
            } else {
                argument.clone()
            }
        })
        .collect();
    Ok(LaunchContract {
        args,
        sanitized_args,
        host_port,
        log_path,
        mcp_endpoint: format!("http://127.0.0.1:{mcp_port}/mcp"),
        mcp_token,
    })
}

fn validate_archive(mode: LaunchMode, archive: Option<&ArchiveSelection>) -> AppResult<()> {
    if mode == LaunchMode::IgpHosted {
        return Err(AppError::validation(
            "IGP hosted launch requires a secure in-memory IGP context that is not available in the desktop tool.",
        ));
    }
    if matches!(
        mode,
        LaunchMode::Editor | LaunchMode::Offline | LaunchMode::LanHost
    ) {
        let archive = archive
            .ok_or_else(|| AppError::validation("The selected launch mode requires an archive."))?;
        if archive.archive_guid.trim().is_empty() || archive.archive_path.trim().is_empty() {
            return Err(AppError::validation(
                "The selected launch mode requires a valid archive GUID and path.",
            ));
        }
        if !Path::new(&archive.archive_path)
            .join("Main.PBArc")
            .is_file()
        {
            return Err(AppError::validation(
                "The selected archive does not contain Main.PBArc.",
            ));
        }
    }
    Ok(())
}

fn append_archive_args(args: &mut Vec<String>, archive: &ArchiveSelection) {
    args.push(format!(
        "--abya-launch-archive-guid={}",
        archive.archive_guid
    ));
    args.push(format!(
        "--abya-launch-archive-path={}",
        archive.archive_path
    ));
    if !archive.level_guid.trim().is_empty() {
        args.push(format!("--abya-launch-level-guid={}", archive.level_guid));
    }
}

fn allocate_port() -> AppResult<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn allocate_port_excluding(excluded: Option<u16>) -> AppResult<u16> {
    for _ in 0..20 {
        let port = allocate_port()?;
        if Some(port) != excluded {
            return Ok(port);
        }
    }
    Err(AppError::new(
        "portAllocationFailed",
        "The desktop tool could not allocate a distinct Runtime MCP port.",
        "",
    ))
}

fn generate_mcp_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> AppPaths {
        let root = std::env::temp_dir().join("abya-launch-tests");
        AppPaths {
            database_path: root.join("app.db"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
            data_dir: root,
        }
    }

    #[test]
    fn arguments_use_production_launch_and_redact_runtime_mcp_token() {
        let archive_root =
            std::env::temp_dir().join(format!("abya-launch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&archive_root).unwrap();
        std::fs::write(archive_root.join("Main.PBArc"), "{}").unwrap();
        let profile = LaunchProfile {
            mode: LaunchMode::Offline,
            window_mode: WindowMode::Windowed,
            visibility_mode: WindowVisibilityMode::Background,
            width: 1280,
            height: 720,
            archive: Some(ArchiveSelection {
                archive_path: archive_root.to_string_lossy().into_owned(),
                archive_guid: "archive".into(),
                archive_name: "Archive".into(),
                level_guid: "level".into(),
                level_name: "Level".into(),
            }),
            host_instance_id: None,
            exit_on_failure: true,
            language: "zh".into(),
        };
        let contract = build_launch_contract(
            "12345678-rest",
            "Demo",
            &profile,
            &paths(),
            None,
            "ws://192.168.1.20:47610/game/v1/connect",
        )
        .unwrap();
        assert!(
            contract
                .args
                .iter()
                .any(|arg| arg
                    == "--abya-devtool-endpoint=ws://192.168.1.20:47610/game/v1/connect")
        );
        assert!(contract.args.contains(&"--abya-devtool-protocol=1".into()));
        assert!(
            contract
                .args
                .contains(&"--abya-devtool-autoconnect=true".into())
        );
        let joined = contract.args.join(" ");
        assert!(joined.contains("--abya-launch-mode=offline"));
        assert!(joined.contains("--abya-launch-archive-guid=archive"));
        assert!(joined.contains("--abya-launch-level-guid=level"));
        assert!(joined.contains("--abya-mcp-port="));
        assert!(joined.contains("--abya-mcp-token="));
        assert!(joined.contains("--abya-mcp-autostart=true"));
        assert!(joined.contains("--abya-mcp-auto-approve=true"));
        assert!(
            !contract
                .sanitized_args
                .join(" ")
                .contains(&contract.mcp_token)
        );
        assert!(
            contract
                .sanitized_args
                .contains(&"--abya-mcp-token=[REDACTED]".into())
        );
        assert_eq!(contract.mcp_token.len(), 43);
        let _ = std::fs::remove_dir_all(archive_root);
    }
}
