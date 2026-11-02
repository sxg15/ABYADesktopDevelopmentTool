use super::{AppError, AppResult, Database};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SETTINGS_KEY: &str = "application";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameClientSource {
    #[default]
    Local,
    Remote,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameClientSelection {
    pub source: GameClientSource,
    pub local_executable_path: String,
    pub installation_id: String,
    pub executable_relative_path: String,
    pub storage_root: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSettingsUpdate {
    pub game_executable_path: String,
    pub workspace_root_path: String,
    pub locale: String,
    pub game_gateway_port: u16,
    pub preferred_adapter_id: String,
    pub lan_broadcast_enabled: bool,
    #[serde(default)]
    pub game_client: GameClientSelection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub game_executable_path: String,
    #[serde(default)]
    pub game_client: GameClientSelection,
    pub workspace_root_path: String,
    pub locale: String,
    pub desktop_mcp_port: u16,
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
    game_client: GameClientSelection,
    #[serde(default)]
    workspace_root_path: String,
    #[serde(default)]
    locale: String,
    #[serde(default = "default_desktop_mcp_port")]
    desktop_mcp_port: u16,
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
    gate: std::sync::Arc<parking_lot::ReentrantMutex<()>>,
}

impl SettingsService {
    /// Reading display preferences must not decrypt/save credentials or allocate workspaces.
    pub fn locale(&self) -> AppResult<String> {
        use rusqlite::OptionalExtension;
        self.database.with_connection(|connection| {
            Ok(connection
                .query_row(
                    "SELECT COALESCE(json_extract(value_json,'$.locale'),'') FROM settings WHERE key=?1",
                    [SETTINGS_KEY],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or_default())
        })
    }

    pub fn new(database: Database, default_workspace_root: PathBuf) -> Self {
        Self {
            database,
            default_workspace_root,
            gate: Default::default(),
        }
    }

    pub fn get(&self) -> AppResult<AppSettings> {
        let _guard = self.gate.lock();
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
            let settings = AppSettings {
                game_client: {
                    let mut selection = stored.game_client;
                    if selection.source == GameClientSource::Local
                        && selection.local_executable_path.is_empty()
                    {
                        selection.local_executable_path = stored.game_executable_path.clone();
                    }
                    selection
                },
                game_executable_path: stored.game_executable_path,
                workspace_root_path: if stored.workspace_root_path.trim().is_empty()
                    || super::AppPaths::legacy_workspace_root()
                        .is_some_and(|p| p == std::path::Path::new(&stored.workspace_root_path))
                {
                    self.default_workspace_root.to_string_lossy().into_owned()
                } else {
                    stored.workspace_root_path
                },
                locale: stored.locale,
                desktop_mcp_port: stored.desktop_mcp_port,
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
            game_client: GameClientSelection::default(),
            workspace_root_path: self.default_workspace_root.to_string_lossy().into_owned(),
            locale: String::new(),
            desktop_mcp_port: default_desktop_mcp_port(),
            game_gateway_port: default_game_gateway_port(),
            preferred_adapter_id: String::new(),
            lan_broadcast_enabled: true,
            tool_id: uuid::Uuid::new_v4().to_string(),
        };
        self.save(&defaults)?;
        Ok(defaults)
    }

    pub fn save(&self, settings: &AppSettings) -> AppResult<()> {
        let _guard = self.gate.lock();
        if settings.desktop_mcp_port < 1024 {
            return Err(AppError::validation(
                "Desktop MCP port must be between 1024 and 65535.",
            ));
        }
        if settings.game_gateway_port < 1024 {
            return Err(AppError::validation(
                "Game gateway port must be between 1024 and 65535.",
            ));
        }
        if settings.desktop_mcp_port == settings.game_gateway_port {
            return Err(AppError::validation(
                "Desktop MCP and the game gateway must use different ports.",
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
            game_client: settings.game_client.clone(),
            workspace_root_path: workspace_root.to_string_lossy().into_owned(),
            locale: settings.locale.trim().to_string(),
            desktop_mcp_port: settings.desktop_mcp_port,
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
        })?;
        super::change_signal::notify();
        Ok(())
    }

    pub fn update_public(&self, input: PublicSettingsUpdate) -> AppResult<AppSettings> {
        let _guard = self.gate.lock();
        let mut settings = self.get()?;
        settings.game_executable_path = input.game_executable_path;
        settings.game_client = input.game_client;
        settings.workspace_root_path = input.workspace_root_path;
        settings.locale = input.locale;
        settings.game_gateway_port = input.game_gateway_port;
        settings.preferred_adapter_id = input.preferred_adapter_id;
        settings.lan_broadcast_enabled = input.lan_broadcast_enabled;
        settings.workspace_root_path = if settings.workspace_root_path.trim().is_empty() {
            self.default_workspace_root.to_string_lossy().into_owned()
        } else {
            settings.workspace_root_path.trim().to_string()
        };
        self.save(&settings)?;
        Ok(settings)
    }

    /// Caller validates the managed installation; preserve all other freshly read settings.
    pub fn update_game_client(
        &self,
        mut selection: GameClientSelection,
        executable_path: String,
    ) -> AppResult<AppSettings> {
        let _guard = self.gate.lock();
        let mut settings = self.get()?;
        selection.local_executable_path = settings.game_client.local_executable_path;
        settings.game_client = selection;
        settings.game_executable_path = executable_path;
        self.save(&settings)?;
        Ok(settings)
    }
}

fn default_desktop_mcp_port() -> u16 {
    47700
}

fn default_game_gateway_port() -> u16 {
    47710
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod mcp_settings_tests {
    use super::*;
    #[test]
    fn obsolete_corrupt_transport_secret_is_not_required_or_persisted() {
        let stored: StoredSettings = serde_json::from_value(serde_json::json!({
            "encryptedDesktopMcpToken":"not-valid-dpapi",
            "encryptedDesktopCliToken":"retired",
            "workspaceRootPath":"C:/isolated",
            "desktopMcpPort":47700
        }))
        .unwrap();
        let saved = serde_json::to_value(stored).unwrap();
        assert!(saved.get("encryptedDesktopMcpToken").is_none());
        assert!(saved.get("encryptedDesktopCliToken").is_none());
        assert_eq!(saved["workspaceRootPath"], "C:/isolated");
        assert_eq!(saved["desktopMcpPort"], 47700);
    }
}
