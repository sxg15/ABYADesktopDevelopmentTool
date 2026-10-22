use super::*;

impl TaskService {
    pub fn production_attempt_runtime(
        &self,
        task: &str,
        id: &str,
        runtime: &WorkbenchAttempt,
    ) -> AppResult<()> {
        let r = self.production_get(task)?.record.ok_or_else(conflict)?;
        self.production_commit(task, r.revision, |r| {
            let a = r
                .workbench
                .attempts
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or_else(conflict)?;
            if a.conversation_id != runtime.conversation_id
                || a.native_id != runtime.native_id
                || !matches!(a.status.as_str(), "queued" | "sending")
            {
                return Err(conflict());
            }
            a.backend_pid = runtime.backend_pid;
            a.backend_created = runtime.backend_created;
            a.generation = runtime.generation;
            a.process_epoch = runtime.process_epoch.clone();
            a.owner_pid = runtime.owner_pid;
            a.owner_created = runtime.owner_created;
            Ok(())
        })
    }
    pub fn production_attempt(&self, task: &str, id: &str) -> AppResult<Option<WorkbenchAttempt>> {
        self.get(task)?;
        self.database.with_connection(|c| {
            Ok(load(c, task)?.and_then(|r| r.workbench.attempts.into_iter().find(|a| a.id == id)))
        })
    }
    pub fn production_begin_attempt(
        &self,
        task: &str,
        expected: Option<u64>,
        mut attempt: WorkbenchAttempt,
    ) -> AppResult<WorkbenchAttempt> {
        if let Some(old) = self.production_attempt(task, &attempt.id)? {
            if old.goal_id != attempt.goal_id
                || old.conversation_id != attempt.conversation_id
                || old.native_id != attempt.native_id
            {
                return Err(conflict());
            }
            return Ok(old);
        }
        let revision = expected.unwrap_or(
            self.production_get(task)?
                .record
                .ok_or_else(|| AppError::validation("尚未启用制作流程。"))?
                .revision,
        );
        self.production_commit(task, revision, |r| {
            if r.workbench.goals.is_empty() {
                workbench::observe(r, None);
            }
            let goal = r
                .workbench
                .goals
                .iter()
                .find(|g| {
                    g.id == attempt.goal_id && g.stage == r.current_stage && g.cycle == r.cycle
                })
                .ok_or_else(|| AppError::validation("子流程不属于当前阶段或周期。"))?;
            if matches!(
                goal.status.as_str(),
                "completed" | "accepted" | "resolved" | "superseded"
            ) {
                return Err(AppError::validation("此子流程已经结束，不能恢复为执行中。"));
            }
            if r.workbench.attempts.iter().any(|a| {
                matches!(
                    a.status.as_str(),
                    "running" | "sending" | "queued" | "reconciling" | "needsReview"
                )
            }) {
                return Err(AppError::validation(
                    "已有执行或待核对请求，请先核对该执行，不能启动第二份。",
                ));
            }
            if r.workbench.attempts.len() >= 2000 {
                return Err(AppError::validation("执行历史已达到容量上限。"));
            }
            attempt.request_id = (goal.source_type == "request").then(|| goal.source_id.clone());
            attempt.predecessor_id = r
                .workbench
                .attempts
                .iter()
                .rev()
                .find(|a| a.goal_id == attempt.goal_id)
                .map(|a| a.id.clone());
            attempt.started_at = now();
            attempt.updated_at = attempt.started_at.clone();
            r.workbench.active_goal_id = Some(attempt.goal_id.clone());
            r.workbench.attempts.push(attempt.clone());
            Ok(())
        })?;
        Ok(attempt)
    }
    // Execution-only writes never approve, close an issue or complete a business goal.
    pub fn production_attempt_status(
        &self,
        task: &str,
        id: &str,
        status: &str,
        turn: Option<&str>,
        reason: &str,
    ) -> AppResult<()> {
        if ![
            "queued",
            "sending",
            "running",
            "finished",
            "paused",
            "interrupted",
            "needsReview",
            "failed",
            "reconciling",
        ]
        .contains(&status)
        {
            return Err(AppError::validation("无效执行状态。"));
        }
        for _ in 0..3 {
            let Some(r) = self.database.with_connection(|c| load(c, task))? else {
                return Ok(());
            };
            let Some(old) = r.workbench.attempts.iter().find(|a| a.id == id) else {
                return Ok(());
            };
            if old.status == status && (turn.is_none() || old.turn_id.as_deref() == turn) {
                return Ok(());
            }
            if matches!(
                old.status.as_str(),
                "finished" | "paused" | "interrupted" | "failed"
            ) && old.status != status
            {
                return Ok(());
            }
            let result = self.production_commit(task, r.revision, |r| {
                let a = r
                    .workbench
                    .attempts
                    .iter_mut()
                    .find(|a| a.id == id)
                    .ok_or_else(conflict)?;
                if a.turn_id.is_some() && turn.is_some() && a.turn_id.as_deref() != turn {
                    return Err(conflict());
                }
                if let Some(turn) = turn {
                    a.turn_id = Some(turn.into());
                }
                a.status = status.into();
                a.reason = reason.chars().take(1000).collect();
                a.updated_at = now();
                Ok(())
            });
            match result {
                Err(e) if e.code == "conflict" => continue,
                other => return other,
            }
        }
        Err(conflict())
    }
}
