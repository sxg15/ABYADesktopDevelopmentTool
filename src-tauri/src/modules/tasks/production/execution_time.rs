use super::*;

/// 结束执行由验收与真实外部条件决定，耗时只用于统计。
pub(super) fn unlimited(r: &ProductionRecord) -> bool {
    r.policy["executionTimePolicy"] == "unlimited"
}

impl TaskService {
    /// 用户打开或恢复旧任务前迁移时间策略，不改变其他阶段契约或已固定 Skill。
    pub(in crate::modules::tasks) fn production_remove_execution_deadlines(
        &self,
        task: &str,
    ) -> AppResult<()> {
        if crate::foundation::cli_sessions::task_active(task)
            || self.get_readonly(task)?.status != TaskStatus::Active
        {
            return Ok(());
        }
        let Some(r) = self.database.with_connection(|c| load(c, task))? else {
            return Ok(());
        };
        if unlimited(&r) {
            return Ok(());
        }
        self.production_commit(task, r.revision, |record| {
            record.policy["executionTimePolicy"] = json!("unlimited");
            migrate(record);
            Ok(())
        })
    }
}

/// 升级仅撤销时间耗尽造成的暂缓，保留暂停、对账及所有历史成果。
pub(super) fn migrate(r: &mut ProductionRecord) {
    if !unlimited(r) {
        return;
    }
    let scope = autonomy::scope(r);
    for u in &mut r.workbench.execution.units {
        if u.cycle != r.cycle || u.scope_hash != scope {
            continue;
        }
        u.execution_limit_ms = 0;
        if u.status == "running" {
            u.status = "pending".into();
            u.attempt_started_at.clear();
            u.recovery_strategy = "先核对中断前操作回执与实际成果，只继续未完成动作。".into();
        }
        if matches!(u.status.as_str(), "deferred" | "failed")
            && (u.defer_reason_code == "budget-exhausted" || u.stop_reason == "execution-budget")
            && !matches!(
                u.defer_reason_code.as_str(),
                "unknown-result" | "external-condition"
            )
        {
            u.definition_history
                .push(json!({"at":now(),"kind":"time-limit-removed",
                "status":u.status,"reason":u.reason,"elapsedMs":u.elapsed_ms}));
            u.status = "pending".into();
            u.defer_reason_code.clear();
            u.stop_reason = "time-limit-removed".into();
            u.attempt_started_at.clear();
            u.reason = "执行不限时；先核对已有成果和回执，继续未完成工作。".into();
            if let Some(item) = r.workbench.execution.technical_queue.get_mut(&u.id)
                && item["reason"] == "execution-budget"
            {
                item["status"] = json!("resolved");
                item["resolvedAt"] = json!(now());
            }
        }
    }
    if r.workbench.execution.preflight.state == "exhausted" {
        r.workbench.execution.preflight.state = "paused".into();
        r.workbench.execution.preflight.started_at.clear();
    }
    r.workbench.execution.parked_condition.clear();
    // 升级不发起原生轮次；由既有用户恢复入口继续原目标。
    r.workbench.execution.state = "paused".into();
    r.workbench.execution.active_unit_id = None;
    autonomy::event(
        &mut r.workbench.execution,
        "execution-time-unlimited",
        None,
        "取消执行期限，保留累计耗时、批准、成果和未知写入；未自动恢复任务。",
    );
}
