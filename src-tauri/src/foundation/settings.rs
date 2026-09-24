use super::{AppError, AppResult, Database};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Utc;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use windows_dpapi::{Scope, decrypt_data, encrypt_data};

const SETTINGS_KEY: &str = "application";
const TOKEN_ENTROPY: &[u8] = b"ABYA Desktop Development Tool CLI";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub game_executable_path: String,
    pub workspace_root_path: String,
    pub locale: String,
    pub desktop_cli_port: u16,
    #[serde(skip_serializing)]
    pub desktop_cli_token: String,
    pub game_gateway_port: u16,
    pub preferred_adapter_id: String,
    pub lan_broadcast_enabled: bool,
    pub tool_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSettings {
    #[serde(default)]
    game_executable_path: String,
    #[serde(default)]
    workspace_root_path: String,
    #[serde(default)]
    locale: String,
    #[serde(default = "default_desktop_cli_port")]
    desktop_cli_port: u16,
    #[serde(default)]
    encrypted_desktop_cli_token: String,
    #[serde(default = "default_game_gateway_port")]
    game_gateway_port: u16,
    #[serde(default)]
    preferred_adapter_id: String,
    #[serde(default = "default_true")]
    lan_broadcast_enabled: bool,
    #[serde(default)]
    tool_id: String,
}

#[derive(Clone)]
pub struct SettingsService {
    database: Database,
    default_workspace_root: PathBuf,
}

impl SettingsService {
    pub fn new(database: Database, default_workspace_root: PathBuf) -> Self {
        Self {
            database,
            default_workspace_root,
        }
    }

    pub fn get(&self) -> AppResult<AppSettings> {
        let stored = self.database.with_connection(|connection| {
            let mut statement =
                connection.prepare("SELECT value_json FROM settings WHERE key=?1")?;
            let value = statement.query_row([SETTINGS_KEY], |row| row.get::<_, String>(0));
            match value {
                Ok(json) => Ok(Some(serde_json::from_str::<StoredSettings>(&json)?)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(error) => Err(error.into()),
            }
        })?;
        if let Some(stored) = stored {
            let token = if stored.encrypted_desktop_cli_token.is_empty() {
                generate_token()
            } else {
                decrypt_token(&stored.encrypted_desktop_cli_token)?
            };
            let settings = AppSettings {
                game_executable_path: stored.game_executable_path,
                workspace_root_path: if stored.workspace_root_path.trim().is_empty()
                    || super::AppPaths::legacy_workspace_root().is_some_and(|p| p == std::path::PathBuf::from(&stored.workspace_root_path)) {
                    self.default_workspace_root.to_string_lossy().into_owned()
                } else {
                    stored.workspace_root_path
                },
                locale: stored.locale,
                desktop_cli_port: stored.desktop_cli_port,
                desktop_cli_token: token,
                game_gateway_port: stored.game_gateway_port,
                preferred_adapter_id: stored.preferred_adapter_id,
                lan_broadcast_enabled: stored.lan_broadcast_enabled,
                tool_id: if stored.tool_id.is_empty() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    stored.tool_id
                },
            };
            self.save(&settings)?;
            return Ok(settings);
        }
        let defaults = AppSettings {
            game_executable_path: String::new(),
            workspace_root_path: self.default_workspace_root.to_string_lossy().into_owned(),
            locale: String::new(),
            desktop_cli_port: default_desktop_cli_port(),
            desktop_cli_token: generate_token(),
            game_gateway_port: default_game_gateway_port(),
            preferred_adapter_id: String::new(),
            lan_broadcast_enabled: true,
            tool_id: uuid::Uuid::new_v4().to_string(),
        };
        self.save(&defaults)?;
        Ok(defaults)
    }

    pub fn save(&self, settings: &AppSettings) -> AppResult<()> {
        if settings.desktop_cli_port < 1024 {
            return Err(AppError::validation(
                "Desktop CLI port must be between 1024 and 65535.",
            ));
        }
        if settings.game_gateway_port < 1024 {
            return Err(AppError::validation(
                "Game gateway port must be between 1024 and 65535.",
            ));
        }
        if settings.desktop_cli_port == settings.game_gateway_port {
            return Err(AppError::validation(
                "Desktop CLI and the game gateway must use different ports.",
            ));
        }
        let workspace_root = if settings.workspace_root_path.trim().is_empty() {
            self.default_workspace_root.clone()
        } else {
            PathBuf::from(settings.workspace_root_path.trim())
        };
        std::fs::create_dir_all(&workspace_root)?;
        if !workspace_root.is_dir() {
            return Err(AppError::validation("Workspace root must be a directory."));
        }
        let stored = StoredSettings {
            game_executable_path: settings.game_executable_path.trim().to_string(),
            workspace_root_path: workspace_root.to_string_lossy().into_owned(),
            locale: settings.locale.trim().to_string(),
            desktop_cli_port: settings.desktop_cli_port,
            encrypted_desktop_cli_token: encrypt_token(&settings.desktop_cli_token)?,
            game_gateway_port: settings.game_gateway_port,
            preferred_adapter_id: settings.preferred_adapter_id.trim().to_string(),
            lan_broadcast_enabled: settings.lan_broadcast_enabled,
            tool_id: settings.tool_id.clone(),
        };
        let json = serde_json::to_string(&stored)?;
        self.database.with_connection(|connection| {
            connection.execute(
                "INSERT INTO settings(key,value_json,updated_at) VALUES(?1,?2,?3)
                 ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",
                rusqlite::params![SETTINGS_KEY, json, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn update_public(
        &self,
        game_executable_path: String,
        workspace_root_path: String,
        locale: String,
        game_gateway_port: u16,
        preferred_adapter_id: String,
        lan_broadcast_enabled: bool,
    ) -> AppResult<AppSettings> {
        let mut settings = self.get()?;
        settings.game_executable_path = game_executable_path;
        settings.workspace_root_path = workspace_root_path;
        settings.locale = locale;
        settings.game_gateway_port = game_gateway_port;
        settings.preferred_adapter_id = preferred_adapter_id;
        settings.lan_broadcast_enabled = lan_broadcast_enabled;
        settings.workspace_root_path = if settings.workspace_root_path.trim().is_empty() {
            self.default_workspace_root.to_string_lossy().into_owned()
        } else {
            settings.workspace_root_path.trim().to_string()
        };
        self.save(&settings)?;
        Ok(settings)
    }

    pub fn regenerate_token(&self) -> AppResult<AppSettings> {
        let mut settings = self.get()?;
        settings.desktop_cli_token = generate_token();
        self.save(&settings)?;
        Ok(settings)
    }
}

fn default_desktop_cli_port() -> u16 {
    47600
}

fn default_game_gateway_port() -> u16 {
    47610
}

fn default_true() -> bool {
    true
}

fn generate_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

pub fn encrypt_token(token: &str) -> AppResult<String> {
    let encrypted = encrypt_data(token.as_bytes(), Scope::User, Some(TOKEN_ENTROPY))
        .map_err(AppError::internal)?;
    Ok(STANDARD.encode(encrypted))
}

pub fn decrypt_token(encoded: &str) -> AppResult<String> {
    let encrypted = STANDARD.decode(encoded).map_err(AppError::internal)?;
    let decrypted =
        decrypt_data(&encrypted, Scope::User, Some(TOKEN_ENTROPY)).map_err(AppError::internal)?;
    String::from_utf8(decrypted).map_err(AppError::internal)
}
