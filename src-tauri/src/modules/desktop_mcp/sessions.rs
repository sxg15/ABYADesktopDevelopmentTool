use parking_lot::Mutex;
use serde_json::Value;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::time::Duration;
use std::{collections::HashMap, sync::Arc, time::Instant};

struct Lease {
    context: Value,
    expires: Instant,
}
struct Session {
    owner: String,
    initialized: bool,
}
#[derive(Default)]
struct State {
    leases: HashMap<String, Lease>,
    sessions: HashMap<String, Session>,
}
#[derive(Clone, Default)]
pub struct McpLeases(Arc<Mutex<State>>);
fn key(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
impl McpLeases {
    /// Provisioned by the APP, never through an agent-callable MCP tool.
    #[cfg(test)]
    pub fn issue(&self, context: Value) -> Option<String> {
        for field in ["taskId", "conversationId"] {
            uuid::Uuid::parse_str(context[field].as_str()?).ok()?;
        }
        if context["provider"] != "codex" {
            return None;
        }
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        self.0.lock().leases.insert(
            key(&token),
            Lease {
                context,
                expires: Instant::now() + Duration::from_secs(12 * 3600),
            },
        );
        Some(token)
    }
    #[cfg(test)]
    pub fn revoke(&self, conversation: &str) {
        let mut s = self.0.lock();
        s.leases
            .retain(|_, v| v.context["conversationId"] != conversation);
        let valid: std::collections::HashSet<_> = s.leases.keys().cloned().collect();
        s.sessions.retain(|_, v| valid.contains(&v.owner));
    }
    pub(super) fn context(&self, token: &str) -> Option<Value> {
        self.0
            .lock()
            .leases
            .get(&key(token))
            .filter(|l| l.expires > Instant::now())
            .map(|l| l.context.clone())
    }
    #[cfg(test)]
    pub(super) fn begin(&self, token: &str) -> Option<String> {
        self.context(token)?;
        self.begin_authenticated(token)
    }
    pub(super) fn begin_authenticated(&self, token: &str) -> Option<String> {
        let mut s = self.0.lock();
        if s.sessions.len() >= 4096 {
            return None;
        }
        let id = uuid::Uuid::new_v4().to_string();
        s.sessions.insert(
            id.clone(),
            Session {
                owner: key(token),
                initialized: false,
            },
        );
        Some(id)
    }
    #[cfg(test)]
    pub(super) fn ready(&self, token: &str, session: &str, initialize: bool) -> bool {
        if self.context(token).is_none() {
            return false;
        }
        self.ready_authenticated(token, session, initialize)
    }
    pub(super) fn ready_authenticated(&self, token: &str, session: &str, initialize: bool) -> bool {
        let mut s = self.0.lock();
        let Some(v) = s
            .sessions
            .get_mut(session)
            .filter(|v| v.owner == key(token))
        else {
            return false;
        };
        if initialize {
            v.initialized = true;
        }
        v.initialized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn leases_bind_identity_initialization_and_revocation() {
        let leases = McpLeases::default();
        let chat = uuid::Uuid::new_v4().to_string();
        let a = leases
            .issue(json!({"provider":"codex","taskId":uuid::Uuid::new_v4(),"conversationId":chat}))
            .unwrap();
        let b=leases.issue(json!({"provider":"codex","taskId":uuid::Uuid::new_v4(),"conversationId":uuid::Uuid::new_v4()})).unwrap();
        let session = leases.begin(&a).unwrap();
        assert!(!leases.ready(&a, &session, false));
        assert!(!leases.ready(&b, &session, true));
        assert!(leases.ready(&a, &session, true));
        leases.revoke(&chat);
        assert!(!leases.ready(&a, &session, false));
        assert!(leases.context(&b).is_some());
    }
}
