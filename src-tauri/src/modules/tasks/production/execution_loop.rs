use super::*;

pub(super) fn enabled(r: &ProductionRecord) -> bool {
    r.policy["implementationExecution"]["inPlaceRecoveryVersion"] == 1
}

pub(super) fn goal_elapsed(r: &ProductionRecord, goal: &str) -> i64 {
    let scope = autonomy::scope(r);
    r.workbench
        .execution
        .units
        .iter()
        .filter(|u| u.goal_id == goal && u.cycle == r.cycle && u.scope_hash == scope)
        .map(|u| u.elapsed_ms)
        .sum()
}
pub(super) fn goal_limit(r: &ProductionRecord) -> i64 {
    r.policy["implementationExecution"]["goalMinutes"]
        .as_i64()
        .unwrap_or(180)
        .clamp(30, 480)
        * 60_000
}
pub(super) fn goal_used(r: &ProductionRecord, goal: &str) -> i64 {
    goal_elapsed(r, goal)
        .saturating_sub(
            *r.workbench
                .execution
                .goal_budget_epochs
                .get(&format!("{}:{goal}", autonomy::scope(r)))
                .unwrap_or(&0),
        )
        .max(0)
}

/// 全目标上限与独立收尾预留防止多个工作项各自续期导致总耗时无限增长。
pub(super) fn budget_exhausted(r: &ProductionRecord, u: &autonomy::WorkUnit, extra: i64) -> bool {
    enabled(r)
        && if u.kind == "finalization" {
            u.elapsed_ms + extra >= autonomy_budget::limit(r, "settlementMinutes", 15) * 60_000
        } else {
            goal_used(r, &u.goal_id) + extra >= goal_limit(r)
        }
}

/// 分类只描述实际观察，不把验证失败转换成产品或平台根因。
pub(super) fn validate_observation(data: &Value) -> AppResult<()> {
    let Some(value) = data.get("observation") else {
        return Ok(());
    };
    if ![
        "not-tested",
        "verification-unavailable",
        "verification-failed",
        "functional-failure",
        "none",
    ]
    .contains(&value["kind"].as_str().unwrap_or(""))
        || ![
            "tool-contract",
            "runtime-state",
            "data-version",
            "verification-method",
            "implementation",
            "platform-suspected",
            "permission",
            "service",
            "unknown",
        ]
        .contains(&value["origin"].as_str().unwrap_or(""))
    {
        return Err(AppError::validation(
            "需分别提供观察类型和待核对的原因；平台确诊使用问题证据接口。",
        ));
    }
    mutation::text(value, "expected")?;
    mutation::text(value, "observed")?;
    Ok(())
}

/// 普通失败直接续接原项；没有新结果时最多三轮不同处理，不创建修复规划任务。
pub(super) fn recover(r: &mut ProductionRecord, id: &str) {
    if !enabled(r) {
        return;
    }
    let exhausted = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| u.id == id)
        .is_some_and(|u| autonomy_budget::exhausted(r, u));
    let waiting_dependency = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| u.id == id)
        .is_some_and(|u| {
            u.depends_on.iter().any(|dependency| {
                r.workbench
                    .execution
                    .units
                    .iter()
                    .any(|other| &other.id == dependency && other.status != "succeeded")
            })
        });
    let Some(u) = r.workbench.execution.units.iter_mut().find(|u| u.id == id) else {
        return;
    };
    if !matches!(
        u.status.as_str(),
        "failed" | "deferred" | "awaiting-condition"
    ) {
        return;
    }
    if u.defer_reason_code == "dependency" && waiting_dependency {
        return;
    }
    if u.defer_reason_code == "unknown-result" {
        u.status = "awaiting-condition".into();
        u.stop_reason = "reconcile-write".into();
        enqueue(r, id, "reconcile-write");
        return;
    }
    // A model-supplied cause is not a native permission gate. Actual approvals,
    // pauses and tool authorization remain enforced before dispatch elsewhere.
    if exhausted || u.recovery_rounds >= 3 {
        let reason = if exhausted {
            "execution-budget"
        } else {
            "no-progress"
        };
        u.status = "deferred".into();
        u.stop_reason = reason.into();
        enqueue(r, id, reason);
        return;
    }
    u.recovery_history
        .push(json!({"at":now(),"status":u.status,"reason":u.reason,
        "observation":u.observation,"resultHash":u.result_hash,"resultPaths":u.result_paths,
        "elapsedMs":u.elapsed_ms,"route":u.route}));
    u.recovery_rounds += 1;
    u.recovery_strategy = match u.recovery_rounds {
        1 => "先核对要求是否实际失败，检查调用、实例、版本及验证方法；修正后在原工作项复验。",
        2 => "上一处理未闭环；选择实质不同的实现或等效验证路径，不能重复原假设或降低必要要求。",
        _ => "复核原假设和已有成果，完成最小可验证收尾；仍不可完成则如实保全并移交内部技术队列。",
    }
    .into();
    if u.route + 1 < u.routes.len() {
        u.route += 1;
    }
    u.status = "pending".into();
    u.stop_reason = "automatic-recovery".into();
    u.defer_reason_code.clear();
    u.attempts = 0;
    u.route_elapsed_ms = 0;
    u.attempt_started_at.clear();
    autonomy::event(
        &mut r.workbench.execution,
        "recovering-in-place",
        Some(id),
        "原工作项自动复核并换路径；不新增人工继续请求。",
    );
}

