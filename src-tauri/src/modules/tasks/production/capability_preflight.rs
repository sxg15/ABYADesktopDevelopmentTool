use super::*;
pub(super) const ISSUE: &str = "preflight:basic";

fn scope(r: &ProductionRecord) -> Value {
    json!([r.cycle, r.documents.get("requirements").map(|d| &d.sha256)])
}

impl TaskService {
    /// 回执保存在临时目录外；首次失败立即更新同一问题卡片。
    pub fn publish_capability_preflight(&self, task: &str, value: &Value) -> AppResult<()> {
        let root = self.production_workspace(task)?;
        let relative = format!(
            "artifacts/capability-preflight/{}.json",
            mutation::text(value, "runId")?
        );
        uuid::Uuid::parse_str(mutation::text(value, "runId")?).map_err(AppError::internal)?;
        files::write_file(&root, &relative, &serde_json::to_vec_pretty(value)?)?;
        let mut persisted = value.clone();
        persisted["reportPath"] = json!(relative);
        persisted["reportHash"] = json!(files::hash_file(&files::safe_path(&root, &relative)?)?.0);
        for _ in 0..3 {
            let r = self.production_get(task)?.record.ok_or_else(conflict)?;
            if r.workbench.execution.capability_preflight["runId"] != value["runId"] {
                return Err(conflict());
            }
            let result = self.production_commit(task,r.revision,|r| {
                r.workbench.execution.capability_preflight=persisted.clone();
                let failure = !value["error"].is_null() || !value["cleanupError"].is_null();
                if failure {
                    let platform = matches!(value["error"]["code"].as_str(),Some("preflight_capability_missing" | "preflight_contract_missing"));
                    let stage=value["stage"].as_str().unwrap_or("plan");
                    let issue=json!({"id":ISSUE,"preflightRunId":value["runId"],"stage":stage,"affectedStages":[stage,"implementation","review"],"kind":"blocker","status":"open",
                        "title":if value["cleanupError"].is_null(){"基础能力预检未通过"}else{"预检临时文件待清理"},"description":if value["error"].is_null(){value["cleanupError"].clone()}else{value["error"]["message"].clone()},"detectedStage":stage,"owningStage":stage,
                        "repairTarget":if platform{"abya-build"}else{"other"},"diagnosisStatus":if platform{"confirmed"}else{"suspected"},
                        "category":if platform{"platform-confirmed"}else{"runtime-unknown"},"preflightDisposition":if value["state"]=="workaround-verified"{"verified-workaround"}else{"pending"},
                        "fix":if value["state"]=="workaround-verified"{"复用本任务已有存档，保存及冷加载已验证；原创建入口问题仍保留。"}else{"核对当前包、运行状态和调用条件；有本任务已有存档时可验证复用路径，禁止自行换包。"},
                        "resumeWhen":"方案可直接提交审核；需要运行 Player 的实施或试玩操作前，在当前包完成隔离预检。",
                        "playerBuildHash":value["playerBuildHash"],"preflightReport":relative,"updatedAt":now()});
                    if let Some(old)=r.issues.iter_mut().find(|i|i["id"]==ISSUE){*old=issue;}else{r.issues.push(issue);}
                } else if value["state"]=="passed" && let Some(old)=r.issues.iter_mut().find(|i|i["id"]==ISSUE){old["status"]=json!("resolved");old["fix"]=json!("基础能力重新实测通过。");old["recheck"]=json!(relative);old["preflightDisposition"]=json!("resolved");old["preflightReport"]=json!(relative);old["preflightRunId"]=value["runId"].clone();}
                else if value["state"]=="existing-verified" && let Some(old)=r.issues.iter_mut().find(|i|i["id"]==ISSUE){
                    old["preflightDisposition"]=json!("verified-existing");old["preflightReport"]=json!(relative);old["preflightRunId"]=value["runId"].clone();
                    old["fix"]=json!("本任务已有存档的加载、保存和冷加载已验证；原新建路径问题仍保留，不阻止适用的已有存档修改。");
                }
                Ok(())
            });
            match result {
                Ok(_) => return Ok(()),
                Err(e) if e.code == "conflict" => continue,
                Err(e) => return Err(e),
            }
        }
        Err(conflict())
    }

