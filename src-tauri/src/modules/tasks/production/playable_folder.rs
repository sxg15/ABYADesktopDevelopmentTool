use super::*;
impl TaskService {
    pub fn playable_archive_folder(
        &self,
        task: &str,
        version: &str,
        preview: bool,
    ) -> AppResult<PathBuf> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        let expected = if preview && r.current_stage != "review" {
            r.workbench.execution.verification_package["version"].as_str()
        } else {
            r.current_version.as_deref()
        };
        if expected != Some(version) {
            return Err(AppError::validation(
                "试玩版本已变化，请刷新后打开当前版本。",
            ));
        }
        let sessions = if preview {
            &r.workbench.execution.preview_sessions
        } else {
            &r.acceptance_sessions
        };
        let mut actual = None;
        for session in sessions
            .iter()
            .rev()
            .filter(|s| s["version"] == version && s["status"] == "ready")
        {
            if let Some(path) = session["launchArchiveCopy"]
                .as_str()
                .or(session["archivePath"].as_str())
                && Path::new(path).is_dir()
                && self.production_acceptance_data_root(task, path)?.is_some()
            {
                actual = Some(PathBuf::from(path));
                break;
            }
            for id in session["instanceIds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let profile: Option<String> = self.database.with_connection(|c| {
                    Ok(c.query_row(
                        "SELECT launch_profile_json FROM game_instances WHERE task_id=?1 AND id=?2",
                        rusqlite::params![task, id],
                        |row| row.get(0),
                    )
                    .optional()?)
                })?;
                if let Some(profile) = profile {
                    let profile: Value = serde_json::from_str(&profile)?;
                    if let Some(path) = profile["archive"]["archivePath"].as_str()
                        && Path::new(path).is_dir()
                        && self.production_acceptance_data_root(task, path)?.is_some()
                    {
                        actual = Some(PathBuf::from(path));
                        break;
                    }
                }
            }
            if actual.is_some() {
                break;
            }
        }
        let source = if let Some(actual) = actual {
            actual
        } else {
            let source = if preview {
                self.preview_record(task)?
            } else {
                r
            };
            if source.current_version.as_deref() != Some(version) {
                return Err(conflict());
            }
            PathBuf::from(mutation::text(&source.version_details, "archiveSource")?)
        };
        let archive = versions::archive_root(&source)?;
        let display = self.project_save_folder(task)?;
        for entry in std::fs::read_dir(display)? {
            let path = entry?.path();
            if path.join("Main.PBArc").is_file()
                && path.canonicalize().ok().as_ref() == Some(&archive)
            {
                return Ok(path);
            }
        }
        Ok(archive)
    }
}