/// 持久化明确的内部处理出口；该队列不代表游戏通过或要求策划点击继续。
pub(super) fn enqueue(r: &mut ProductionRecord, id: &str, reason: &str) {
    let Some(u) = r.workbench.execution.units.iter().find(|u| u.id == id) else {
        return;
    };
    r.workbench.execution.technical_queue.insert(
        id.into(),
        json!({
            "unitId":id,"goalId":u.goal_id,"reason":reason,"status":"open",
            "observation":u.observation,"resultPaths":u.result_paths,"issueIds":u.issue_ids,
            "elapsedMs":u.elapsed_ms,"nextCondition":u.next_condition,"updatedAt":now()
        }),
    );
}

/// 时间片或普通轮次结束不是功能失败；保留在原项内继续，不生成失败诊断。
pub(super) fn checkpoint(r: &mut ProductionRecord) {
    if !enabled(r) {
        return;
    }
    let Some(id) = r.workbench.execution.active_unit_id.clone() else {
        return;
    };
    let Some(u) = r
        .workbench
        .execution
        .units
        .iter_mut()
        .find(|u| u.id == id && u.status == "running")
    else {
        return;
    };
    u.unreported_turns += 1;
    if u.unreported_turns >= 3 {
        u.status = "deferred".into();
        u.stop_reason = "missing-result-report".into();
        u.reason = "连续轮次未交还可核对的工作结果；保留原操作与成果，停止重复派发。".into();
        u.attempt_started_at.clear();
        enqueue(r, &id, "missing-result-report");
        return;
    }
    u.status = "pending".into();
    u.recovery_strategy =
        "先核对上一轮已完成的工具与文件，只补结果登记或确实未完成的动作；不要重放已有写入。".into();
    u.stop_reason = "turn-checkpoint".into();
    u.attempt_started_at.clear();
    autonomy::event(
        &mut r.workbench.execution,
        "checkpoint",
        Some(&id),
        "本轮结束，沿原工作项继续剩余动作；未判定功能失败。",
    );
}

pub(super) fn settle_queue(r: &mut ProductionRecord) {
    if !enabled(r) {
        return;
    }
    let relevant = completion::relevant(r);
    let ids: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| relevant.contains(&u.id) && u.status != "succeeded")
        .map(|u| {
            (
                u.id.clone(),
                if u.defer_reason_code == "unknown-result" {
                    "reconcile-write"
                } else if u.defer_reason_code == "dependency" {
                    "dependency"
                } else if u.defer_reason_code == "budget-exhausted" {
                    "execution-budget"
                } else {
                    "technical-review"
                },
            )
        })
        .collect();
    for (id, reason) in ids {
        enqueue(r, &id, reason);
    }
}

pub(super) fn complete(r: &mut ProductionRecord, id: &str) {
    if let Some(u) = r.workbench.execution.units.iter_mut().find(|u| u.id == id) {
        u.stop_reason = "completed".into();
    }
    if let Some(item) = r.workbench.execution.technical_queue.get_mut(id) {
        item["status"] = json!("resolved");
        item["resolvedAt"] = json!(now());
    }
}

