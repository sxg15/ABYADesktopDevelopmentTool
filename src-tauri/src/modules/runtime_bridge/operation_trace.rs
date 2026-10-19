use crate::foundation::{AppError, AppResult};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;

struct DigestWriter {
    hash: Sha256,
    bytes: usize,
}
impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > (16 * 1024 * 1024usize).saturating_sub(self.bytes) {
            return Err(std::io::Error::other("input budget exceeded"));
        }
        self.hash.update(bytes);
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn operation_metadata(input: &Value) -> AppResult<Value> {
    let mut digest = DigestWriter {
        hash: Sha256::new(),
        bytes: 0,
    };
    serde_json::to_writer(&mut digest, input)
        .map_err(|_| AppError::validation("工具输入超过 16 MiB 预算，尚未发送。"))?;
    let mut summary =
        json!({"sha256":format!("{:x}",digest.hash.finalize()),"jsonBytes":digest.bytes});
    for key in ["dryRun", "save", "offset", "limit"] {
        if input[key].is_boolean() || input[key].is_number() {
            summary[key] = input[key].clone();
        }
    }
    if let Some(operations) = input["operations"].as_array() {
        summary["operationCount"] = json!(operations.len());
    }
    for key in ["nodeId", "pageId", "documentId", "expectedRevision"] {
        if let Some(value) = input[key].as_str() {
            summary[key] = json!(crate::modules::instances::diagnostic_text(value));
        }
    }
    Ok(summary)
}
