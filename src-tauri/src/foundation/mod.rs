mod database;
mod error;
mod paths;
mod settings;

pub use database::{Database, DatabaseRepairReport};
pub use error::{AppError, AppResult};
pub use paths::AppPaths;
pub use settings::{
    AppSettings, GameClientSelection, GameClientSource, PublicSettingsUpdate, SettingsService,
    selected_game_executable,
};

pub mod build_info;
pub mod change_signal;
pub mod checkout_update;
pub mod cli_environment;
pub mod cli_sessions;
pub mod cli_transport;
pub mod clipboard;
pub mod desktop_release;
pub mod notifications;
pub mod secrets;
pub mod single_instance;

/// Coordinate managed executable reads with removal of managed client packages.
/// Take a read guard for launch/version capture, a write guard for client maintenance.
pub static EXECUTABLE_FILES: parking_lot::RwLock<()> = parking_lot::RwLock::new(());
/// 设置保存与进程启动互斥，防止校验后立即换包。
pub static PLAYER_SELECTION: parking_lot::RwLock<()> = parking_lot::RwLock::new(());
