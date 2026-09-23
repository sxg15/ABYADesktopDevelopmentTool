pub(crate) mod transcript;
pub(crate) mod workflow;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalProvider {
    #[default]
    Codex,
    Grok,
}
