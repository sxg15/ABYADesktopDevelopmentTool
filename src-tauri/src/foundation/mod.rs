mod database;
mod error;
mod paths;
mod settings;

pub use database::{Database, DatabaseRepairReport};
pub use error::{AppError, AppResult};
pub use paths::AppPaths;
pub use settings::{
    AppSettings, GameClientSelection, GameClientSource, PublicSettingsUpdate, SettingsService,
};

pub mod build_info;
pub mod change_signal;
pub mod clipboard;
pub mod mcp_http;
pub mod mcp_sessions;
pub mod notifications;
pub mod secrets;
pub mod single_instance;

/// Coordinate managed executable reads with removal of managed client packages.
/// Take a read guard for launch/version capture, a write guard for client maintenance.
pub static EXECUTABLE_FILES: parking_lot::RwLock<()> = parking_lot::RwLock::new(());
