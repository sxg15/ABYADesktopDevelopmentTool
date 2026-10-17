//! 在独立进程中重定向 HKCU，验证系统代理而不改变用户代理设置。
use super::{github::GitHub, oauth::OAuth};
use crate::foundation::cli_transport::Connection;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RegOverridePredefKey};
use winreg::RegKey;

const CHILD: &str = "ABYA_PROXY_TEST_HKCU";
const TEST: &str = "modules::game_clients::proxy_tests::windows_proxy_routes_download_and_oauth";

struct RegistryOverride(RegKey);
impl Drop for RegistryOverride {
    fn drop(&mut self) {
        // 只恢复本测试进程的句柄映射；真实 Internet Settings 从未被写入。
        unsafe { RegOverridePredefKey(HKEY_CURRENT_USER, std::ptr::null_mut()) };
    }
}

fn server(replies: Vec<String>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in replies {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "Expected proxy/local connection");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            // Windows accepted sockets can inherit the listener's nonblocking mode.
            // Header reads use the bounded blocking timeout below, not arrival timing.
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&mut socket);
            let mut request = String::new();
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                request.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let length = request
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            reader.read_exact(&mut vec![0; length]).unwrap();
            socket.write_all(reply.as_bytes()).unwrap();
            requests.push(request);
        }
        requests
    });
    (address, handle)
}

#[test]
fn windows_proxy_routes_download_and_oauth() {
    if let Ok(path) = std::env::var(CHILD) {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(path, winreg::enums::KEY_ALL_ACCESS)
            .unwrap();
        let registry = RegistryOverride(key);
        assert_eq!(
            unsafe { RegOverridePredefKey(HKEY_CURRENT_USER, registry.0.raw_handle()) },
            0
        );
        verify_routes();
        return;
    }
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!("Software\\ABYA-Proxy-Tests\\{}", uuid::Uuid::new_v4());
    let (key, _) = root.create_subkey(&path).unwrap();
    drop(key);
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", TEST, "--nocapture"])
        .env(CHILD, &path);
    for name in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"] {
        child.env_remove(name).env_remove(name.to_ascii_lowercase());
    }
    let output = child.output();
    root.delete_subkey_all(&path).unwrap();
    let output = output.unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn verify_routes() {
    let (proxy, requests) = server(vec![
        "HTTP/1.1 302 Found\r\nLocation: https://release-assets.githubusercontent.com/test.zip\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
    ]);
    let (settings, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .unwrap();
    settings.set_value("ProxyEnable", &1_u32).unwrap();
    settings.set_value("ProxyServer", &proxy).unwrap();
    settings.set_value("ProxyOverride", &"").unwrap();
    let mut github = GitHub::new(Some("fixture-token".into())).unwrap();
    github.test_api = Some("http://127.0.0.1:1".into());
    assert!(github.download(10).is_err());
    assert!(OAuth::new().unwrap().start().is_err());
    let requests = requests.join().unwrap();
    assert!(requests[0].starts_with("GET http://127.0.0.1:1/repos/"));
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains("accept: application/octet-stream")
    );
    assert!(requests[1].starts_with("CONNECT release-assets.githubusercontent.com:443 "));
    assert!(requests[2].starts_with("CONNECT github.com:443 "));
    for request in &requests[1..] {
        assert!(!request.contains("fixture-token"));
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
    }

    // 系统代理已启用且没有排除本机地址，CLI 仍须直接访问本机服务。
    let (local, request) = server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}".into(),
    ]);
    let result = Connection::Http {
        endpoint: format!("http://{local}/api/v1/command"),
        token: "local-fixture".into(),
    }
    .request(&serde_json::json!({}), Duration::from_secs(3))
    .unwrap();
    assert_eq!(result["ok"], true);
    assert!(request.join().unwrap()[0].starts_with("POST /api/v1/command "));

    // 关闭系统代理后，新建下载客户端恢复直连并保留二进制响应。
    settings.set_value("ProxyEnable", &0_u32).unwrap();
    let (direct, request) = server(vec![
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 7\r\nConnection: close\r\n\r\nPK-test".into(),
    ]);
    let mut bytes = Vec::new();
    let mut github = GitHub::new(None).unwrap();
    github.test_api = Some(format!("http://{direct}"));
    github
        .download(10)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, b"PK-test");
    assert!(request.join().unwrap()[0].starts_with("GET /repos/"));
}
