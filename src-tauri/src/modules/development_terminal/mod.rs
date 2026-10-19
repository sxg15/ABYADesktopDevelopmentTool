mod attention;
pub mod timing;
pub use attention::ConversationAttention;
pub(crate) mod transcript;
pub(crate) mod workflow;

// 异常报告只保留可审阅的摘要；不复制原始命令、参数或长的不透明值。
pub(crate) fn diagnostic_summary(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if [
        "token",
        "secret",
        "password",
        "authorization",
        "api_key",
        "credential",
        "cookie",
        "api-key",
        "apikey",
        "access_key",
        "ghp_",
        "$",
        "=",
        "`",
        "|",
        ";",
        "://",
        "bearer",
        "sk-",
    ]
    .iter()
    .any(|key| lower.contains(key))
    {
        return "[已过滤敏感内容]".into();
    }
    text.split_whitespace()
        .take(48)
        .map(|part| {
            if part.len() > 96 {
                "[已省略长值]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(512)
        .collect()
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalProvider {
    #[default]
    Codex,
}
