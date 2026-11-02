use crate::foundation::{AppError, AppResult};
use base64::Engine;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
#[derive(Clone)]
struct Connection {
    descriptor: PathBuf,
    endpoint: String,
    token: String,
    session: String,
}
impl Connection {
    fn current(&self, pid: u32) -> bool {
        if std::fs::metadata(&self.descriptor).is_err() {
            return false;
        }
        let Ok(raw) = std::fs::read(&self.descriptor) else {
            return false;
        };
        if raw.len() > 65536 {
            return false;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&raw) else {
            return false;
        };
        value["processId"].as_u64() == Some(pid as u64)
            && value["endpoint"] == self.endpoint
            && value["token"] == self.token
    }
}
static CONNECTIONS: OnceLock<Mutex<HashMap<String, Connection>>> = OnceLock::new();
fn unknown() -> AppError {
    AppError::new(
        "outcome_unknown",
        "MCP 请求结果未知，请先核对原操作和实际状态。",
        "",
    )
}
fn local_root() -> AppResult<PathBuf> {
    #[cfg(windows)]
    {
        use windows::Win32::{
            System::Com::CoTaskMemFree,
            UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
        };
        unsafe {
            let p = SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None)
                .map_err(AppError::internal)?;
            let text = p.to_string();
            CoTaskMemFree(Some(p.0.cast()));
            Ok(PathBuf::from(text.map_err(AppError::internal)?))
        }
    }
    #[cfg(not(windows))]
    {
        Err(AppError::validation("当前实例发现仅支持 Windows。"))
    }
}
fn candidates(pid: u32) -> AppResult<Vec<Connection>> {
    let dir = local_root()?.join("AbyaPB/Mcp/instances");
    let mut found = vec![];
    if !dir.is_dir() {
        return Err(AppError::new(
            "runtime_mcp_unavailable",
            "未发现 PB 原生 MCP 服务；不会回退 CLI。",
            "not_executed",
        ));
    }
    for entry in std::fs::read_dir(dir)? {
        let file = entry?.path();
        if file.extension().and_then(|s| s.to_str()) != Some("json")
            || file.metadata()?.len() > 65536
        {
            continue;
        }
        let Ok(raw) = std::fs::read(&file) else {
            continue;
        };
        let Ok(v) = serde_json::from_slice::<Value>(&raw) else {
            continue;
        };
        if v["processId"].as_u64() != Some(pid as u64) {
            continue;
        }
        let Some(endpoint) = v["endpoint"].as_str() else {
            continue;
        };
        let Some(token) = v["token"].as_str() else {
            continue;
        };
        let Ok(url) = reqwest::Url::parse(endpoint) else {
            continue;
        };
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.path() != "/mcp"
            || url.port().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || token.len() < 32
        {
            continue;
        }
        if !found
            .iter()
            .any(|c: &Connection| c.endpoint == endpoint && c.token == token)
        {
            found.push(Connection {
                endpoint: endpoint.into(),
                token: token.into(),
                session: String::new(),
                descriptor: file.clone(),
            });
        }
    }
    Ok(found)
}
async fn post(
    client: &reqwest::Client,
    c: &Connection,
    body: Value,
    timeout: Duration,
) -> AppResult<(reqwest::header::HeaderMap, Value)> {
    let mut req = client
        .post(&c.endpoint)
        .bearer_auth(&c.token)
        .header("Accept", "application/json, text/event-stream")
        .timeout(timeout)
        .json(&body);
    if !c.session.is_empty() {
        req = req
            .header("Mcp-Session-Id", &c.session)
            .header("MCP-Protocol-Version", "2025-11-25");
    }
    let response = req.send().await.map_err(|e| {
        if e.is_connect() {
            AppError::new(
                "runtime_mcp_not_connected",
                "MCP 连接尚未建立，业务操作未发送。",
                "not_executed",
            )
        } else {
            unknown()
        }
    })?;
    if matches!(response.status().as_u16(), 400 | 401 | 403 | 404) {
        return Err(AppError::new(
            "runtime_mcp_rejected",
            "MCP 在派发前拒绝请求；核对会话与参数后重试。",
            "not_executed",
        ));
    }
    if !response.status().is_success() {
        return Err(unknown());
    }
    let headers = response.headers().clone();
    if response.status() == reqwest::StatusCode::ACCEPTED {
        return Ok((headers, Value::Null));
    }
    let mut response = response;
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unknown())? {
        if data.len() + chunk.len() > 32 * 1024 * 1024 {
            return Err(unknown());
        }
        data.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&data).map_err(|_| unknown())?;
    if value["jsonrpc"] != "2.0" || value["id"] != body["id"] || value.get("error").is_some() {
        return Err(unknown());
    }
    Ok((headers, value["result"].clone()))
}
fn data(value: &Value) -> Value {
    value["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .find_map(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Null)
}
async fn connect(
    client: &reqwest::Client,
    mut c: Connection,
    pid: u32,
    id: &str,
    owner: &str,
) -> AppResult<Connection> {
    let (headers,result)=post(client,&c,json!({"jsonrpc":"2.0","id":uuid::Uuid::new_v4().to_string(),"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":owner,"version":"1"}}}),Duration::from_secs(5)).await?;
    if result["protocolVersion"] != "2025-11-25" {
        return Err(AppError::validation("PB MCP 协议版本不兼容。"));
    }
    c.session = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(unknown)?
        .into();
    post(
        client,
        &c,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        Duration::from_secs(5),
    )
    .await?;
    let (_,identity)=post(client,&c,json!({"jsonrpc":"2.0","id":uuid::Uuid::new_v4().to_string(),"method":"tools/call","params":{"name":"abya_host_identity","arguments":{}}}),Duration::from_secs(5)).await?;
    let identity = data(&identity);
    if identity["processId"].as_u64() != Some(pid as u64) || identity["desktopInstanceId"] != id {
        return Err(AppError::new(
            "runtime_identity_mismatch",
            "PB MCP 实例身份不匹配。",
            "not_executed",
        ));
    }
    Ok(c)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn execute(
    pid: u32,
    id: &str,
    args: &[&str],
    input: Option<Value>,
    owner: &str,
    operation: &str,
    output: &Path,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
    memory_pressure: &dyn Fn() -> bool,
) -> AppResult<Value> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(AppError::internal)?;
    runtime.block_on(async {
        let started=std::time::Instant::now();
        let client=crate::foundation::mcp_http::async_client()?;
        let key=format!("{pid}:{id}:{owner}");let cache=CONNECTIONS.get_or_init(Default::default);
        let cached=cache.lock().get(&key).cloned().filter(|c|c.current(pid));
        let connection=if let Some(c)=cached{c}else{
            let options=candidates(pid)?;
            let verified=tokio::time::timeout(timeout.min(Duration::from_secs(15)),async {
                let mut verified=vec![];
                for c in options{if let Ok(c)=connect(&client,c,pid,id,owner).await{verified.push(c);}}
                verified
            }).await.map_err(|_|AppError::new("runtime_mcp_not_ready","MCP 身份核对超时，业务操作尚未发送。","not_executed"))?;
            let mut verified=verified;
            if verified.len()!=1{return Err(AppError::new("runtime_mcp_unavailable","没有唯一且身份匹配的 PB MCP 实例。","not_executed"));}
            let c=verified.remove(0);cache.lock().insert(key.clone(),c.clone());c
        };
        if cancelled.load(Ordering::SeqCst)||memory_pressure(){return Err(AppError::new("cancelled","操作尚未发送。","not_executed"));}
        let method=if args==["capability","list"]{"tools/list"}else{"tools/call"};
        let name=if args==["status"]{"abya_host_identity"}else{args.get(2).copied().unwrap_or("")};
        let body=json!({"jsonrpc":"2.0","id":operation,"method":method,"params":{"name":name,"arguments":input.unwrap_or_else(||json!({})),"_meta":{"abya/operationId":operation}}});
        let remaining=timeout.saturating_sub(started.elapsed());
        if remaining.is_zero(){return Err(AppError::new("runtime_mcp_not_ready","连接准备已耗尽时限，业务操作尚未发送。","not_executed"));}
        let request=post(&client,&connection,body,remaining);tokio::pin!(request);
        let mut poll=tokio::time::interval(Duration::from_millis(100));
        let result=loop{tokio::select!{
            result=&mut request=>break result,
            _=poll.tick()=>{if cancelled.load(Ordering::SeqCst)||memory_pressure(){
                let _=post(&client,&connection,json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":operation}}),Duration::from_secs(3)).await;
                break Err(unknown());
            }}
        }};
        let (_,mut result)=match result{Ok(v)=>v,Err(e)=>{cache.lock().remove(&key);return Err(e);}};
        if args==["capability","list"]{if !result["tools"].is_array(){return Err(unknown());}return Ok(json!({"success":true,"data":result}));}
        if args==["status"]{let mut value=data(&result);if result["isError"]==true || value["processId"].as_u64()!=Some(pid as u64) || value["desktopInstanceId"]!=id{return Err(unknown());}value["endpoint"]=json!(connection.endpoint);return Ok(value);}
        std::fs::create_dir_all(output)?;
        if result["isError"]==true {
            std::fs::write(output.join("error-result.json"),serde_json::to_vec(&result)?)?;
            if data(&result)["errorCode"]=="outcome_unknown"{return Err(unknown());}
        }
        let mut artifacts=vec![];
        for (i,block) in result["content"].as_array_mut().into_iter().flatten().enumerate(){
            if block["type"]!="image"{continue;}
            let extension=match block["mimeType"].as_str(){Some("image/png")=>"png",Some("image/jpeg")=>"jpg",_=>continue};
            let bytes=base64::engine::general_purpose::STANDARD.decode(block["data"].as_str().unwrap_or("")).map_err(AppError::internal)?;
            if bytes.len()>16*1024*1024{return Err(unknown());}
            let path=output.join(format!("image-{i}.{extension}"));std::fs::write(&path,bytes)?;block["_meta"]=json!({"abya/artifactPath":path});artifacts.push(json!({"type":"text","text":json!({"artifactPath":path}).to_string()}));
        }
        if let Some(content)=result["content"].as_array_mut(){content.extend(artifacts);}
        Ok(json!({"success":result["isError"]!=true,"content":result["content"],"requestId":operation}))
    })
}

#[cfg(test)]
mod native_fault_tests {
    use super::*;
    use std::io::{Read, Write};

    // Exercise the real native client after dispatch, without a CLI peer or replay.
    fn failed_response(hang: bool) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let peer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut buffer = [0; 8192];
            assert!(socket.read(&mut buffer).unwrap() > 0);
            if hang {
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\nContent-Type: application/json\r\n\r\n{").unwrap();
                std::thread::sleep(Duration::from_millis(200));
            }
            // Closing after receiving a request cannot prove whether a write executed.
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime.block_on(async {
            post(&crate::foundation::mcp_http::async_client().unwrap(),
                &Connection { descriptor: PathBuf::new(), endpoint, token: "test-only".into(), session: "owned".into() },
                json!({"jsonrpc":"2.0","id":"original-operation","method":"tools/call","params":{"name":"write"}}),
                Duration::from_millis(80)).await.unwrap_err()
        });
        peer.join().unwrap();
        assert_eq!(error.code, "outcome_unknown");
    }

    #[test]
    fn mcp_transport_fault_disconnect_preserves_unknown_outcome() {
        failed_response(false);
    }
    #[test]
    fn mcp_transport_fault_timeout_preserves_unknown_outcome() {
        failed_response(true);
    }
}
