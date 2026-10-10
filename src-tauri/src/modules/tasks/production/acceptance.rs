use super::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceSpec {
    pub version: String,
    pub executable_path: String,
    pub archive_path: String,
    pub archive_guid: String,
    pub level_guid: String,
    pub seats: u8,
    pub required_tools: Vec<String>,
    pub require_ui: bool,
}

fn launch_path(value: String) -> String {
    if let Some(path) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{path}");
    }
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}

pub(super) fn configure(r: &mut ProductionRecord, data: &Value) -> AppResult<()> {
    let version = validation::version(r)?.to_owned();
    let seats = data["seats"]
        .as_u64()
        .unwrap_or(if r.player_mode == "multiplayer" { 2 } else { 1 });
    if !(1..=8).contains(&seats)
        || (r.player_mode == "single" && seats != 1)
        || (r.player_mode == "multiplayer" && seats < 2)
    {
        return Err(AppError::validation(
            "验收人数须符合已确认模式，支持1至8席。",
        ));
    }
    let required = data.get("requiredTools").cloned().unwrap_or(json!([]));
    if required.as_array().is_none_or(|items| {
        items.len() > 64
            || items.iter().any(|v| {
                v.as_str().is_none_or(|s| {
                    s.is_empty()
                        || s.len() > 128
                        || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                })
            })
    }) {
        return Err(AppError::validation(
            "requiredTools 必须为运行时能力名称数组。",
        ));
    }
    r.acceptance_config = json!({"version":version,"seats":seats,"requiredTools":required,
        "requireUi":data["requireUi"].as_bool().unwrap_or(true)});
    Ok(())
}

impl TaskService {
    pub fn acceptance_spec(&self, id: &str) -> AppResult<AcceptanceSpec> {
        let root = self.production_workspace(id)?;
        let r = self
            .database
            .with_connection(|c| load(c, id))?
            .ok_or_else(|| AppError::validation("请先登记可试玩版本。"))?;
        validation::approved(&r, &root, "requirements")?;
        validation::approved(&r, &root, "plan")?;
        if !["single", "multiplayer"].contains(&r.player_mode.as_str()) {
            return Err(AppError::validation("请先确认单人或多人模式。"));
        }
        versions::verify(&r.version_details)?;
        let version = validation::version(&r)?.to_owned();
        if !r.acceptance_config.is_null() && r.acceptance_config["version"] != version {
            return Err(AppError::validation(
                "验收配置属于旧版本，请登记当前版本的验收配置。",
            ));
        }
        let string = |key| mutation::text(&r.version_details, key).map(str::to_owned);
        Ok(AcceptanceSpec {
            version,
            executable_path: launch_path(string("playerSource")?),
            archive_path: launch_path(string("archiveSource")?),
            archive_guid: string("archiveGuid")?,
            level_guid: string("levelGuid")?,
            seats: r.acceptance_config["seats"]
                .as_u64()
                .unwrap_or(if r.player_mode == "multiplayer" { 2 } else { 1 })
                as u8,
            required_tools: r.acceptance_config["requiredTools"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            require_ui: r.acceptance_config["requireUi"].as_bool().unwrap_or(true),
        })
    }

    pub fn acceptance_last(&self, id: &str) -> AppResult<Option<Value>> {
        Ok(self
            .database
            .with_connection(|c| load(c, id))?
            .and_then(|r| r.acceptance_sessions.last().cloned()))
    }

    pub fn acceptance_record(&self, id: &str, version: &str, session: Value) -> AppResult<()> {
        let r = self
            .production_get(id)?
            .record
            .ok_or_else(|| AppError::validation("缺少制作记录。"))?;
        self.production_commit(id, r.revision, |r| {
            if r.current_version.as_deref() != Some(version) {
                return Err(AppError::validation(
                    "启动期间作品版本变化，请核对后重新启动。",
                ));
            }
            r.acceptance_sessions.push(session);
            if r.acceptance_sessions.len() > 50 {
                r.acceptance_sessions.remove(0);
            }
            Ok(())
        })?;
        self.production_export(id).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_windows_fingerprints_use_normal_player_launch_paths() {
        assert_eq!(
            launch_path(r"\\?\C:\Archives\Cat".into()),
            r"C:\Archives\Cat"
        );
        assert_eq!(
            launch_path(r"\\?\UNC\server\share\Cat".into()),
            r"\\server\share\Cat"
        );
        assert_eq!(launch_path("/tmp/archive".into()), "/tmp/archive");
    }
}
