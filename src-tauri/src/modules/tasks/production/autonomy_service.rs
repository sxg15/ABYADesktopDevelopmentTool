use super::autonomy::{self, WorkUnit};
use super::*;

impl TaskService {
    pub fn production_current(&self, task: &str) -> AppResult<Value> {
        let t = self.get_readonly(task)?;
        let view = self.production_attention(&t)?;
        let Some(r) = view.record else {
            return Ok(json!({"taskId":task,"revision":0}));
        };
        let scope_hash = autonomy::scope(&r);
        let ledger = &r.workbench.execution;
        let mut ordered: Vec<_> = ledger
            .units
            .iter()
            .filter(|u| u.cycle == r.cycle && u.scope_hash == scope_hash)
            .collect();
        ordered.sort_by_key(|u| {
            (
                Some(&u.goal_id) != r.workbench.active_goal_id.as_ref(),
                u.status == "succeeded",
            )
        });
        let current_units:Vec<_>=ordered.iter().map(|u|json!({"id":u.id,"goalId":u.goal_id,"title":u.title,"kind":u.kind,"status":u.status,"dependsOn":u.depends_on,"definitionRevision":u.definition_revision,"milestone":u.milestone,"deferReasonCode":u.defer_reason_code,"nextCondition":u.next_condition,"repairFor":u.repair_for})).collect();
        Ok(
            json!({"taskId":task,"revision":r.revision,"cycle":r.cycle,"stage":r.current_stage,
            "workflowVersion":r.workflow_version,"implementationPolicy":r.policy["implementationExecution"],
            "preflightRecovery":ledger.preflight,
            "fixtureVersions":ledger.validation_snapshots.iter().take(16).map(|(id,v)|json!({"id":id,"cycle":v["cycle"],"archiveHash":v["archiveHash"]})).collect::<Vec<_>>(),
            "version":r.current_version,"autonomous":autonomy::autonomous_implementation(&r),
            "requests":r.workbench.requests.iter().rev().filter(|q|q["status"]!="completed"&&q["stage"]==r.current_stage).take(12).map(|q|json!({"id":q["id"],"revision":q["revision"],"status":q["status"],"text":q["text"].as_str().unwrap_or("").chars().take(1000).collect::<String>()})).collect::<Vec<_>>(),
            "documentRefs":r.documents.iter().map(|(kind,d)|json!({"kind":kind,"path":d.path,"sha256":d.sha256,"revision":d.revision})).collect::<Vec<_>>(),
            "executionTiming":super::validation_runs::timing(&r),"validationImpact":{"mode":r.version_details["validationImpact"]["mode"],"unknownImpact":r.version_details["validationImpact"]["unknownImpact"],"mustRetest":r.version_details["validationImpact"]["mustRetest"],"changedFileCount":r.version_details["validationImpact"]["changedFiles"].as_array().map_or(0,Vec::len)},"executionState":ledger.state,"activeGoalId":r.workbench.active_goal_id,
            "activeUnit":ledger.units.iter().find(|u|Some(&u.id)==ledger.active_unit_id.as_ref()),
            "workUnitCount":ledger.units.iter().filter(|u|u.cycle==r.cycle&&u.scope_hash==scope_hash).count(),
            "workUnits":current_units, "workUnitsTruncated":false, "verificationPackage":ledger.verification_package,
            "issueCount":r.issues.iter().filter(|i|i["status"]!="resolved").count(),
            "issues":r.issues.iter().filter(|i|i["status"]!="resolved").take(16).map(|i|json!({"id":i["id"],"status":i["status"],"repairTarget":i["repairTarget"],"description":i["description"].as_str().unwrap_or("").chars().take(300).collect::<String>(),"handling":{"mode":i["executionHandling"]["mode"],"reason":i["executionHandling"]["reason"].as_str().unwrap_or("").chars().take(200).collect::<String>(),"impact":i["executionHandling"]["impact"].as_str().unwrap_or("").chars().take(200).collect::<String>()}})).collect::<Vec<_>>(),
            "pendingQuestions":r.question_groups.iter().filter(|g|g.status=="pending").map(|g|&g.id).collect::<Vec<_>>(),
            "approvals":view.approval_statuses,"stageStatuses":view.stage_statuses,
            "latestEvents":ledger.events.iter().rev().take(12).collect::<Vec<_>>(),
            "note":"当前摘要是导航索引；文件、审批和操作结果仍需按引用核对。"}),
        )
    }
    pub fn production_schedule(
        &self,
        task: &str,
        event_id: &str,
        advance: bool,
    ) -> AppResult<Option<WorkUnit>> {
        for _ in 0..3 {
            let r = self
                .database
                .with_connection(|c| load(c, task))?
                .ok_or_else(conflict)?;
            if !autonomy::autonomous_implementation(&r)
                || ((r.workbench.execution.state == "settledWithGaps"
                    || completion::blocked_state(&r.workbench.execution.state))
                    && r.workbench.execution.parked_condition == autonomy::condition(&r))
            {
                return Ok(None);
            }
            let root = self.production_workspace(task)?;
            validation::approved(&r, &root, "requirements")?;
            validation::approved(&r, &root, "plan")?;
            let scope_hash = autonomy::scope(&r);
            if r.workbench.execution.scope_hash == scope_hash
                && (r.workbench.execution.last_event == event_id
                    || (!advance
                        && r.workbench.execution.state == "running"
                        && r.workbench.execution.units.iter().any(|u| {
                            Some(&u.id) == r.workbench.execution.active_unit_id.as_ref()
                                && u.status == "running"
                        })))
            {
                return Ok(r
                    .workbench
                    .execution
                    .units
                    .iter()
                    .find(|u| {
                        Some(&u.id) == r.workbench.execution.active_unit_id.as_ref()
                            && u.status == "running"
                    })
                    .cloned());
            }
            let result = self.production_commit(task, r.revision, |r| {
                schedule(r, event_id, advance)?;
                if matches!(
                    r.workbench.execution.state.as_str(),
                    "waitingDependencies" | "settled"
                ) {
                    finish_available_work(r, &root)?;
                }
                Ok(())
            });
            match result {
                Err(e) if e.code == "conflict" => continue,
                other => other?,
            }
            let r = self
                .database
                .with_connection(|c| load(c, task))?
                .ok_or_else(conflict)?;
            return Ok(r.workbench.execution.units.into_iter().find(|u| {
                Some(&u.id) == r.workbench.execution.active_unit_id.as_ref()
                    && u.status == "running"
            }));
        }
        Err(conflict())
    }
}
fn finish_available_work(r: &mut ProductionRecord, root: &Path) -> AppResult<()> {
    let scope = autonomy::scope(r);
    let relevant = completion::relevant(r);
    let conditions = r
        .workbench
        .execution
        .units
        .iter()
        .map(|u| (u.id.clone(), autonomy::unit_condition(r, u)))
        .collect::<BTreeMap<_, _>>();
    for u in &mut r.workbench.execution.units {
        if relevant.contains(&u.id)
            && u.cycle == r.cycle
            && u.scope_hash == scope
            && u.status == "pending"
        {
            u.status = "deferred".into();
            u.defer_reason_code = "dependency".into();
            u.reason = "前置工作未完成；本轮暂缓部署或验证，已有成果保留。".into();
            u.condition_hash = conditions.get(&u.id).cloned().unwrap_or_default();
        }
    }
    let units: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| relevant.contains(&u.id) && u.cycle == r.cycle && u.scope_hash == scope)
        .map(|u| {
            json!({"id":u.id,"title":u.title,"kind":u.kind,"status":u.status,
            "reason":u.reason,"resultPaths":u.result_paths})
        })
        .collect();
    let done = units.iter().filter(|u| u["status"] == "succeeded").count();
    let issues: Vec<_> = r.issues.iter().filter(|i| i["status"] != "resolved")
        .map(|i| json!({"id":i["id"],"description":i["description"],"handling":i["executionHandling"]})).collect();
    let summary = format!(
        "本轮执行已结束：{} 项完成，{} 项未完成或待验证。问题与成果已汇总；执行结束不代表验收通过。",
        done,
        units.len() - done
    );
    r.workbench.execution.summary =
        json!({"at":now(),"version":r.current_version,"units":units,"issues":issues});
    // The scheduler, rather than the model remembering a final tool call, guarantees closure.
    autonomy::conclude(r, root, &json!({"summary":summary}))
}
fn schedule(r: &mut ProductionRecord, event_id: &str, advance: bool) -> AppResult<()> {
    if r.workbench.goals.is_empty() {
        super::workbench::observe(r, None);
    }
    let scope_hash = autonomy::scope(r);
    let loop_enabled = completion::enabled(r);
    let relevant = completion::relevant(r);
    super::autonomy_budget::account(r);
    let route_ms = super::autonomy_budget::limit(r, "routeMinutes", 15) * 60_000;
    let attempt_limit = super::autonomy_budget::limit(r, "routeAttemptLimit", 2) as u32;
    let exhausted: Vec<_> = r
        .workbench
        .execution
        .units
        .iter()
        .filter(|u| super::autonomy_budget::exhausted(r, u))
        .map(|u| u.id.clone())
        .collect();
    let condition = autonomy::condition(r);
    let conditions = r
        .workbench
        .execution
        .units
        .iter()
        .map(|u| (u.id.clone(), autonomy::unit_condition(r, u)))
        .collect::<BTreeMap<_, _>>();
    let goal = r
        .workbench
        .active_goal_id
        .clone()
        .ok_or_else(|| AppError::validation("缺少当前原子目标。"))?;
    let ledger = &mut r.workbench.execution;
    if ledger.scope_hash != scope_hash {
        ledger.scope_hash = scope_hash.clone();
        ledger.active_unit_id = None;
        ledger.state = "ready".into();
        ledger.last_event.clear();
        ledger.last_progress.clear();
    }
    if ledger.last_event == event_id {
        return Ok(());
    }
    if !ledger
        .units
        .iter()
        .any(|u| u.cycle == r.cycle && u.scope_hash == scope_hash)
    {
        ledger.units.push(WorkUnit {
            scope_hash: scope_hash.clone(),
            id: format!("implementation-{}-{}-bootstrap", r.cycle, &scope_hash[..8]),
            goal_id: goal.clone(),
            cycle: r.cycle,
            title: "拆解已批准计划并建立实现、恢复与验证工作队列".into(),
            routes: vec![
                "按批准计划先登记milestone=core的真实输入、核心循环、保存冷加载与候选包工作；扩展功能标extension。禁止先堆大量抽象端口。写入队列说明文件后报告本项完成。".into(),
                "核对已有实现和失败记录，按可完成范围登记工作项；不等待用户继续。".into(),
            ],
            status: "pending".into(),
            updated_at: now(),
            ..Default::default()
        });
    }
    if advance
        && let Some(unit) = ledger
            .units
            .iter_mut()
            .find(|u| Some(&u.id) == ledger.active_unit_id.as_ref() && u.status == "running")
    {
        unit.status = "failed".into();
        unit.reason = "本轮尚未报告工作项完成，按预算切换工作或恢复路线。".into();
    }
    for unit in ledger.units.iter_mut().filter(|u| {
        (!loop_enabled || relevant.contains(&u.id))
            && u.cycle == r.cycle
            && u.scope_hash == scope_hash
    }) {
        let unit_condition = conditions
            .get(&unit.id)
            .cloned()
            .unwrap_or_else(|| condition.clone());
        let renewed = unit.status == "deferred"
            && unit.condition_hash != unit_condition
            && (!loop_enabled || unit.defer_reason_code == "dependency");
        if renewed {
            unit.status = "pending".into();
            unit.attempts = 0;
            unit.route = 0;
            if !loop_enabled {
                unit.elapsed_ms = 0;
            }
            unit.route_elapsed_ms = 0;
            unit.condition_hash = unit_condition.clone();
        }
        if !renewed
            && matches!(unit.status.as_str(), "pending" | "failed")
            && exhausted.contains(&unit.id)
        {
            unit.status = "deferred".into();
            unit.condition_hash = unit_condition.clone();
            unit.defer_reason_code = "budget-exhausted".into();
            unit.reason = "本项或关联故障的累计恢复预算已用完，保留成果并继续独立工作。".into();
        }
        if unit.status == "failed"
            && (unit.attempts >= attempt_limit || unit.route_elapsed_ms >= route_ms)
        {
            unit.route += 1;
            unit.attempts = 0;
            unit.route_elapsed_ms = 0;
            if unit.route >= unit.routes.len() {
                unit.status = "deferred".into();
                unit.condition_hash = unit_condition.clone();
                unit.reason = "不同执行路线已达到恢复预算；暂缓本项，继续其他工作。".into();
            }
        }
    }
    if !advance
        && ledger
            .units
            .iter()
            .any(|u| Some(&u.id) == ledger.active_unit_id.as_ref() && u.status == "running")
    {
        if let Some(u) = ledger
            .units
            .iter_mut()
            .find(|u| Some(&u.id) == ledger.active_unit_id.as_ref())
        {
            u.attempt_started_at = now();
        }
        ledger.state = "running".into();
        autonomy::event(ledger, "resumed", None, "沿用原工作项恢复。");
        return Ok(());
    }
    if loop_enabled {
        completion::ensure_repair(r);
    }
    let relevant = completion::relevant(r);
    let sig = completion::signature(r);
    let final_id = if loop_enabled {
        completion::final_id(r)
    } else {
        format!("implementation-{}-{}-handoff", r.cycle, &scope_hash[..8])
    };
    let ledger = &mut r.workbench.execution;
    let runnable = ledger.units.iter().any(|u| {
        (!loop_enabled || relevant.contains(&u.id))
            && u.cycle == r.cycle
            && u.scope_hash == scope_hash
            && matches!(u.status.as_str(), "pending" | "failed")
            && u.depends_on.iter().all(|id| {
                ledger
                    .units
                    .iter()
                    .any(|d| &d.id == id && d.status == "succeeded")
            })
    });
    if !runnable
        && ledger
            .units
            .iter()
            .filter(|u| {
                (!loop_enabled || relevant.contains(&u.id))
                    && u.cycle == r.cycle
                    && u.scope_hash == scope_hash
            })
            .all(|u| u.status != "running")
        && !ledger.units.iter().any(|u| {
            u.id == final_id
                || loop_enabled
                    && u.kind == "finalization"
                    && u.goal_id == goal
                    && u.input_signature == sig
        })
    {
        for u in &mut ledger.units {
            if (!loop_enabled || relevant.contains(&u.id))
                && u.cycle == r.cycle
                && u.scope_hash == scope_hash
                && u.status == "pending"
            {
                u.status = "deferred".into();
                u.defer_reason_code = "dependency".into();
                u.reason = "前置工作未完成，保留已有产出供最终核对。".into();
                u.condition_hash = conditions.get(&u.id).cloned().unwrap_or_default();
            }
        }
        ledger.units.push(WorkUnit {input_signature:sig,kind:"finalization".into(),scope_hash:scope_hash.clone(),id:final_id.clone(),goal_id:r.workbench.active_goal_id.clone().unwrap_or_default(),cycle:r.cycle,
            title:"恢复正式候选、核对实际自测并汇总缺口".into(),routes:vec!["先完成测试夹具清理或通过activate-version恢复已验证的冻结候选；2.8没有候选时，优先完成真实输入及核心循环接入，捕获candidate并核对lua/input/core-loop/save-reload证据，调用build-verification-package。需要继续实现时登记具体工作项后报告本项暂缓，由APP继续派发；不得仅写缺口清单结束。核对已完成的真实回执并登记版本适用的必要自测，禁止重放已完成测试。满足原门槛再complete-stage；缺口如实保留，能完成的版本核对与材料登记不能因其他项暂缓而省略。".into(),"核对已有产出、候选与未测项，保存汇总并conclude-implementation，不伪造通过。".into()],status:"pending".into(),updated_at:now(),..Default::default()});
    }
    let next = ledger
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| {
            (!loop_enabled || relevant.contains(&u.id) || u.id == final_id)
                && u.cycle == r.cycle
                && u.scope_hash == scope_hash
                && matches!(u.status.as_str(), "pending" | "failed")
                && u.depends_on.iter().all(|id| {
                    ledger
                        .units
                        .iter()
                        .any(|d| d.id == *id && d.status == "succeeded")
                })
        })
        .min_by_key(|(_, u)| {
            if loop_enabled && r.current_version.is_none() {
                match (u.kind.as_str(), u.milestone.as_str()) {
                    ("repair-plan", _) | (_, "core") => 0,
                    (_, "extension") => 2,
                    ("finalization", _) => 3,
                    _ => 1,
                }
            } else {
                usize::from(u.status == "failed")
            }
        })
        .map(|(i, _)| i);
    ledger.last_event = event_id.into();
    if let Some(index) = next {
        let unit = &mut ledger.units[index];
        unit.status = "running".into();
        unit.attempt_started_at = now();
        unit.attempts += 1;
        if unit.attempts == 1 {
            unit.started_at = now();
        }
        unit.updated_at = now();
        let id = unit.id.clone();
        let reason = unit.routes[unit.route].clone();
        if !loop_enabled {
            r.workbench.active_goal_id = Some(unit.goal_id.clone());
        }
        ledger.active_unit_id = Some(id.clone());
        ledger.state = "running".into();
        autonomy::event(ledger, "scheduled", Some(&id), &reason);
    } else {
        ledger.active_unit_id = None;
        ledger.parked_condition = condition.clone();
        ledger.state = if ledger
            .units
            .iter()
            .filter(|u| {
                (!loop_enabled || relevant.contains(&u.id))
                    && u.cycle == r.cycle
                    && u.scope_hash == scope_hash
            })
            .all(|u| u.status == "succeeded")
        {
            "settled"
        } else {
            "waitingDependencies"
        }
        .into();
        autonomy::event(
            ledger,
            "parked",
            None,
            "当前可执行工作已收敛；保留未完成项，不要求反复点击继续，也不宣称验收通过。",
        );
    }
    r.workbench.execution.last_progress = autonomy::progress(r);
    Ok(())
}
impl TaskService {
    pub fn production_execution_wakeable(&self, task: &str) -> AppResult<Option<u64>> {
        let t = self.get_readonly(task)?;
        if t.status != TaskStatus::Active {
            return Ok(None);
        }
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(None);
        };
        let l = &r.workbench.execution;
        Ok((autonomy::autonomous_implementation(&r)
            && matches!(
                l.state.as_str(),
                "waitingDependencies"
                    | "settledWithGaps"
                    | "repairRequired"
                    | "externalBlocked"
                    | "ready"
            )
            && (l.state == "ready" || l.parked_condition != autonomy::condition(&r)))
        .then_some(r.revision))
    }
    pub fn production_execution_pause(&self, task: &str) -> AppResult<()> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(());
        };
        if super::preflight_recovery::preflight_recovery_active(&r) {
            return self.production_commit(task, r.revision, |r| {
                super::preflight_recovery::pause(r);
                Ok(())
            });
        }
        if !autonomy::autonomous_implementation(&r)
            || !matches!(r.workbench.execution.state.as_str(), "running" | "ready")
        {
            return Ok(());
        }
        self.production_commit(task, r.revision, |r| {
            super::autonomy_budget::account(r);
            r.workbench.execution.state = "paused".into();
            autonomy::event(
                &mut r.workbench.execution,
                "paused",
                None,
                "执行控制已暂停；不会因轮询自动恢复。",
            );
            Ok(())
        })
    }
}
impl TaskService {
    pub fn production_prepare_goal(&self, task: &str, goal: Option<&str>) -> AppResult<()> {
        let Some(goal) = goal else {
            return Ok(());
        };
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        if !autonomy::autonomous_implementation(&r) {
            return Ok(());
        }
        let scope_hash = autonomy::scope(&r);
        if r.workbench
            .execution
            .units
            .iter()
            .any(|u| u.goal_id == goal && u.scope_hash == scope_hash)
        {
            return Ok(());
        }
        let root = self.production_workspace(task)?;
        use sha2::{Digest, Sha256};
        let id = format!(
            "goal-bootstrap-{:x}",
            Sha256::digest(format!("{scope_hash}:{goal}").as_bytes())
        );
        self.production_commit(task,r.revision,|r|autonomy::apply(r,&root,"save-work-unit",&json!({
            "id":id,"goalId":goal,"title":"核对本次用户目标与已批准方案，拆解剩余工作",
            "dependsOn":[],"issueIds":[],"routes":["读取 production current 的用户请求与已有成果，登记对应工作项；补充说明不得被忽略，范围变化仍保留必要决定。","核对原请求与差异，保留不能直接实施的决定，继续可完成范围。"]})))
    }
}
