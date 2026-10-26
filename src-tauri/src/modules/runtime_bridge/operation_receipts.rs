use super::*;
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Write, path::Path};
fn valid_id(id: &str) -> AppResult<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(AppError::validation("无效运行操作 ID。"));
    }
    Ok(())
}
fn read(path: &Path) -> AppResult<Value> {
    if !path.exists() {
        return Ok(Value::Null);
    }
    if path.metadata()?.len() > 20 * 1024 * 1024 {
        return Err(AppError::validation("运行收据超过容量。"));
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
fn once(path: &Path, hash: &str, run: impl FnOnce() -> AppResult<Value>) -> AppResult<Value> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let pending =
        json!({"inputHash":hash,"status":"pending","startedAt":chrono::Utc::now().to_rfc3339()});
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(&serde_json::to_vec(&pending)?)?;
            file.sync_all()?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let old = read(path).map_err(|_| {
                AppError::new(
                    "outcome_unknown",
                    "原操作收据尚未完整写入，请查询原操作；未重发。",
                    "",
                )
            })?;
            if old["inputHash"] != hash {
                return Err(AppError::new(
                    "conflict",
                    "同一运行操作 ID 的输入已变化，未执行。",
                    "not_executed",
                ));
            }
            if old["status"] == "returned" {
                return Ok(
                    json!({"success":old["runtimeSuccess"],"content":[{"type":"text","text":json!({"operationReceipt":old,"alreadyReturned":true,"payloadNotRetained":true,"message":"原操作已有返回收据，未再次执行。原始响应未缓存，请按既有输出或实际状态核对。"}).to_string()}]}),
                );
            }
            return Err(AppError::new(
                if old["status"] == "notExecuted" {
                    "runtimeNotExecuted"
                } else {
                    "outcome_unknown"
                },
                "原操作已有收据，请核对实际状态；相同操作不会自动重放。",
                old["status"].as_str().unwrap_or("unknown"),
            ));
        }
        Err(e) => return Err(e.into()),
    }
    let result = run();
    let mut receipt = pending;
    match &result {
        Ok(value) => {
            receipt["status"] = json!("returned");
            receipt["runtimeSuccess"] = json!(value["success"].as_bool().unwrap_or(false));
            if value["verification"].is_object() {
                receipt["verification"] = value["verification"].clone();
            }
            receipt["resultHash"] = json!(format!(
                "{:x}",
                Sha256::digest(value.to_string().as_bytes())
            ));
        }
        Err(e) => {
            receipt["status"] = json!(if e.detail == "not_executed" {
                "notExecuted"
            } else {
                "outcomeUnknown"
            });
            receipt["errorCode"] = json!(e.code);
        }
    }
    receipt["completedAt"] = json!(chrono::Utc::now().to_rfc3339());
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temp, serde_json::to_vec(&receipt)?)
        .and_then(|_| std::fs::rename(temp, path))
        .map_err(|_| {
            AppError::new(
                "outcome_unknown",
                "运行结果返回但收据保存失败，请读取实际状态；未重放。",
                "",
            )
        })?;
    result
}
impl RuntimeBridgeService {
    fn receipt_path(&self, id: &str, operation: &str) -> AppResult<PathBuf> {
        valid_id(operation)?;
        valid_id(id)?;
        let instance = self.instances.read(id)?;
        let task = self.tasks.get_readonly(
            instance
                .task_id
                .as_deref()
                .ok_or_else(|| AppError::validation("实例没有任务归属。"))?,
        )?;
        Ok(PathBuf::from(task.workspace_path)
            .join("artifacts/runtime")
            .join(id)
            .join("operation-receipts")
            .join(format!("{operation}.json")))
    }
    pub fn operation_receipt(&self, id: &str, operation: &str) -> AppResult<Value> {
        let mut value = read(&self.receipt_path(id, operation)?)?;
        if let Some(v) = value.as_object_mut() {
            v.remove("result");
        }
        Ok(value)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run(
        &self,
        id: &str,
        args: &[&str],
        input: Option<Value>,
        session: &str,
        operation: &str,
        timeout: Duration,
        cancel: Arc<AtomicBool>,
    ) -> AppResult<Value> {
        if args.first() != Some(&"capability") || args.get(1) != Some(&"run") {
            return self.run_inner(id, args, input, session, operation, timeout, cancel);
        }
        let path = self.receipt_path(id, operation)?;
        let hash = format!(
            "{:x}",
            Sha256::digest(json!([id, session, args, input]).to_string())
        );
        once(&path, &hash, || {
            self.run_inner(id, args, input, session, operation, timeout, cancel)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_loss_repeats_result_not_side_effect_and_unknown_never_replays() {
        let root =
            std::env::temp_dir().join(format!("abya-runtime-receipts-{}", uuid::Uuid::new_v4()));
        let path = root.join("one.json");
        let count = std::sync::atomic::AtomicUsize::new(0);
        let action = || {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(json!({"success":true,"requestId":"one"}))
        };
        assert_eq!(once(&path, "hash", action).unwrap()["success"], true);
        assert_eq!(once(&path, "hash", action).unwrap()["success"], true);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(once(&path, "other", action).is_err());
        let unknown = root.join("unknown.json");
        assert!(
            once(&unknown, "hash", || Err(AppError::new(
                "timeout",
                "unknown",
                "outcome_unknown"
            )))
            .is_err()
        );
        assert!(once(&unknown, "hash", action).is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(valid_id("../escape").is_err());
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
    }
    #[test]
    fn concurrent_same_operation_is_not_executed_twice() {
        let root = std::env::temp_dir().join(format!(
            "abya-runtime-receipt-race-{}",
            uuid::Uuid::new_v4()
        ));
        let path = root.join("one.json");
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let worker_path = path.clone();
            let worker = scope.spawn(move || {
                once(&worker_path, "hash", || {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(json!({"success":true}))
                })
            });
            started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(once(&path, "hash", || panic!("must not execute")).is_err());
            release_tx.send(()).unwrap();
            worker.join().unwrap().unwrap();
        });
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
    }
}
#[test]
fn runtime_receipt_never_persists_raw_output_or_credentials() {
    let root =
        std::env::temp_dir().join(format!("abya-receipt-redaction-{}", uuid::Uuid::new_v4()));
    let path = root.join("one.json");
    once(&path, "safehash", || {
        Ok(json!({"success":true,"content":[{"type":"text","text":"Bearer secret-canary"}]}))
    })
    .unwrap();
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("secret-canary")
    );
    let replay = once(&path, "safehash", || panic!("no replay")).unwrap();
    assert!(
        replay["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("payloadNotRetained")
    );
    let resolved = root.canonicalize().unwrap();
    assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    std::fs::remove_dir_all(resolved).unwrap();
}
