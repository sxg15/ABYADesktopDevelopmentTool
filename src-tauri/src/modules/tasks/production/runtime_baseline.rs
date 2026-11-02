use super::*;

impl TaskService {
    pub fn production_check_player_pin(&self, task: &str, player: &Path) -> AppResult<()> {
        let Some(r) = self.production_get(task)?.record else {
            return Ok(());
        };
        if !execution_loop::enabled(&r) {
            return Ok(());
        }
        let Some(pinned) = r.version_details["playerSource"].as_str() else {
            return Ok(());
        };
        if Path::new(pinned).canonicalize()? != player.canonicalize()?
            || versions::player_fingerprint(player)? != r.version_details["playerBuildHash"]
        {
            return Err(AppError::new(
                "player_pin_mismatch",
                "任务已有固定运行包；不能因一次报错自行换包。需要平台维护时由 APP 明确选择和登记新构建。",
                "not_executed",
            ));
        }
        Ok(())
    }

    /// 校验实际运行包指纹；调用方只能提供由托管实例服务解析的路径。
    pub fn baseline_build_hash(&self, task: &str, player: &Path) -> AppResult<String> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if !execution_loop::enabled(&r) {
            return Err(AppError::validation("当前任务未启用 Player 基线。"));
        }
        let hash = versions::player_fingerprint(player)?;
        if r.version_details["playerBuildHash"] != hash {
            return Err(AppError::validation("实际 Player 与当前登记构建不一致。"));
        }
        Ok(hash)
    }

    /// 只保存桥接层从真实实例读取的测试结果；复用基线不会修改游戏检查或批准。
    pub fn save_runtime_baseline(&self, task: &str, hash: &str, value: Value) -> AppResult<Value> {
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(conflict());
        }
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if r.version_details["playerBuildHash"] != hash {
            return Err(conflict());
        }
        let root = self.production_workspace(task)?;
        let shared = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            ".abya-player-baselines",
        )?;
        std::fs::create_dir_all(&shared)?;
        let name = format!("{hash}.json");
        let path = shared.join(&name);
        let raw = serde_json::to_vec_pretty(&value)?;
        if raw.len() > 2 * 1024 * 1024 {
            return Err(AppError::validation("基线目录过大。"));
        }
        let temporary = shared.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&temporary, &raw)?;
        std::fs::rename(temporary, &path)?;
        self.production_commit(task, r.revision, |r| {
            r.workbench.execution.runtime_baselines.insert(
                hash.into(),
                json!({
                    "path":path,"sha256":files::hash_file(&path)?.0,"recordedAt":now(),
                    "scope":"declared-native-checks-only","sourceTask":task
                }),
            );
            Ok(())
        })?;
        Ok(baseline_summary(&value, &path, ""))
    }

    pub fn runtime_baseline_for_player(
        &self,
        task: &str,
        player: &Path,
        query: &str,
    ) -> AppResult<Value> {
        if !player.is_file() {
            return Ok(json!({"available":false,"reason":"player-not-selected-or-missing"}));
        }
        if query.len() > 256 {
            return Err(AppError::validation("能力检索词过长。"));
        }
        let hash = versions::player_fingerprint(player)?;
        self.read_runtime_baseline(task, &hash, query)
    }

    fn read_runtime_baseline(&self, task: &str, hash: &str, query: &str) -> AppResult<Value> {
        let root = self.production_workspace(task)?;
        let path = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            &format!(".abya-player-baselines/{hash}.json"),
        )?;
        if !path.is_file() {
            return Ok(Value::Null);
        }
        if std::fs::metadata(&path)?.len() > 2 * 1024 * 1024 {
            return Err(AppError::validation("基线目录过大。"));
        }
        let value: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        if value["playerBuildHash"] != hash {
            return Err(conflict());
        }
        Ok(baseline_summary(&value, &path, query))
    }
}

fn baseline_summary(value: &Value, path: &Path, query: &str) -> Value {
    let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    let capabilities:Vec<_>=value["observedCapabilities"].as_array().into_iter().flatten()
        .filter(|v|!words.is_empty()&&words.iter().all(|word|format!("{} {} {}",v["id"],v["name"],v["description"]).to_lowercase().contains(word)))
        .take(20).map(|v|json!({"id":v.get("id").or_else(||v.get("name")),"description":v["description"].as_str().unwrap_or("").chars().take(160).collect::<String>(),"riskLevel":v["riskLevel"],"requiredContext":v["requiredContext"]})).collect();
    let reports:Vec<_>=value["reports"].as_array().into_iter().flatten().map(|v|json!({"testId":v["testId"],"name":v["report"]["name"],"state":v["report"]["state"],"assertionCount":v["report"]["assertions"].as_array().map_or(0,Vec::len)})).collect();
    json!({"available":true,"schema":value["schema"],"playerBuildHash":value["playerBuildHash"],"scope":value["scope"],"sourceTaskId":value["sourceTaskId"],"reports":reports,"capabilityCount":value["observedCapabilities"].as_array().map_or(0,Vec::len),"matchingCapabilities":capabilities,"detailsPath":path,"note":value["note"]})
}
