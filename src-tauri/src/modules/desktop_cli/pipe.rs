use super::{
    pipe_security,
    protocol::{self, HttpState},
};
use crate::foundation::{AppError, AppResult, cli_sessions};
use axum::{Json, extract::State, http::HeaderMap};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::NamedPipeServer,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    credential: String,
    request: Value,
}
pub(super) fn start(state: HttpState, token: String) -> AppResult<tokio::task::JoinHandle<()>> {
    let name = format!(r"\\.\pipe\abya-desktop-{}", uuid::Uuid::new_v4().simple());
    let sids = pipe_security::allowed_sids()?;
    let guard = tauri::async_runtime::handle();
    let _entered = guard.inner().enter();
    let mut listener = pipe_security::create(&name, &sids, true).map_err(AppError::internal)?;
    cli_sessions::activate(name.clone());
    Ok(tokio::spawn(async move {
        let mut requests = tokio::task::JoinSet::new();
        loop {
            while requests.len() >= 32 {
                let _ = requests.join_next().await;
            }
            if listener.connect().await.is_err() {
                break;
            }
            let next = match pipe_security::create(&name, &sids, false) {
                Ok(v) => v,
                Err(_) => break,
            };
            let connected = std::mem::replace(&mut listener, next);
            let state = state.clone();
            let token = token.clone();
            let sids = sids.clone();
            requests.spawn(async move {
                let _ = serve(connected, state, token, sids).await;
            });
            while requests.try_join_next().is_some() {}
        }
    }))
}
async fn serve(
    mut pipe: NamedPipeServer,
    state: HttpState,
    token: String,
    sids: Vec<String>,
) -> std::io::Result<()> {
    let envelope = tokio::time::timeout(Duration::from_secs(5), async {
        let length = pipe.read_u32_le().await? as usize;
        if length > 4 * 1024 * 1024 {
            return Err(std::io::Error::other("request_too_large"));
        }
        let mut data = vec![0; length];
        pipe.read_exact(&mut data).await?;
        serde_json::from_slice::<Envelope>(&data).map_err(std::io::Error::other)
    })
    .await??;
    let request_id = envelope.request["requestId"].clone();
    let response = if !pipe_security::peer_allowed(&pipe, &sids)
        || !cli_sessions::authorize(&envelope.credential, &envelope.request["context"])
    {
        json!({"schemaVersion":1,"requestId":request_id,"success":false,"error":{"code":"session_unauthorized","message":"Reopen the task terminal to renew its CLI session."}})
    } else if matches!(
        envelope.request["command"].as_str(),
        Some("development_task_create" | "development_task_list")
    ) {
        json!({"schemaVersion":1,"requestId":request_id,"success":false,"error":{"code":"session_scope_denied"}})
    } else if let Ok(input) = serde_json::from_value(envelope.request) {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
        match tokio::time::timeout(
            Duration::from_secs(720),
            protocol::handle_command(State(state), headers, Json(input)),
        )
        .await
        {
            Ok(response) => {
                let bytes = axum::body::to_bytes(response.into_body(), 32 * 1024 * 1024)
                    .await
                    .map_err(std::io::Error::other)?;
                serde_json::from_slice(&bytes).map_err(std::io::Error::other)?
            }
            Err(_) => {
                json!({"schemaVersion":1,"requestId":request_id,"success":false,"error":{"code":"outcome_unknown"}})
            }
        }
    } else {
        json!({"schemaVersion":1,"requestId":request_id,"success":false,"error":{"code":"invalid_request"}})
    };
    let data = serde_json::to_vec(&response)?;
    tokio::time::timeout(Duration::from_secs(15), async {
        pipe.write_u32_le(data.len() as u32).await?;
        pipe.write_all(&data).await
    })
    .await??;
    Ok(())
}