    /// 只删除经过 UUID、父目录、owner 和无链接校验的本次测试根。
    pub fn cleanup_capability_preflight_files(&self, task: &str, run: &str) -> AppResult<()> {
        uuid::Uuid::parse_str(task).map_err(AppError::internal)?;
        uuid::Uuid::parse_str(run).map_err(AppError::internal)?;
        let root = self.production_workspace(task)?;
        let data = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            &format!(".abya-preflight/{task}/{run}"),
        )?;
        let data = if data.exists() {
            data
        } else {
            files::safe_path(
                root.parent().ok_or_else(conflict)?,
                &format!(".abya-preflight/{run}"),
            )?
        };
        if !data.exists() {
            return Ok(());
        }
        if !data.join("owner.json").exists() && std::fs::read_dir(&data)?.next().is_none() {
            std::fs::remove_dir(data)?;
            return Ok(());
        }
        let owner: Value = serde_json::from_slice(&std::fs::read(data.join("owner.json"))?)?;
        if owner["taskId"] != task
            || owner["runId"] != run
            || owner["purpose"] != "capability-preflight"
        {
            return Err(AppError::validation("预检清理目录归属不匹配，未删除。"));
        }
        versions::inventory(std::slice::from_ref(&data))?;
        // owner 最后删除，某个文件被占用时下一次仍能核对归属并继续清理。
        for child in std::fs::read_dir(&data)? {
            let child = child?;
            if child.file_name() == "owner.json" {
                continue;
            }
            if child.file_type()?.is_dir() {
                std::fs::remove_dir_all(child.path())?;
            } else {
                std::fs::remove_file(child.path())?;
            }
        }
        std::fs::remove_file(data.join("owner.json"))?;
        std::fs::remove_dir(data)?;
        Ok(())
    }

    pub fn preflight_recovery_records(&self) -> AppResult<Vec<Value>> {
        self.database.with_connection(|db| {
            let mut q = db.prepare("SELECT body FROM task_production")?;
            let rows = q.query_map([], |row| row.get::<_, String>(0))?;
            let mut result = vec![];
            for row in rows {
                let r: ProductionRecord = serde_json::from_str(&row?)?;
                let p = r.workbench.execution.capability_preflight;
                if p["cleanup"]["state"] == "pending" {
                    result.push(p);
                }
            }
            Ok(result)
        })
    }

    /// 替代路径只允许复制本任务已有编辑存档，绝不借用其他游戏或冻结快照。
    pub fn copy_preflight_existing_archive(&self, task: &str, run: &str) -> AppResult<PathBuf> {
        let roots = self.task_archive_sources(task)?;
        let root = self.production_workspace(task)?;
        uuid::Uuid::parse_str(run).map_err(AppError::internal)?;
        let destination = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            &format!(".abya-preflight/{run}/Archives"),
        )?;
        let mut archives = vec![];
        for directory in roots["editableRoots"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let directory = Path::new(directory);
            if directory.join("Main.PBArc").is_file() {
                archives.push(directory.to_path_buf());
            } else if directory.is_dir() {
                for item in std::fs::read_dir(directory)? {
                    let path = item?.path();
                    if path.join("Main.PBArc").is_file() {
                        archives.push(path);
                    }
                }
            }
        }
        archives.sort();
        archives.dedup();
        if archives.len() != 1 {
            return Err(AppError::validation(
                "复用路径需要本任务恰好一份明确的编辑存档；当前没有唯一来源。",
            ));
        }
        super::version_snapshots::copy_verified(&archives[0], &destination)
    }
}

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["capabilityPreflightVersion"] == 1
}

