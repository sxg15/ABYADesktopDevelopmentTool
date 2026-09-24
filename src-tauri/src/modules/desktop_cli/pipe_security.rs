use std::{ffi::c_void, io, os::windows::io::AsRawHandle, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree},
    Security::Authorization::*,
    Security::*,
    System::{Pipes::*, Threading::*},
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn sid_text(sid: PSID) -> io::Result<String> {
    unsafe {
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        LocalFree(text.cast());
        Ok(result)
    }
}
fn token_sid(token: *mut c_void) -> io::Result<String> {
    unsafe {
        let mut len = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut len);
        let mut buffer = vec![0usize; (len as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), len, &mut len) == 0 {
            return Err(io::Error::last_os_error());
        }
        sid_text((*(buffer.as_ptr() as *const TOKEN_USER)).User.Sid)
    }
}
fn account_sid(account: &str) -> Option<String> {
    unsafe {
        let name = wide(account);
        let mut sid_len = 0;
        let mut domain_len = 0;
        let mut kind = 0;
        LookupAccountNameW(
            ptr::null(),
            name.as_ptr(),
            ptr::null_mut(),
            &mut sid_len,
            ptr::null_mut(),
            &mut domain_len,
            &mut kind,
        );
        let mut sid = vec![0usize; (sid_len as usize).div_ceil(size_of::<usize>())];
        let mut domain = vec![0u16; domain_len as usize];
        if LookupAccountNameW(
            ptr::null(),
            name.as_ptr(),
            sid.as_mut_ptr().cast(),
            &mut sid_len,
            domain.as_mut_ptr(),
            &mut domain_len,
            &mut kind,
        ) == 0
        {
            return None;
        }
        sid_text(sid.as_mut_ptr().cast()).ok()
    }
}
pub(super) fn allowed_sids() -> io::Result<Vec<String>> {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid = token_sid(token);
        CloseHandle(token);
        let mut result = vec![sid?];
        let computer = std::env::var("COMPUTERNAME").unwrap_or_else(|_| ".".into());
        for name in ["CodexSandboxOffline", "CodexSandboxOnline"] {
            if let Some(sid) = account_sid(&format!("{computer}\\{name}")) {
                result.push(sid);
            }
        }
        Ok(result)
    }
}
pub(super) fn create(
    name: &str,
    sids: &[String],
    first: bool,
) -> io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    let sddl = format!(
        "D:P{}",
        sids.iter()
            .enumerate()
            .map(|(index, s)| if index == 0 {
                format!("(A;;GRGW;;;{s})")
            } else {
                format!("(A;;0x0012019b;;;{s})")
            })
            .collect::<String>()
    );
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(&sddl).as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        result
    }
}
pub(super) fn peer_allowed(
    pipe: &tokio::net::windows::named_pipe::NamedPipeServer,
    sids: &[String],
) -> bool {
    // Never await or return while impersonating. Read-only identity query only.
    unsafe {
        if ImpersonateNamedPipeClient(pipe.as_raw_handle()) == 0 {
            return false;
        }
        let mut token = ptr::null_mut();
        let result = if OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) != 0 {
            let sid = token_sid(token);
            CloseHandle(token);
            sid.is_ok_and(|sid| sids.contains(&sid))
        } else {
            false
        };
        if RevertToSelf() == 0 {
            std::process::abort();
        }
        result
    }
}
