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
        Ok(
            json!({"taskId":task,"revision":r.revision,"cycle":r.cycle,"stage":r.current_stage,
            "version":r.current_version,"autonomous":autonomy::autonomous_implementation(&r),
            "requests":r.workbench.requests.iter().rev().filter(|q|q["status"]!="completed"&&q["stage"]==r.current_stage).take(12).map(|q|json!({"id":q["id"],"revision":q["revision"],"status":q["status"],"text":q["text"].as_str().unwrap_or("").chars().take(1000).collect::<String>()})).collect::<Vec<_>>(),
            "documentRefs":r.documents.iter().map(|(kind,d)|json!({"kind":kind,"path":d.path,"sha256":d.sha256,"revision":d.revision})).collect::<Vec<_>>(),
            "executionTiming":super::validation_runs::timing(&r),"validationImpact":{"mode":r.version_details["validationImpact"]["mode"],"unknownImpact":r.version_details["validationImpact"]["unknownImpact"],"mustRetest":r.version_details["validationImpact"]["mustRetest"],"changedFileCount":r.version_details["validationImpact"]["changedFiles"].as_array().map_or(0,Vec::len)},"executionState":ledger.state,"activeGoalId":r.workbench.active_goal_id,
            "activeUnit":ledger.units.iter().find(|u|Some(&u.id)==ledger.active_unit_id.as_ref()),
            "workUnitCount":ledger.units.iter().filter(|u|u.cycle==r.cycle&&u.scope_hash==scope_hash).count(),
            "workUnits":ledger.units.iter().filter(|u|u.cycle==r.cycle&&u.scope_hash==scope_hash).take(32).map(|u|json!({"id":u.id,"goalId":u.goal_id,"title":u.title,"status":u.status,"dependsOn":u.depends_on})).collect::<Vec<_>>(),
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
                || (r.workbench.execution.state == "settledWithGaps"
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
            let result =
                self.production_commit(task, r.revision, |r| schedule(r, event_id, advance));
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
fn schedule(r: &mut ProductionRecord, event_id: &str, advance: bool) -> AppResult<()> {
    if r.workbench.goals.is_empty() {
        super::workbench::observe(r, None);
    }
    let scope_hash = autonomy::scope(r);
    let progress = autonomy::progress(r);
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
            goal_id: goal,
            cycle: r.cycle,
            title: "拆解已批准计划并建立实现、恢复与验证工作队列".into(),
            routes: vec![
                "按批准计划登记有依赖的工作项，写入队列说明文件后报告本项完成。".into(),
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
        let elapsed = chrono::DateTime::parse_from_rfc3339(&unit.started_at)
            .ok()
            .map(|t| Utc::now().signed_duration_since(t).num_minutes())
            .unwrap_or(0);
        if progress != ledger.last_progress && elapsed < 15 {
            unit.attempts = 0;
            unit.status = "pending".into();
        } else {
            unit.status = "failed".into();
            unit.reason = "本轮未产生可核对的结果，自动切换工作或恢复路线。".into();
        }
    }
    for unit in ledger
        .units
        .iter_mut()
        .filter(|u| u.cycle == r.cycle && u.scope_hash == scope_hash)
    {
        let unit_condition = conditions
            .get(&unit.id)
            .cloned()
            .unwrap_or_else(|| condition.clone());
        if unit.status == "deferred" && unit.condition_hash != unit_condition {
            unit.status = "pending".into();
            unit.attempts = 0;
            unit.route = 0;
            unit.condition_hash = unit_condition.clone();
        }
        if unit.status == "failed" && unit.attempts >= 2 {
            unit.route += 1;
            unit.attempts = 0;
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
        ledger.state = "running".into();
        autonomy::event(ledger, "resumed", None, "沿用原工作项恢复。");
        return Ok(());
    }
    let final_id = format!("implementation-{}-{}-handoff", r.cycle, &scope_hash[..8]);
    if ledger
        .units
        .iter()
        .filter(|u| u.cycle == r.cycle && u.scope_hash == scope_hash)
        .all(|u| u.status == "succeeded")
        && !ledger.units.iter().any(|u| u.id == final_id)
    {
        ledger.units.push(WorkUnit {scope_hash:scope_hash.clone(),id:final_id,goal_id:r.workbench.active_goal_id.clone().unwrap_or_default(),cycle:r.cycle,
            title:"统一自测、证据与交付缺口收敛".into(),routes:vec!["核对原门槛，通过才 complete-stage；未通过自行修复或 conclude-implementation 如实保留缺口。".into(),"核对全部产出与未完成项，提交真实收敛摘要，禁止伪造通过。".into()],status:"pending".into(),updated_at:now(),..Default::default()});
    }
    let next = ledger
        .units
        .iter()
        .position(|u| {
            u.cycle == r.cycle
                && u.scope_hash == scope_hash
                && u.status == "pending"
                && u.depends_on.iter().all(|id| {
                    ledger
                        .units
                        .iter()
                        .any(|d| d.id == *id && d.status == "succeeded")
                })
        })
        .or_else(|| {
            ledger.units.iter().position(|u| {
                u.cycle == r.cycle
                    && u.scope_hash == scope_hash
                    && u.status == "failed"
                    && u.depends_on.iter().all(|id| {
                        ledger
                            .units
                            .iter()
                            .any(|d| d.id == *id && d.status == "succeeded")
                    })
            })
        });
    ledger.last_event = event_id.into();
    if let Some(index) = next {
        let unit = &mut ledger.units[index];
        unit.status = "running".into();
        unit.attempts += 1;
        if unit.attempts == 1 {
            unit.started_at = now();
        }
        unit.updated_at = now();
        let id = unit.id.clone();
        let reason = unit.routes[unit.route].clone();
        r.workbench.active_goal_id = Some(unit.goal_id.clone());
        ledger.active_unit_id = Some(id.clone());
        ledger.state = "running".into();
        autonomy::event(ledger, "scheduled", Some(&id), &reason);
    } else {
        ledger.active_unit_id = None;
        ledger.parked_condition = condition.clone();
        ledger.state = if ledger
            .units
            .iter()
            .filter(|u| u.cycle == r.cycle && u.scope_hash == scope_hash)
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
                "waitingDependencies" | "settledWithGaps" | "ready"
            )
            && (l.state == "ready" || l.parked_condition != autonomy::condition(&r)))
        .then_some(r.revision))
    }
    pub fn production_execution_pause(&self, task: &str) -> AppResult<()> {
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(());
        };
        if !autonomy::autonomous_implementation(&r) || r.workbench.execution.state == "paused" {
            return Ok(());
        }
        self.production_commit(task, r.revision, |r| {
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