pub(super) fn reopen_finalization(r: &mut ProductionRecord) {
    if !enabled(r) {
        return;
    }
    let signature = completion::signature(r);
    let id = r
        .workbench
        .execution
        .units
        .iter()
        .find(|u| {
            u.kind == "finalization"
                && Some(&u.goal_id) == r.workbench.active_goal_id.as_ref()
                && u.status == "deferred"
                && u.defer_reason_code != "unknown-result"
                && u.input_signature != signature
                && !autonomy_budget::exhausted(r, u)
        })
        .map(|u| u.id.clone());
    if let Some(id) = id {
        let u = r
            .workbench
            .execution
            .units
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap();
        u.status = "pending".into();
        u.recovery_rounds = 0;
        u.unreported_turns = 0;
        u.input_signature = signature;
        u.stop_reason = "new-result-finalization".into();
        u.defer_reason_code.clear();
    }
}

/// 显式升级时吸收未结束的旧规划节点，不丢弃原结果、预算或未知写入。
pub(super) fn migrate(r: &mut ProductionRecord) {
    if !enabled(r) || r.current_stage != "implementation" {
        return;
    }
    let relevant = completion::relevant(r);
    if let Some(goal) = r.workbench.active_goal_id.clone() {
        let elapsed = goal_elapsed(r, &goal);
        let epoch_key = format!("{}:{goal}", autonomy::scope(r));
        r.workbench
            .execution
            .goal_budget_epochs
            .entry(epoch_key)
            .or_insert(elapsed);
    }
    let replacements: BTreeMap<_, _> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| {
            relevant.contains(&u.id)
                && u.kind == "repair-plan"
                && !u.repair_for.is_empty()
                && !matches!(u.status.as_str(), "succeeded" | "awaiting-condition")
                && u.defer_reason_code != "unknown-result"
        })
        .map(|u| (u.id.clone(), u.repair_for.clone()))
        .collect();
    for u in &mut r.workbench.execution.units {
        if !relevant.contains(&u.id) {
            continue;
        }
        if replacements.contains_key(&u.id) {
            u.definition_history.push(
                json!({"status":u.status,"reason":u.reason,"elapsedMs":u.elapsed_ms,"at":now()}),
            );
            u.status = "superseded".into();
            u.reason = "显式升级后由原工作项承接恢复；旧诊断结果与时间保留。".into();
        } else {
            for dep in &mut u.depends_on {
                if let Some(original) = replacements.get(dep) {
                    *dep = original.clone();
                }
            }
            u.depends_on.retain(|id| id != &u.id);
        }
        if u.kind == "finalization" && u.status != "succeeded" {
            u.routes=vec!["核对当前成果与必要检查，保全实际可玩包；只补尚缺动作，正式条件满足再交试玩，其他情况如实保留技术待补项。".into()];
            u.route = 0;
        }
        u.attempt_started_at.clear();
        if u.status == "running" {
            u.status = "pending".into();
            u.stop_reason = "upgrade-checkpoint".into();
        }
    }
    let retry: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| {
            completion::is_repairable(u)
                && !matches!(u.kind.as_str(), "repair-plan" | "handoff-repair")
        })
        .map(|u| u.id.clone())
        .collect();
    for id in retry {
        recover(r, &id);
    }
    r.workbench.execution.state = "paused".into();
    r.workbench.execution.active_unit_id = None;
    autonomy::event(
        &mut r.workbench.execution,
        "execution-contract-upgraded",
        None,
        "切换同项恢复，保留批准、成果、累计耗时与未知写入；未自动启动。",
    );
}

impl TaskService {
    /// 新任务的编辑数据独立保存，避免默认全局存档目录混入其他任务的旧状态。
    pub fn production_authoring_data_root(&self, task: &str) -> AppResult<Option<PathBuf>> {
        let r = self.production_get(task)?.record;
        if r.as_ref().is_none_or(|r| !enabled(r)) {
            return Ok(None);
        }
        uuid::Uuid::parse_str(task).map_err(|_| conflict())?;
        let root = self.production_workspace(task)?;
        let data = files::safe_path(
            root.parent().ok_or_else(conflict)?,
            &format!(".abya-authoring/{task}"),
        )?;
        let owner = data.join("owner.json");
        if data.exists() {
            let value: Value = serde_json::from_slice(&std::fs::read(&owner)?)?;
            if value["taskId"] != task {
                return Err(AppError::validation("编辑数据目录归属不匹配。"));
            }
        } else {
            std::fs::create_dir_all(&data)?;
            std::fs::write(
                owner,
                serde_json::to_vec(&json!({"taskId":task,"purpose":"authoring"}))?,
            )?;
        }
        Ok(Some(data))
    }

