use super::{AppPaths, AppResult};
use serde::Serialize;
use std::sync::Arc;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{SystemNotifications, is_foreground, system_english};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformNotificationStatus {
    pub available: bool,
    pub reason: String,
    pub identity: String,
    pub test_environment: bool,
}

#[derive(Clone)]
pub struct ToastMessage {
    pub key: String,
    pub title: String,
    pub body: String,
    pub test: bool,
}

pub trait NotificationBackend: Send + Sync {
    fn status(&self) -> PlatformNotificationStatus;
    /// False means the app came to the foreground before delivery.
    fn show(&self, message: ToastMessage) -> AppResult<bool>;
    fn remove(&self, key: &str) -> AppResult<()>;
    fn clear(&self) -> AppResult<()>;
    fn shutdown(&self);
}

pub type ActivationHandler = Arc<dyn Fn(String) + Send + Sync>;

pub fn start(paths: &AppPaths, handler: ActivationHandler) -> Arc<dyn NotificationBackend> {
    Arc::new(SystemNotifications::new(paths, handler))
}

pub fn valid_key(key: &str) -> bool {
    key == "test" || (key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn xml_escape(value: &str, limit: usize) -> String {
    let mut result = String::new();
    for c in value
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .take(limit)
    {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            '\u{fffe}' | '\u{ffff}' => {}
            other => result.push(other),
        }
    }
    result
}

pub(crate) fn toast_xml(message: &ToastMessage) -> String {
    format!(
        "<toast launch=\"{}\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        xml_escape(&message.key, 64),
        xml_escape(&message.title, 120),
        xml_escape(&message.body, 300),
    )
}

#[cfg(not(windows))]
pub struct SystemNotifications;
#[cfg(not(windows))]
impl SystemNotifications {
    pub fn new(_: &AppPaths, _: ActivationHandler) -> Self {
        Self
    }
}
#[cfg(not(windows))]
impl NotificationBackend for SystemNotifications {
    fn status(&self) -> PlatformNotificationStatus {
        PlatformNotificationStatus {
            available: false,
            reason: "unsupported".into(),
            identity: String::new(),
            test_environment: false,
        }
    }
    fn show(&self, _: ToastMessage) -> AppResult<bool> {
        Ok(false)
    }
    fn remove(&self, _: &str) -> AppResult<()> {
        Ok(())
    }
    fn clear(&self) -> AppResult<()> {
        Ok(())
    }
    fn shutdown(&self) {}
}
#[cfg(not(windows))]
pub fn is_foreground() -> bool {
    true
}
#[cfg(not(windows))]
pub fn system_english() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_and_bounds_only_notification_copy() {
        let xml = toast_xml(&ToastMessage {
            key: "test".into(),
            title: "<翻卡片> & \"测试\"\u{0}".into(),
            body: "需确认".repeat(300),
            test: true,
        });
        assert!(xml.contains("&lt;翻卡片&gt; &amp; &quot;测试&quot;"));
        assert!(!xml.contains('\0'));
        assert!(xml.len() < 1400);
        assert!(valid_key("test"));
        assert!(valid_key(&"a".repeat(64)));
        assert!(!valid_key("../anything"));
    }
}
