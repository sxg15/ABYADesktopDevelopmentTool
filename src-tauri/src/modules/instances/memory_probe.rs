use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySample {
    pub at: String,
    pub private_bytes: u64,
    pub working_set_bytes: u64,
    pub available_bytes: u64,
    pub commit_bytes: u64,
    pub commit_limit_bytes: u64,
}

#[cfg(windows)]
pub(super) struct MemoryProbe(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl MemoryProbe {
    // 持有原进程句柄，防止 PID 被复用后采样到其他进程。
    pub fn new(pid: u32) -> std::io::Result<Self> {
        use windows_sys::Win32::System::Threading::*;
        let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
        if handle.is_null() {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }
    pub fn sample(&self) -> std::io::Result<MemorySample> {
        use windows_sys::Win32::System::{ProcessStatus::*, SystemInformation::*};
        unsafe {
            let mut process: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            process.cb = std::mem::size_of_val(&process) as u32;
            let mut physical: MEMORYSTATUSEX = std::mem::zeroed();
            physical.dwLength = std::mem::size_of_val(&physical) as u32;
            let mut system: PERFORMANCE_INFORMATION = std::mem::zeroed();
            system.cb = std::mem::size_of_val(&system) as u32;
            if K32GetProcessMemoryInfo(
                self.0,
                (&mut process as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                process.cb,
            ) == 0
                || GlobalMemoryStatusEx(&mut physical) == 0
                || K32GetPerformanceInfo(&mut system, system.cb) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(MemorySample {
                at: chrono::Utc::now().to_rfc3339(),
                private_bytes: process.PrivateUsage as u64,
                working_set_bytes: process.WorkingSetSize as u64,
                available_bytes: physical.ullAvailPhys,
                commit_bytes: (system.CommitTotal as u64).saturating_mul(system.PageSize as u64),
                commit_limit_bytes: (system.CommitLimit as u64)
                    .saturating_mul(system.PageSize as u64),
            })
        }
    }
}
#[cfg(windows)]
impl Drop for MemoryProbe {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
pub(super) struct MemoryProbe;
#[cfg(not(windows))]
impl MemoryProbe {
    pub fn new(_: u32) -> std::io::Result<Self> {
        Err(std::io::Error::other("Memory sampling requires Windows"))
    }
    pub fn sample(&self) -> std::io::Result<MemorySample> {
        Err(std::io::Error::other("Memory sampling unavailable"))
    }
}