    /// 明确恢复只重排原项；历史轮次入账，总目标额度不重置。
    pub fn production_resume_in_place(&self, task: &str) -> AppResult<()> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if !enabled(&r) || !autonomy::autonomous_implementation(&r) {
            return Ok(());
        }
        let relevant = completion::relevant(&r);
        if r.workbench
            .execution
            .units
            .iter()
            .any(|u| relevant.contains(&u.id) && matches!(u.status.as_str(), "pending" | "running"))
        {
            return Ok(());
        }
        let id = r
            .workbench
            .execution
            .units
            .iter()
            .find(|u| {
                relevant.contains(&u.id)
                    && u.status == "deferred"
                    && u.defer_reason_code != "unknown-result"
                    && !autonomy_budget::exhausted(&r, u)
            })
            .map(|u| u.id.clone())
            .ok_or_else(|| {
                AppError::validation(
                    "当前仅剩需对账或超出总执行额度的工作；成果保留，未生成新的诊断任务。",
                )
            })?;
        self.production_commit(task,r.revision,|r|{
            let u=r.workbench.execution.units.iter_mut().find(|u|u.id==id).unwrap();
            u.recovery_history.push(json!({"at":now(),"kind":"explicit-user-resume","previousRounds":u.recovery_rounds,"elapsedMs":u.elapsed_ms,"reason":u.reason}));
            u.recovery_rounds=0;u.unreported_turns=0;u.status="pending".into();u.stop_reason="user-resume".into();u.defer_reason_code.clear();
            u.recovery_strategy="用户明确恢复；先核对已有结果和新的条件，仅继续原项剩余动作。".into();
            r.workbench.execution.state="ready".into();
            Ok(())
        })
    }

    /// 其他阶段结束一轮却没有交付物时，原目标有限续接，不向策划反复索要继续。
    pub fn production_stage_checkpoint(
        &self,
        task: &str,
        turn: &str,
        material: &str,
    ) -> AppResult<bool> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if !enabled(&r) || autonomy::autonomous_implementation(&r) {
            return Ok(true);
        }
        let key = format!(
            "{}:{}:{}",
            r.current_stage,
            r.cycle,
            r.workbench.active_goal_id.as_deref().unwrap_or("stage")
        );
        let previous = &r
            .workbench
            .execution
            .stage_recovery
            .get(&key)
            .cloned()
            .unwrap_or(Value::Null);
        if previous["turnId"] == turn {
            return Ok(previous["continue"] == true);
        }
        let unchanged = previous["material"] == material;
        let count = if unchanged {
            previous["unchangedTurns"].as_u64().unwrap_or(0) + 1
        } else {
            0
        };
        let proceed = count < 2;
        self.production_commit(task,r.revision,|r|{
            r.workbench.execution.stage_recovery.insert(key.clone(),json!({"turnId":turn,"material":material,"unchangedTurns":count,"continue":proceed}));
            if proceed && r.workbench.execution.state=="technicalReview" {
                r.workbench.execution.state="ready".into();
                if let Some(item)=r.workbench.execution.technical_queue.get_mut(&key){item["status"]=json!("in-progress");}
            }
            if !proceed {
                r.workbench.execution.state="technicalReview".into();
                r.workbench.execution.technical_queue.insert(key.clone(),json!({"unitId":key,"status":"open","reason":"no-artifact-progress","stage":r.current_stage,"goalId":r.workbench.active_goal_id,"updatedAt":now()}));
                autonomy::event(&mut r.workbench.execution,"stage-technical-review",None,"连续轮次未形成当前阶段交付物；资料已保全，等待技术处理，不要求策划反复继续。");
            }
            Ok(())
        })?;
        Ok(proceed)
    }
}
