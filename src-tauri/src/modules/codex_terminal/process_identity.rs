#[derive(Debug, PartialEq, Eq)]
pub(super) enum Liveness {
    Alive,
    Dead,
    Unknown,
}

#[cfg(windows)]
pub(super) fn inspect(pid: u32, expected: Option<u64>) -> (Liveness, Option<u64>) {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, FILETIME, GetLastError},
        System::Threading::{
            GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return (
                if GetLastError() == ERROR_INVALID_PARAMETER {
                    Liveness::Dead
                } else {
                    Liveness::Unknown
                },
                None,
            );
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exit = std::mem::zeroed();
        let mut kernel = std::mem::zeroed();
        let mut user = std::mem::zeroed();
        let mut code = 0;
        let ok = GetProcessTimes(handle, &mut created, &mut exit, &mut kernel, &mut user) != 0
            && GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        if !ok {
            return (Liveness::Unknown, None);
        }
        let stamp = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        (
            if expected.is_some_and(|old| old != stamp) || code != 259 {
                Liveness::Dead
            } else {
                Liveness::Alive
            },
            Some(stamp),
        )
    }
}
#[cfg(not(windows))]
pub(super) fn inspect(_pid: u32, _expected: Option<u64>) -> (Liveness, Option<u64>) {
    (Liveness::Unknown, None)
}
pub(super) fn stamp(pid: u32) -> Option<u64> {
    inspect(pid, None).1
}
pub(super) fn known(pid: u32, stamp: Option<u64>) -> Liveness {
    if stamp.is_none() {
        Liveness::Unknown
    } else {
        inspect(pid, stamp).0
    }
}
