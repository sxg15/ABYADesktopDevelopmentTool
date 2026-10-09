use super::*;
use serde_json::Value;

const FILE: &str = "execution-settings.json";

pub(super) fn read(directory: &Path) -> AppResult<Value> {
    let file = directory.join(FILE);
    if !file.exists() {
        return Ok(json!({}));
    }
    Ok(serde_json::from_slice(&std::fs::read(file)?)?)
}

pub(super) fn save(directory: &Path, source: &Value) -> AppResult<Value> {
    let mut value = json!({});
    for key in [
        "model",
        "effort",
        "approvalPolicy",
        "approvalsReviewer",
        "sandboxPolicy",
    ] {
        if let Some(v) = source.get(key).filter(|v| !v.is_null()) {
            value[key] = v.clone();
        }
    }
    if value["effort"].is_null()
        && let Some(v) = source.get("reasoningEffort")
    {
        value["effort"] = v.clone();
    }
    if value["sandboxPolicy"].is_null()
        && let Some(v) = source.get("sandbox")
    {
        value["sandboxPolicy"] = v.clone();
    }
    if value["model"].is_null() {
        return Ok(value);
    }
    std::fs::create_dir_all(directory)?;
    let tmp = directory.join(format!("execution-settings-{}.tmp", Uuid::new_v4()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&value)?)?;
    std::fs::rename(tmp, directory.join(FILE))?;
    Ok(value)
}

pub(super) fn params(native: &str, settings: &Value) -> Value {
    let mut p = json!({"threadId":native});
    for key in [
        "model",
        "effort",
        "approvalPolicy",
        "approvalsReviewer",
        "sandboxPolicy",
    ] {
        if let Some(v) = settings.get(key).filter(|v| !v.is_null()) {
            p[key] = v.clone();
        }
    }
    p
}

impl CodexTerminalService {
    pub fn execution_settings(&self, task_id: &str, conversation_id: &str) -> AppResult<Value> {
        let (codex, task) = self.validate_open_request(task_id, conversation_id)?;
        let conversation = self.conversation(task_id, conversation_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let settings = read(&directory)?;
        let server = self.runtime_servers.lock().get(conversation_id).cloned();
        let metadata = match &server {
            Some(s) if s.is_alive() => s.clone(),
            _ => self.ensure_app_server(&codex)?,
        };
        let mut models = vec![];
        let mut cursor = Value::Null;
        loop {
            let response = metadata.request(
                "model/list",
                json!({"limit":100,"includeHidden":false,"cursor":cursor}),
            )?;
            models.extend(response["data"].as_array().cloned().unwrap_or_default());
            cursor = response["nextCursor"].clone();
            if cursor.is_null() || models.len() > 1000 {
                break;
            }
        }
        let running = if let (Some(s), Some(id)) = (&server, &conversation.native_session_id) {
            let snapshot = s.request("thread/read", json!({"threadId":id,"includeTurns":false}))?;
            snapshot["thread"]["status"]["type"] == "active"
        } else {
            false
        };
        Ok(
            json!({"settings":settings,"models":models,"running":running,"connected":server.as_ref().is_some_and(|s|s.is_alive())}),
        )
    }

    pub fn update_execution_settings(
        &self,
        task_id: &str,
        conversation_id: &str,
        input: Value,
    ) -> AppResult<Value> {
        let _guard = self.lifecycle_lock.lock();
        let (_, task) = self.validate_open_request(task_id, conversation_id)?;
        let conversation = self.conversation(task_id, conversation_id)?;
        if conversation.archived {
            return Err(AppError::validation("请先恢复此会话。"));
        }
        let native = conversation
            .native_session_id
            .as_deref()
            .ok_or_else(|| AppError::validation("请先打开终端。"))?;
        let server = self
            .runtime_servers
            .lock()
            .get(conversation_id)
            .cloned()
            .filter(|s| s.is_alive())
            .ok_or_else(|| AppError::validation("请先连接终端，再修改执行设置。"))?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let mut desired = read(&directory)?;
        if input.get("effort").is_some() && !input["model"].is_string() {
            return Err(AppError::validation("修改思考强度时请同时指定当前模型。"));
        }
        if let Some(model) = input["model"].as_str() {
            let catalog = self.execution_settings(task_id, conversation_id)?;
            let entry = catalog["models"]
                .as_array()
                .and_then(|ms| ms.iter().find(|m| m["model"] == model))
                .ok_or_else(|| AppError::validation("当前账号暂不可使用此模型，请刷新列表。"))?;
            let effort = input["effort"]
                .as_str()
                .or(entry["defaultReasoningEffort"].as_str())
                .unwrap_or("medium");
            if !entry["supportedReasoningEfforts"]
                .as_array()
                .is_some_and(|es| es.iter().any(|e| e["reasoningEffort"] == effort))
            {
                return Err(AppError::validation("此模型不支持所选思考强度。"));
            }
            desired["model"] = json!(model);
            desired["effort"] = json!(effort);
        }
        if let Some(mode) = input["permissionMode"].as_str() {
            match mode {
                "auto-review" => {
                    desired["approvalPolicy"] = json!("on-request");
                    desired["approvalsReviewer"] = json!("auto_review");
                    desired["sandboxPolicy"] = json!({"type":"workspaceWrite","writableRoots":[task.workspace_path],"networkAccess":true});
                }
                "full-access" => {
                    desired["approvalPolicy"] = json!("never");
                    desired["approvalsReviewer"] = json!("user");
                    desired["sandboxPolicy"] = json!({"type":"dangerFullAccess"});
                }
                _ => return Err(AppError::validation("请选择AI自动审核或完全同意。")),
            }
        }
        if desired["model"].is_null() {
            return Err(AppError::validation("尚未读取当前模型，请重新连接终端。"));
        }
        if input["applyNow"] == true {
            server.interrupt_and_wait(native)?;
        }
        server.request("thread/settings/update", params(native, &desired))?;
        save(&directory, &desired)?;
        self.execution_settings(task_id, conversation_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_persistence_excludes_credentials_and_preserves_native_values() {
        let dir = std::env::temp_dir().join(format!("abya-settings-{}", Uuid::new_v4()));
        let value=save(&dir,&json!({"model":"test","reasoningEffort":"high","approvalPolicy":"on-request","approvalsReviewer":"auto_review","sandbox":{"type":"workspaceWrite"},"token":"secret","config":{"password":"secret"}})).unwrap();
        assert_eq!(read(&dir).unwrap(), value);
        assert!(!value.to_string().contains("secret"));
        let p = params("native", &value);
        assert_eq!(p["threadId"], "native");
        assert_eq!(p["effort"], "high");
        assert_eq!(p["sandboxPolicy"]["type"], "workspaceWrite");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
