use super::{InstanceOrigin, InstanceService, ProcessState, WindowVisibilityMode};
use crate::foundation::{AppError, AppResult};
use chrono::Utc;
use process_wrap::std::{ChildWrapper, CommandWrap, JobObject};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub schema: String,
    pub id: String,
    pub task_id: String,
    pub instance_id: String,
    pub path: String,
    pub metadata_path: String,
    pub source_version: Option<String>,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub frame_count: u64,
    pub audio: bool,
}
pub(super) struct LiveRecording {
    child: Box<dyn ChildWrapper>,
    info: RecordingInfo,
    directory: PathBuf,
    restore_visibility: WindowVisibilityMode,
}
struct RestoreVisibility<'a> {
    service: &'a InstanceService,
    id: &'a str,
    mode: WindowVisibilityMode,
    armed: bool,
}
impl Drop for RestoreVisibility<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.service.set_window_visibility(self.id, self.mode);
        }
    }
}

pub fn recorder_executable() -> AppResult<PathBuf> {
    let beside = std::env::current_exe()?
        .parent()
        .unwrap()
        .join("tools/ffmpeg/runtime/bin/ffmpeg.exe");
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools/ffmpeg/runtime/bin/ffmpeg.exe");
    [beside, source]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| {
            AppError::validation("录制组件缺失，请运行 setup-recorder 或使用包含录制组件的便携包。")
        })
}
fn persist(directory: &Path, info: &RecordingInfo) -> AppResult<()> {
    std::fs::write(
        directory.join("recording.json"),
        serde_json::to_vec_pretty(info)?,
    )?;
    Ok(())
}
fn frames(directory: &Path) -> u64 {
    std::fs::read_to_string(directory.join("progress.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            l.strip_prefix("frame=")
                .and_then(|n| n.trim().parse::<u64>().ok())
        })
        .max()
        .unwrap_or(0)
}

fn capture_command(hwnd: isize, output: &Path, max_seconds: u32) -> AppResult<Command> {
    let mut command = Command::new(recorder_executable()?);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-f",
            "gdigrab",
            "-framerate",
            "15",
            "-draw_mouse",
            "0",
            "-i",
        ])
        .arg(format!("hwnd={hwnd}"))
        .args([
            "-t",
            &max_seconds.to_string(),
            "-an",
            "-vf",
            "pad=ceil(iw/2)*2:ceil(ih/2)*2",
            "-c:v",
            "libopenh264",
            "-b:v",
            "4M",
            "-pix_fmt",
            "yuv420p",
            "-progress",
        ])
        .arg(output.join("progress.txt"))
        .args(["-movflags", "+faststart"])
        .arg(output.join("capture.mp4"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(output.join("recorder.log"))?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}

impl InstanceService {
    pub fn recording_start(
        &self,
        task_id: &str,
        id: &str,
        directory: &Path,
        source_version: Option<String>,
        max_seconds: u32,
    ) -> AppResult<RecordingInfo> {
        let instance = self.read(id)?;
        if instance.origin != InstanceOrigin::Managed
            || instance.task_id.as_deref() != Some(task_id)
            || instance.process_state != ProcessState::Running
        {
            return Err(AppError::validation(
                "只能录制本任务正在运行的受管游戏实例。",
            ));
        }
        if !(5..=7200).contains(&max_seconds) {
            return Err(AppError::validation("录制上限应为 5—7200 秒。"));
        }
        let mut active = self.recordings.lock();
        if active.contains_key(id) {
            return Err(AppError::validation("此实例已有录制，请先停止并取回结果。"));
        }
        let restore_visibility = instance
            .profile
            .as_ref()
            .map_or(WindowVisibilityMode::Visible, |p| p.visibility_mode);
        let mut restore = RestoreVisibility {
            service: self,
            id,
            mode: restore_visibility,
            armed: true,
        };
        self.set_window_visibility(id, WindowVisibilityMode::Visible)?;
        std::thread::sleep(Duration::from_millis(250));
        let hwnd = window_handle(
            instance
                .pid
                .ok_or_else(|| AppError::validation("实例无 PID。"))?,
        )?;
        raise_without_focus(hwnd);
        let recording_id = uuid::Uuid::new_v4().to_string();
        let output = directory.join(&recording_id);
        std::fs::create_dir_all(&output)?;
        let relative = format!("artifacts/runtime/{id}/recordings/{recording_id}");
        let info = RecordingInfo {
            schema: "abya.recording/v1".into(),
            id: recording_id,
            task_id: task_id.into(),
            instance_id: id.into(),
            path: format!("{relative}/capture.mp4"),
            metadata_path: format!("{relative}/recording.json"),
            source_version,
            status: "recording".into(),
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            frame_count: 0,
            audio: false,
        };
        let command = capture_command(hwnd, &output, max_seconds)?;
        let mut wrap = CommandWrap::from(command);
        wrap.wrap(JobObject);
        let mut child = wrap.spawn()?;
        std::thread::sleep(Duration::from_millis(400));
        if let Some(status) = child.try_wait()? {
            let _ = self.set_window_visibility(id, restore_visibility);
            return Err(AppError::validation(format!(
                "录制启动失败 ({status})，检查 {}",
                output.join("recorder.log").display()
            )));
        }
        persist(&output, &info)?;
        active.insert(
            id.into(),
            LiveRecording {
                child,
                info: info.clone(),
                directory: output,
                restore_visibility,
            },
        );
        restore.armed = false;
        let service = self.clone();
        let instance_id = id.to_owned();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let visibility = {
                    let mut recordings = service.recordings.lock();
                    let Some(run) = recordings.get_mut(&instance_id) else {
                        return;
                    };
                    match run.child.try_wait() {
                        Ok(Some(status)) => {
                            run.info.frame_count = frames(&run.directory);
                            run.info.status = if status.success() && run.info.frame_count > 0 {
                                "completed"
                            } else {
                                "failed"
                            }
                            .into();
                            run.info.ended_at = Some(Utc::now().to_rfc3339());
                            let _ = persist(&run.directory, &run.info);
                            Some(run.restore_visibility)
                        }
                        Ok(None) => None,
                        Err(_) => Some(run.restore_visibility),
                    }
                };
                if let Some(mode) = visibility {
                    let _ = service.set_window_visibility(&instance_id, mode);
                    return;
                }
            }
        });
        Ok(info)
    }
}