pub(super) fn gate(r: &ProductionRecord, root: &Path) -> AppResult<()> {
    if !enabled(r) {
        return Ok(());
    }
    let p = &r.workbench.execution.capability_preflight;
    if p["state"] == "existing-verified"
        && (p["reuseExisting"] != true
            || p["purpose"] != "maintenance"
            || !matches!(p["stage"].as_str(), Some("implementation" | "review")))
    {
        return Err(AppError::validation(
            "已有存档预检缺少有效的修改阶段与复用范围。",
        ));
    }
    let required = if matches!(
        p["state"].as_str(),
        Some("workaround-verified" | "existing-verified")
    ) {
        vec!["runtime-connect", "save-archive", "cold-load"]
    } else {
        vec![
            "runtime-connect",
            "create-archive",
            "save-archive",
            "cold-load",
        ]
    };
    if required.iter().any(|id| {
        !p["checks"].as_array().is_some_and(|checks| {
            checks
                .iter()
                .any(|c| c["id"] == *id && c["status"] == "passed")
        })
    }) {
        let message = match p["state"].as_str() {
            None => "尚未运行基础预检。开始 Player 实施前，请在 APP 中授权启动隔离预检。",
            Some("running") => "基础预检正在执行，请等待本次回执。",
            Some("failed" | "interrupted") => {
                "已有失败或中断的基础预检回执，请查看失败阶段并重新验证。"
            }
            _ => "基础预检缺少真实完成的必要检查。",
        };
        return Err(AppError::validation(message));
    }
    if !matches!(
        p["state"].as_str(),
        Some("passed" | "workaround-verified" | "existing-verified")
    ) || p["scope"] != scope(r)
        || p["cleanup"]["state"] != "completed"
    {
        return Err(AppError::validation(
            "请在当前阶段完成基础能力预检及临时存档清理，再继续编辑；未测、失败或结果未知不能视为通过。",
        ));
    }
    let report = files::safe_path(root, mutation::text(p, "reportPath")?)?;
    if files::hash_file(&report)?.0 != mutation::text(p, "reportHash")? {
        return Err(AppError::validation("预检证据已变化，请重新验证。"));
    }
    Ok(())
}

pub(super) fn workaround(r: &ProductionRecord, i: &Value) -> bool {
    let p = &r.workbench.execution.capability_preflight;
    i["id"] == ISSUE
        && matches!(
            p["state"].as_str(),
            Some("workaround-verified" | "existing-verified")
        )
        && p["scope"] == scope(r)
        && i["preflightRunId"] == p["runId"]
        && p["cleanup"]["state"] == "completed"
        && (r.version_details["playerBuildHash"].is_null()
            || r.version_details["playerBuildHash"] == p["playerBuildHash"])
}

