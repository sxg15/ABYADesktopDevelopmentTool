use super::{AppError, AppPaths, AppResult, build_info};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct UpdateFiles(PathBuf);
impl Drop for UpdateFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn prepare_and_handoff(
    paths: &AppPaths,
    commit: &str,
    number: u64,
    ensure_idle: impl Fn() -> AppResult<()>,
) -> AppResult<()> {
    if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::validation("无效的 Git 提交。"));
    }
    ensure_idle()?;
    let local = build_info::read(paths)?;
    if !local
        .release_number
        .is_some_and(|installed| installed < number)
    {
        return Err(AppError::validation("请从已验证的旧版发布包重新检查更新。"));
    }
    let package = Path::new(&local.executable)
        .parent()
        .ok_or_else(|| AppError::validation("找不到发布目录。"))?;
    let root = package
        .parent()
        .filter(|path| {
            package.file_name().is_some_and(|n| n == "Publish") && path.join(".git").exists()
        })
        .ok_or_else(|| {
            AppError::validation(
                "请用 GitHub Desktop 克隆项目，从克隆目录中的 Publish 启动 App，再原地更新。",
            )
        })?;
    let dir = std::env::temp_dir().join(format!("abya-update-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir)?;
    let files = UpdateFiles(dir);
    let script = files.0.join("update.ps1");
    fs::write(
        &script,
        [
            b"\xef\xbb\xbf".as_slice(),
            include_bytes!("update_checkout.ps1"),
        ]
        .concat(),
    )?;
    let powershell = PathBuf::from(
        std::env::var_os("SystemRoot")
            .ok_or_else(|| AppError::validation("Windows PowerShell 不可用。"))?,
    )
    .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let command = |phase: &str| {
        let mut cmd = Command::new(&powershell);
        cmd.current_dir(&files.0)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&script)
            .args(["-Phase", phase, "-Root"])
            .arg(root)
            .args([
                "-Commit",
                commit,
                "-Number",
                &number.to_string(),
                "-AppPid",
                &std::process::id().to_string(),
            ])
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd
    };
    let output = command("Prepare").output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::validation(if detail.trim().is_empty() {
            "Git 更新准备失败。"
        } else {
            detail.trim()
        }));
    }
    ensure_idle()?;
    let mut worker = command("Apply")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while !files.0.join("ready").is_file() {
        if worker.try_wait()?.is_some() {
            return Err(AppError::validation("更新程序启动失败，当前版本仍在运行。"));
        }
        if Instant::now() >= deadline {
            worker.kill()?;
            worker.wait()?;
            return Err(AppError::validation("更新程序未就绪，请重试。"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // The detached worker now owns cleanup and waits for normal application shutdown.
    std::mem::forget(files);
    Ok(())
}