impl InstanceService {
    pub fn recording_get(&self, id: &str) -> AppResult<RecordingInfo> {
        let mut active = self.recordings.lock();
        let run = active
            .get_mut(id)
            .ok_or_else(|| AppError::not_found("Active recording"))?;
        run.info.frame_count = frames(&run.directory);
        if let Some(status) = run.child.try_wait()? {
            run.info.status = if status.success() && run.info.frame_count > 0 {
                "completed"
            } else {
                "failed"
            }
            .into();
            run.info.ended_at = Some(Utc::now().to_rfc3339());
            persist(&run.directory, &run.info)?;
        }
        Ok(run.info.clone())
    }

    pub fn recording_stop(&self, id: &str) -> AppResult<RecordingInfo> {
        let mut run = self
            .recordings
            .lock()
            .remove(id)
            .ok_or_else(|| AppError::not_found("Active recording"))?;
        if let Some(stdin) = run.child.stdin().as_mut() {
            let _ = stdin.write_all(b"q\n");
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        let success = loop {
            if let Some(status) = run.child.try_wait()? {
                break status.success();
            }
            if Instant::now() >= deadline {
                let _ = run.child.start_kill();
                let _ = run.child.wait();
                break false;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        run.info.frame_count = frames(&run.directory);
        run.info.status = if success && run.info.frame_count > 0 {
            "completed"
        } else {
            "interrupted"
        }
        .into();
        run.info.ended_at = Some(Utc::now().to_rfc3339());
        persist(&run.directory, &run.info)?;
        let _ = self.set_window_visibility(id, run.restore_visibility);
        Ok(run.info)
    }
    pub(super) fn stop_instance_recording(&self, id: &str) {
        if self.recordings.lock().contains_key(id) {
            let _ = self.recording_stop(id);
        }
    }
}

#[cfg(windows)]
fn raise_without_focus(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        SetWindowPos(
            hwnd as _,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}
#[cfg(not(windows))]
fn raise_without_focus(_hwnd: isize) {}

#[cfg(windows)]
fn window_handle(pid: u32) -> AppResult<isize> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClientRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };
    struct Search {
        pid: u32,
        hwnd: isize,
        area: i64,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> windows_sys::core::BOOL {
        let s = unsafe { &mut *(data as *mut Search) };
        let mut owner = 0;
        let mut rect = RECT::default();
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
        }
        if owner == s.pid
            && unsafe {
                IsWindowVisible(hwnd) != 0
                    && IsIconic(hwnd) == 0
                    && GetClientRect(hwnd, &mut rect) != 0
            }
        {
            let area = i64::from(rect.right - rect.left) * i64::from(rect.bottom - rect.top);
            if area > s.area {
                s.hwnd = hwnd as isize;
                s.area = area;
            }
        }
        1
    }
    let mut search = Search {
        pid,
        hwnd: 0,
        area: 1024,
    };
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
    }
    if search.hwnd == 0 {
        return Err(AppError::validation(
            "未找到可录制的实例窗口，请等待窗口就绪且不要最小化。",
        ));
    }
    Ok(search.hwnd)
}
#[cfg(not(windows))]
fn window_handle(_pid: u32) -> AppResult<isize> {
    Err(AppError::validation("当前录制组件只支持 Windows。"))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Uses the installed recorder and a brief owned non-activating Windows fixture"]
    fn recorder_smoke_captures_owned_window() {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let title: Vec<u16> = "ABYA Recorder Smoke\0".encode_utf16().collect();
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_VISIBLE,
                24,
                24,
                320,
                240,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        struct Window(windows_sys::Win32::Foundation::HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let _window = Window(hwnd);
        assert_eq!(window_handle(std::process::id()).unwrap(), hwnd as isize);
        let output =
            std::env::temp_dir().join(format!("abya-recorder-smoke-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&output).unwrap();
        let mut command = capture_command(hwnd as isize, &output, 2).unwrap();
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(12);
        let status = loop {
            let mut message = MSG::default();
            unsafe {
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                panic!("recorder timeout");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(
            status.success(),
            "{}",
            std::fs::read_to_string(output.join("recorder.log")).unwrap()
        );
        assert!(frames(&output) > 0);
        let probe = recorder_executable().unwrap().with_file_name("ffprobe.exe");
        let result = Command::new(probe)
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=codec_name,width,height",
                "-of",
                "json",
            ])
            .arg(output.join("capture.mp4"))
            .output()
            .unwrap();
        let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(data["streams"][0]["width"], 320);
        assert_eq!(data["streams"][0]["codec_name"], "h264");
        println!("Owned recording fixture: {}", output.display());
    }
}
