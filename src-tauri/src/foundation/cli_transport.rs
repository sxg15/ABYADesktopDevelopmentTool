use super::{AppError, AppResult, cli_environment};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone)]
pub enum Connection {
    Http {
        endpoint: String,
        token: String,
    },
    Pipe {
        name: String,
        credential: String,
        server_pid: u32,
    },
}
impl Connection {
    pub fn discover() -> AppResult<Self> {
        match (
            std::env::var("ABYA_DESKTOP_PIPE"),
            std::env::var("ABYA_DESKTOP_SESSION_TOKEN"),
        ) {
            (Ok(name), Ok(credential)) => {
                let valid = name
                    .strip_prefix(r"\\.\pipe\abya-desktop-")
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok());
                if !valid || credential.len() != 64 {
                    return Err(AppError::validation(
                        "Invalid CLI session. Reopen the task terminal.",
                    ));
                }
                let server_pid = std::env::var("ABYA_DESKTOP_PID")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .filter(|pid| *pid != 0)
                    .ok_or_else(|| {
                        AppError::validation(
                            "Missing Desktop server identity. Reopen the task terminal.",
                        )
                    })?;
                Ok(Self::Pipe {
                    name,
                    credential,
                    server_pid,
                })
            }
            (Err(_), Err(_)) => {
                let (endpoint, token) = cli_environment::connection()?;
                Ok(Self::Http { endpoint, token })
            }
            _ => Err(AppError::validation(
                "Incomplete CLI session. Reopen the task terminal.",
            )),
        }
    }
    pub fn request(&self, request: &Value, timeout: Duration) -> AppResult<Value> {
        match self {
            Self::Http { endpoint, token } => reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(timeout)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(AppError::internal)?
                .post(endpoint)
                .bearer_auth(token)
                .json(request)
                .send()
                .and_then(|response| response.json())
                .map_err(|_| unknown()),
            Self::Pipe {
                name,
                credential,
                server_pid,
            } => {
                let data = serde_json::to_vec(&json!({"credential":credential,"request":request}))?;
                if data.len() > 4 * 1024 * 1024 {
                    return Err(AppError::validation("CLI request exceeds 4 MiB."));
                }
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(AppError::internal)?
                    .block_on(async {
                        tokio::time::timeout(timeout, pipe_request(name, *server_pid, &data))
                            .await
                            .map_err(|_| unknown())?
                    })
            }
        }
    }
}
fn unknown() -> AppError {
    AppError::new(
        "outcome_unknown",
        "CLI response unavailable. Verify actual state before retrying.",
        "",
    )
}
async fn pipe_request(name: &str, server_pid: u32, data: &[u8]) -> AppResult<Value> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let started = std::time::Instant::now();
    let mut pipe = loop {
        // Identification only: the server may inspect identity, never act as the client.
        match open_pipe(name) {
            Ok(pipe) => break pipe,
            Err(error)
                if error.raw_os_error() == Some(231)
                    && started.elapsed() < Duration::from_secs(3) =>
            {
                tokio::time::sleep(Duration::from_millis(25)).await
            }
            Err(_) => {
                return Err(AppError::new(
                    "desktop_connection_unavailable",
                    "Desktop CLI pipe is unavailable. Reopen the task terminal; check that Desktop is running.",
                    "",
                ));
            }
        }
    };
    use std::os::windows::io::AsRawHandle;
    let mut actual_pid = 0;
    if unsafe {
        windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId(
            pipe.as_raw_handle(),
            &mut actual_pid,
        )
    } == 0
        || actual_pid != server_pid
    {
        return Err(AppError::new(
            "desktop_identity_mismatch",
            "Desktop pipe identity changed. Reopen the task terminal.",
            "",
        ));
    }
    pipe.write_u32_le(data.len() as u32)
        .await
        .map_err(|_| unknown())?;
    pipe.write_all(data).await.map_err(|_| unknown())?;
    let len = pipe.read_u32_le().await.map_err(|_| unknown())? as usize;
    if len > 32 * 1024 * 1024 {
        return Err(unknown());
    }
    let mut response = vec![0; len];
    pipe.read_exact(&mut response)
        .await
        .map_err(|_| unknown())?;
    serde_json::from_slice(&response).map_err(|_| unknown())
}

fn open_pipe(name: &str) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Storage::FileSystem::{CreateFileW, OPEN_EXISTING},
    };
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // Explicit read/write rights omit FILE_CREATE_PIPE_INSTANCE. Identification-only SQOS.
    unsafe {
        let handle = CreateFileW(
            name.as_ptr(),
            0x0012019b,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            0x40110000,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        tokio::net::windows::named_pipe::NamedPipeClient::from_raw_handle(handle)
    }
}
