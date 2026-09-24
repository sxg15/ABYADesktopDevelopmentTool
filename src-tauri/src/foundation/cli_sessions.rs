//! In-memory, revocable capabilities for managed terminal sessions.
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::OnceLock, time::{Duration, Instant}};

#[derive(Default)]
struct Registry { pipe: String, leases: HashMap<String, Lease> }
struct Lease { token: String, context: Value, expires: Instant }
static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
fn registry() -> &'static Mutex<Registry> { REGISTRY.get_or_init(Default::default) }

pub fn activate(pipe: String) {
    *registry().lock() = Registry { pipe, leases: HashMap::new() };
}
pub fn deactivate() { *registry().lock() = Registry::default(); }
pub fn revoke(provider: &str, conversation: &str) {
    registry().lock().leases.remove(&format!("{provider}:{conversation}"));
}
pub fn task_active(task: &str) -> bool {
    registry().lock().leases.values().any(|lease| lease.context["taskId"] == task && lease.expires > Instant::now())
}
pub fn environment(provider: &str, task: &str, conversation: &str) -> Vec<(String, String)> {
    let mut state = registry().lock();
    if state.pipe.is_empty() { return vec![]; }
    let pipe = state.pipe.clone();
    let context = json!({"provider":provider,"taskId":task,"conversationId":conversation});
    let key = format!("{provider}:{conversation}");
    let lease = state.leases.entry(key).or_insert_with(|| Lease {
        token: format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple()),
        context: context.clone(), expires: Instant::now() + Duration::from_secs(12*3600),
    });
    if lease.context != context || lease.expires <= Instant::now() {
        lease.token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        lease.context = context;
        lease.expires = Instant::now() + Duration::from_secs(12*3600);
    }
    vec![("ABYA_DESKTOP_PIPE".into(), pipe), ("ABYA_DESKTOP_SESSION_TOKEN".into(), lease.token.clone())]
}
pub fn authorize(token: &str, context: &Value) -> bool {
    authorize_in(&registry().lock(), token, context)
}
fn authorize_in(state: &Registry, token: &str, context: &Value) -> bool {
    let provider = context["provider"].as_str().unwrap_or("");
    let conversation = context["conversationId"].as_str().unwrap_or("");
    let Some(lease) = state.leases.get(&format!("{provider}:{conversation}")) else { return false; };
    let mut diff = token.len() ^ lease.token.len();
    for (i, b) in lease.token.bytes().enumerate() {
        diff |= usize::from(b ^ token.as_bytes().get(i).copied().unwrap_or(0));
    }
    diff == 0 && lease.context == *context && lease.expires > Instant::now()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_wrong_credentials_context_expiry_and_revocation() {
        let context = json!({"provider":"codex","taskId":"one","conversationId":"chat"});
        let mut state = Registry::default();
        state.leases.insert("codex:chat".into(), Lease { token:"secret".into(), context:context.clone(), expires:Instant::now()+Duration::from_secs(60) });
        assert!(authorize_in(&state,"secret",&context));
        assert!(!authorize_in(&state,"wrong",&context));
        for key in ["taskId","conversationId","provider"] {
            let mut other=context.clone(); other[key]=json!("other");
            assert!(!authorize_in(&state,"secret",&other));
        }
        state.leases.get_mut("codex:chat").unwrap().expires=Instant::now()-Duration::from_secs(1);
        assert!(!authorize_in(&state,"secret",&context));
        state.leases.clear(); assert!(!authorize_in(&state,"secret",&context));
    }
}