impl TaskService {
    /// 固定隔离预检支持方案、实现和试玩修改；不改阶段或授予普通开发权限。
    #[cfg(test)]
    pub fn begin_capability_preflight(
        &self,
        task: &str,
        reuse: bool,
        retry: bool,
    ) -> AppResult<Value> {
        self.begin_capability_preflight_from(task, reuse, retry, true)
    }
    pub fn begin_capability_preflight_from(
        &self,
        task: &str,
        reuse: bool,
        retry: bool,
        user_initiated: bool,
    ) -> AppResult<Value> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if self.get_readonly(task)?.status != TaskStatus::Active
            || !matches!(
                r.current_stage.as_str(),
                "plan" | "implementation" | "review"
            )
        {
            return Err(AppError::validation(
                "基础能力预检可在活动任务的方案、实现或试玩修改阶段运行。",
            ));
        }
        let root = self.production_workspace(task)?;
        validation::approved(&r, &root, "requirements")?;
        let maintenance = matches!(r.current_stage.as_str(), "implementation" | "review");
        if maintenance {
            validation::approved(&r, &root, "plan")?;
        }
        // 试玩修改已有作品，只验证当前编辑链路，避免重新依赖无关的新建入口。
        let reuse = reuse || r.current_stage == "review";
        let player = self.task_player_status(task);
        let old = &r.workbench.execution.capability_preflight;
        if old["state"] == "running" || old["cleanup"]["state"] == "pending" {
            return Err(AppError::validation(
                "原预检尚未结束或清理，请先等待自动核对。",
            ));
        }
        if !retry
            && player.as_ref().is_ok_and(|p| p["matches"] == true)
            && player
                .as_ref()
                .is_ok_and(|p| p["selectedPath"] == old["playerPath"])
            && (old["state"] == "passed" || old["reuseExisting"] == reuse)
            && old["scope"] == scope(&r)
            && old["playerBuildHash"]
                == player
                    .as_ref()
                    .ok()
                    .map(|s| s["selectedHash"].clone())
                    .unwrap_or(Value::Null)
            && !old.is_null()
            && (!matches!(
                old["state"].as_str(),
                Some("passed" | "workaround-verified" | "existing-verified")
            ) || gate(&r, &root).is_ok())
        {
            return Ok(old.clone());
        }
        if !user_initiated
            && r.no_questions["enabled"] != true
            && !(old["userAuthorized"] == true
                && old["scope"] == scope(&r)
                && old["reuseExisting"] == reuse
                && old["stage"] == r.current_stage
                && player.as_ref().is_ok_and(|p| {
                    p["matches"] == true
                        && p["selectedHash"] == old["playerBuildHash"]
                        && p["selectedPath"] == old["playerPath"]
                }))
        {
            return Err(AppError::new(
                "preflight_authorization_required",
                "请在 APP 的基础能力预检面板授权启动隔离 Player。方案可直接提交和审核。",
                "not_executed",
            ));
        }
        let run = uuid::Uuid::new_v4().to_string();
        let data = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            &format!(".abya-preflight/{run}"),
        )?;
        let mut value = json!({"runId":run,"taskId":task,"stage":r.current_stage,"purpose":if maintenance{"maintenance"}else{"initialization"},"scope":scope(&r),"suiteVersion":1,"state":"running","phase":"starting","reuseExisting":reuse,
            "dataRoot":data,"instances":[],"checks":[],"cleanup":{"state":"pending"},"startedAt":now(),"reportPath":format!("artifacts/capability-preflight/{run}.json")});
        value["userAuthorized"] = json!(true);
        if reuse {
            value["routeIssue"] = old["error"].clone();
        }
        match player {
            Ok(p) if p["matches"] == true => {
                value["playerPath"] = p["selectedPath"].clone();
                value["playerBuildHash"] = p["selectedHash"].clone();
            }
            Ok(_) => {
                value["error"] = json!({"code":"player_pin_mismatch","message":"当前 APP 包与任务记录不同，请先在启动窗口确认切换。"});
            }
            Err(e) => {
                value["error"] = json!({"code":e.code,"message":e.message});
            }
        }
        self.production_commit(task, r.revision, |r| {
            r.policy["capabilityPreflightVersion"] = json!(1);
            r.workbench.execution.capability_preflight = value.clone();
            Ok(())
        })?;
        if value["error"].is_null() {
            std::fs::create_dir_all(&data)?;
            std::fs::write(
                data.join("owner.json"),
                serde_json::to_vec(
                    &json!({"taskId":task,"runId":run,"purpose":"capability-preflight"}),
                )?,
            )?;
        }
        Ok(value)
    }
}

impl TaskService {
    pub fn validate_current_preflight(&self, task: &str) -> AppResult<()> {
        let Some(r) = self.production_get(task)?.record else {
            return Ok(());
        };
        if !enabled(&r) {
            return Ok(());
        }
        gate(&r, &self.production_workspace(task)?)?;
        let current = self.task_player_status(task)?;
        let p = &r.workbench.execution.capability_preflight;
        if current["selectedHash"] != p["playerBuildHash"]
            || current["selectedPath"] != p["playerPath"]
        {
            return Err(AppError::validation(
                "APP 选定包或指纹已变化，请在当前阶段重新执行基础能力预检。",
            ));
        }
        Ok(())
    }
}
