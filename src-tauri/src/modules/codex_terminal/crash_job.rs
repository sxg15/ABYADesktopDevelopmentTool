// Non-inheritable lifetime job: Windows closes it even if the APP is terminated.
use crate::foundation::{AppError, AppResult};
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::sync::Arc;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
pub(super) fn attach(process: BorrowedHandle<'_>) -> AppResult<Arc<OwnedHandle>> {
    unsafe {
        let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if raw.is_null() {
            return Err(AppError::internal(std::io::Error::last_os_error()));
        }
        let job = OwnedHandle::from_raw_handle(raw);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        ) == 0
            || AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) == 0
        {
            return Err(AppError::internal(std::io::Error::last_os_error()));
        }
        Ok(Arc::new(job))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use process_wrap::std::{CommandWrap, CreationFlags, JobObject};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    #[test]
    fn closing_lifetime_job_kills_owned_process_inside_existing_job() {
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(CreationFlags(
            windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(0x08000000),
        ));
        wrapped.wrap(JobObject);
        let mut child = wrapped.spawn().unwrap();
        let guard = attach(child.process_handle().unwrap()).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        drop(guard);
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        let exited = child.try_wait().unwrap().is_some();
        if !exited {
            let _ = child.kill();
        }
        assert!(
            exited,
            "Windows must terminate the owned child on job handle closure"
        );
    }
}
