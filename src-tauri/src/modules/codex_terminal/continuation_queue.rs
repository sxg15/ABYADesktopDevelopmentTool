use super::intake_receipt::{self, IntakeContinueStatus};
use super::*;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContinueRequest {
    #[serde(default)]
    pub repair: Option<super::issue_repair::RepairIntent>,
    pub id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub native_id: String,
    pub group_id: Option<String>,
    pub answer_revision: u64,
    #[serde(default)]
    pub after_revision: Option<u64>,
    #[serde(default)]
    pub automatic_after: Option<String>,
    #[serde(default)]
    pub goal_id: Option<String>,
    #[serde(default)]
    pub attempt_id: Option<String>,
    pub status: String,
    pub owner: String,
    pub revision: u64,
    pub updated_at: String,
    pub after_turn: Option<String>,
    pub turn_id: Option<String>,
    pub error: Option<String>,
}

impl ContinueRequest {
    pub(super) fn response(&self, status: IntakeContinueStatus) -> IntakeContinuation {
        IntakeContinuation {
            status,
            conversation_id: self.conversation_id.clone(),
            native_session_id: self.native_id.clone(),
            turn_id: self.turn_id.clone(),
            request_id: Some(self.id.clone()),
        }
    }
}

pub(super) fn read(directory: &Path) -> AppResult<Option<ContinueRequest>> {
    let path = directory.join("continuation.json");
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&std::fs::read(path)?)?))
}
pub(super) fn save(directory: &Path, request: &mut ContinueRequest) -> AppResult<()> {
    request.revision += 1;
    request.updated_at = Utc::now().to_rfc3339();
    let path = directory.join("continuation.json");
    let temp = directory.join(format!("continuation-{}.tmp", Uuid::new_v4()));
    intake_receipt::save(&temp, &serde_json::to_value(&request)?, true)?;
    std::fs::rename(temp, path)?;
    if let Some(repair) = &request.repair {
        let receipt = super::issue_repair::receipt_path(directory, &repair.fingerprint)?;
        intake_receipt::save(
            &receipt,
            &serde_json::to_value(&request)?,
            !receipt.exists(),
        )?;
    }
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("continuation-events.jsonl"))?;
    writeln!(log, "{}", serde_json::to_string(request)?)?;
    Ok(())
}

pub(super) fn waiting(
    record: Option<&crate::modules::tasks::ProductionView>,
) -> Option<IntakeContinueStatus> {
    let r = record?.record.as_ref()?;
    if r.stages.values().any(|s| s == "awaiting-confirmation") {
        return Some(IntakeContinueStatus::WaitingForApproval);
    }
    if r.question_groups.iter().any(|g| g.status == "pending") {
        return Some(IntakeContinueStatus::WaitingForAnswers);
    }
    None
}

impl CodexTerminalService {
    pub fn continue_after_revision(
        &self,
        task: &str,
        conversation: &str,
        revision: Option<u64>,
    ) -> AppResult<IntakeContinuation> {
        self.request_continuation(task, conversation, None, 0, revision)
    }

    pub(super) fn request_continuation(
        &self,
        task: &str,
        conversation: &str,
        group: Option<&str>,
        revision: u64,
        after_revision: Option<u64>,
    ) -> AppResult<IntakeContinuation> {
        let gate = self.gate(conversation);
        let _guard = gate
            .normal
            .try_lock_for(Duration::from_secs(2))
            .ok_or_else(|| {
                AppError::new(
                    "codex_control_busy",
                    "当前会话正在处理，请稍后重试；其他任务可继续使用。",
                    "not_executed",
                )
            })?;
        let generation = gate.epoch();
        if self.stopping.lock().contains(conversation) {
            return Err(AppError::validation("正在暂停，请等待暂停完成。"));
        }
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(AppError::validation("APP 正在退出，未启动执行。"));
        }
        let metadata = self.conversation(task, conversation)?;
        if metadata.archived {
            return Err(AppError::validation("原会话已归档，请先恢复。"));
        }
        let native = metadata
            .native_session_id
            .ok_or_else(|| AppError::validation("请先连接原会话。"))?;
        let task_record = self.tasks.get(task)?;
        if task_record.status != crate::modules::tasks::TaskStatus::Active {
            return Err(AppError::validation("仅进行中的任务可以继续。"));
        }
        let directory =
            conversation_directory(Path::new(&task_record.workspace_path), conversation);
        self.queue_tasks
            .lock()
            .insert(conversation.into(), task.into());
        let previous = read(&directory)?;
        if previous.as_ref().is_some_and(|q| {
            q.task_id != task || q.conversation_id != conversation || q.native_id != native
        }) {
            return Err(AppError::validation(
                "待处理请求与当前任务会话不匹配，请核对记录。",
            ));
        }
        if previous.as_ref().is_some_and(|q| {
            q.repair.is_some()
                && matches!(
                    q.status.as_str(),
                    "queued" | "needsConnection" | "sending" | "needsReview"
                )
        }) && (group.is_some() || after_revision.is_some())
        {
            return Err(AppError::validation(
                "已有问题修复请求等待处理，当前决定已保存；请先核对原会话中的修复请求。",
            ));
        }
        if let Some(old) = previous
            .as_ref()
            .filter(|q| q.status == "sending" || q.status == "needsReview")
        {
            return Ok(old.response(IntakeContinueStatus::NeedsReview));
        }
        let mut request = if let Some(old) = previous.as_ref().filter(|q| {
            (q.status == "queued" || q.status == "needsConnection")
                && q.owner == self.process_epoch
                && ((group.is_none() && after_revision.is_none())
                    || (q.group_id.as_deref() == group
                        && q.answer_revision == revision
                        && q.after_revision == after_revision))
        }) {
            old.clone()
        } else {
            ContinueRequest {
                repair: if group.is_none() && after_revision.is_none() {
                    previous
                        .as_ref()
                        .filter(|q| {
                            matches!(
                                q.status.as_str(),
                                "paused" | "queued" | "needsConnection" | "needsResume"
                            )
                        })
                        .and_then(|q| q.repair.clone())
                } else {
                    None
                },
                id: Uuid::new_v4().to_string(),
                task_id: task.into(),
                conversation_id: conversation.into(),
                native_id: native,
                group_id: group.map(str::to_owned).or_else(|| {
                    if after_revision.is_some() {
                        return None;
                    }
                    previous
                        .as_ref()
                        .filter(|q| {
                            q.status == "paused"
                                || q.status == "queued"
                                || q.status == "needsConnection"
                        })
                        .and_then(|q| q.group_id.clone())
                }),
                answer_revision: if group.is_some() {
                    revision
                } else if after_revision.is_some() {
                    0
                } else {
                    previous.as_ref().map_or(0, |q| q.answer_revision)
                },
                after_revision,
                automatic_after: None,
                goal_id: self.tasks.production_get(task)?.record.and_then(|r| {
                    r.workbench
                        .requests
                        .iter()
                        .rev()
                        .find(|v| {
                            v["revision"].as_u64() == after_revision && after_revision.is_some()
                        })
                        .map(|v| format!("request:{}", v["id"].as_str().unwrap_or("")))
                        .or(r.workbench.active_goal_id)
                }),
                attempt_id: None,
                status: "queued".into(),
                owner: self.process_epoch.clone(),
                revision: previous.as_ref().map_or(0, |q| q.revision),
                updated_at: String::new(),
                after_turn: None,
                turn_id: None,
                error: None,
            }
        };
        {
            let _publication = gate.publication.lock();
            gate.check(generation)?;
            self.cancelled.lock().remove(conversation);
        }
        self.auto_progress.lock().remove(conversation);
        save(&directory, &mut request)?;
        self.dispatch_or_record(&directory, &mut request)
    }

    pub fn flush_continuation(
        &self,
        task: &str,
        conversation: &str,
        request_id: &str,
    ) -> AppResult<IntakeContinuation> {
        let gate = self.gate(conversation);
        let _guard = gate
            .normal
            .try_lock_for(Duration::from_secs(2))
            .ok_or_else(|| {
                AppError::new(
                    "codex_control_busy",
                    "当前会话正在处理，请稍后重试；其他任务可继续使用。",
                    "not_executed",
                )
            })?;
        let t = self.tasks.get(task)?;
        self.conversation(task, conversation)?;
        let directory = conversation_directory(Path::new(&t.workspace_path), conversation);
        let mut request =
            read(&directory)?.ok_or_else(|| AppError::validation("没有待续接请求。"))?;
        if request.task_id != task || request.conversation_id != conversation {
            return Err(AppError::validation(
                "待处理请求与当前任务会话不匹配，请核对记录。",
            ));
        }
        if request.id != request_id {
            return Err(AppError::validation("继续请求已更新，请读取最新状态。"));
        }
        self.dispatch_or_record(&directory, &mut request)
    }

    pub(super) fn dispatch_or_record(
        &self,
        directory: &Path,
        request: &mut ContinueRequest,
    ) -> AppResult<IntakeContinuation> {
        let result = self.dispatch_locked(directory, request);
        if let Err(error) = &result
            && (request.status == "queued" || request.status == "needsConnection")
        {
            request.status = "needsResume".into();
            request.error = Some(error.message.clone());
            save(directory, request)?;
        }
        result
    }

    fn dispatch_locked(
        &self,
        directory: &Path,
        request: &mut ContinueRequest,
    ) -> AppResult<IntakeContinuation> {
        use IntakeContinueStatus as S;
        let gate = self.gate(&request.conversation_id);
        let generation = gate.epoch();
        Uuid::parse_str(&request.id)
            .map_err(|_| AppError::validation("继续请求标识无效，请核对记录。"))?;
        if self.shutting_down.load(Ordering::SeqCst)
            || self.cancelled.lock().contains(&request.conversation_id)
            || request.owner != self.process_epoch
            || request.status == "paused"
        {
            return Ok(request.response(S::Paused));
        }
        if request.status == "sending" || request.status == "needsReview" {
            return Ok(request.response(S::NeedsReview));
        }
        if request.status != "queued" && request.status != "needsConnection" {
            if request.status == "sent" || request.status == "running" {
                let workflow = self.workflow(&request.task_id, &request.conversation_id)?;
                let turn = workflow
                    .turns
                    .iter()
                    .find(|t| Some(&t.id) == request.turn_id.as_ref());
                let status = match turn.map(|t| &t.status) {
                    Some(CodexWorkflowTurnStatus::Completed) => S::Completed,
                    Some(
                        CodexWorkflowTurnStatus::Interrupted | CodexWorkflowTurnStatus::Failed,
                    ) => S::Paused,
                    _ => S::Running,
                };
                return Ok(request.response(status));
            }
            return Ok(request.response(S::Completed));
        }
        let metadata = self.conversation(&request.task_id, &request.conversation_id)?;
        if self.tasks.get(&request.task_id)?.status != crate::modules::tasks::TaskStatus::Active {
            return Err(AppError::validation("任务已结束，未自动继续。"));
        }
        if metadata.archived || metadata.native_session_id.as_deref() != Some(&request.native_id) {
            return Err(AppError::validation("原生会话关联已变化，未自动继续。"));
        }
        let production = self.tasks.production_get(&request.task_id)?;
        if request.after_revision.is_some_and(|revision| {
            production
                .record
                .as_ref()
                .is_none_or(|r| r.revision < revision)
        }) {
            return Err(AppError::validation("流程记录版本尚未匹配，未自动继续。"));
        }
        if let Some(status) = waiting(Some(&production)) {
            request.status = if request.repair.is_some() {
                request.error = Some("修复已暂停，请完成人工待办后重新请求 AI 修复。".into());
                "paused"
            } else if status == S::WaitingForAnswers {
                "waitingForAnswers"
            } else {
                "waitingForApproval"
            }
            .into();
            save(directory, request)?;
            return Ok(request.response(status));
        }
        let repair_spec = if let Some(repair) = &request.repair {
            match self
                .tasks
                .prepare_issue_repair(&request.task_id, &repair.issue_id, None)
            {
                Ok(spec) if spec.fingerprint == repair.fingerprint => Some(spec),
                _ => {
                    request.status = "superseded".into();
                    save(directory, request)?;
                    return Ok(request.response(S::Superseded));
                }
            }
        } else {
            None
        };
        if let Some(group) = request.group_id.as_ref() {
            let valid = production.record.as_ref().is_some_and(|r| {
                r.question_groups.iter().any(|g| {
                    &g.id == group
                        && g.status == "submitted"
                        && g.provider == "codex"
                        && g.conversation_id == request.conversation_id
                        && g.native_session_id.as_deref() == Some(&request.native_id)
                        && g.answers
                            .last()
                            .is_some_and(|a| a.revision == request.answer_revision)
                })
            });
            if !valid {
                request.status = "superseded".into();
                save(directory, request)?;
                return Ok(request.response(S::Superseded));
            }
        }
        let runtime = self.runtime_handle(&request.conversation_id);
        let Some(server) = runtime.filter(|s| s.is_alive()) else {
            request.status = "needsConnection".into();
            save(directory, request)?;
            return Ok(request.response(S::NeedsConnection));
        };
        let snapshot = server.request(
            "thread/read",
            json!({"threadId":request.native_id,"includeTurns":true}),
        )?;
        if snapshot["thread"]["id"].as_str() != Some(&request.native_id) {
            return Err(AppError::validation("无法核对原生会话身份。"));
        }
        self.reconcile_snapshot(
            &request.task_id,
            &request.conversation_id,
            &request.native_id,
            &snapshot,
        )?;
        if let Some(expected) = &request.automatic_after {
            let latest = snapshot["thread"]["turns"]
                .as_array()
                .and_then(|turns| turns.last());
            if !super::auto_progress::eligible(&production)
                || latest.is_none_or(|t| {
                    t["id"] != *expected
                        || (t["status"] != "completed"
                            && !(production.record.as_ref().is_some_and(|r| {
                                crate::modules::tasks::autonomous_implementation(r)
                                    || crate::modules::tasks::preflight_recovery_active(r)
                            }) && (t["status"] == "failed"
                                || t["status"] == "interrupted"
                                    && self
                                        .auto_progress
                                        .lock()
                                        .get(&request.conversation_id)
                                        .is_some_and(|r| {
                                            r.turn == *expected && r.budget_interrupt
                                        }))))
                })
            {
                request.status = "superseded".into();
                save(directory, request)?;
                return Ok(request.response(S::Superseded));
            }
        }
        if let Some(group) = request.group_id.as_ref() {
            let path = directory.join(format!("intake-{}-{}.json", group, request.answer_revision));
            if path.exists() {
                let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
                match intake_receipt::decide(Some(&saved), &request.native_id, Some(&snapshot)) {
                    Ok(intake_receipt::ReceiptDecision::Existing(status)) => {
                        request.status = if status == S::Running {
                            "running"
                        } else {
                            "settled"
                        }
                        .into();
                        request.turn_id = saved["turnId"].as_str().map(str::to_owned);
                        save(directory, request)?;
                        return Ok(request.response(status));
                    }
                    Err(error) => {
                        request.status = "needsReview".into();
                        request.error = Some(error.message.clone());
                        save(directory, request)?;
                        return Err(error);
                    }
                    _ => {}
                }
            }
        }
        if let Some(turn) = snapshot["thread"]["turns"]
            .as_array()
            .and_then(|turns| turns.last())
            .filter(|t| t["status"] == "inProgress")
        {
            if request.group_id.is_none()
                && request.after_revision.is_none()
                && request.repair.is_none()
            {
                request.status = "running".into();
                save(directory, request)?;
                return Ok(request.response(S::Running));
            }
            request.status = "queued".into();
            request.after_turn = turn["id"].as_str().map(str::to_owned);
            save(directory, request)?;
            return Ok(request.response(S::Queued));
        }
        if self.session(&request.conversation_id).is_err() {
            request.status = "needsConnection".into();
            save(directory, request)?;
            return Ok(request.response(S::NeedsConnection));
        }
        gate.check(generation)?;
        if self.cancelled.lock().contains(&request.conversation_id) {
            return Ok(request.response(S::Paused));
        }
        let autonomous = production
            .record
            .as_ref()
            .is_some_and(crate::modules::tasks::autonomous_implementation);
        let unit = if autonomous {
            self.tasks
                .production_prepare_goal(&request.task_id, request.goal_id.as_deref())?;
            self.tasks.production_schedule(
                &request.task_id,
                &format!("dispatch:{}", request.id),
                false,
            )?
        } else {
            None
        };
        if autonomous && unit.is_none() {
            if let Some(id) = &request.attempt_id {
                self.tasks.production_attempt_status(
                    &request.task_id,
                    id,
                    "paused",
                    None,
                    "当前无可派发工作；原目标与结果保留。",
                )?;
            }
            request.status = "settled".into();
            save(directory, request)?;
            return Ok(request.response(S::WorkSettled));
        }
        if let Some(unit) = &unit {
            request.goal_id = production
                .record
                .as_ref()
                .filter(|r| r.policy["implementationExecution"]["completionLoopVersion"] == 1)
                .and_then(|r| r.workbench.active_goal_id.clone())
                .or_else(|| Some(unit.goal_id.clone()));
        }
        let selected_goal = request.goal_id.clone().or_else(|| {
            production
                .record
                .as_ref()
                .and_then(|r| r.workbench.active_goal_id.clone())
        });
        if let Some(goal) = selected_goal {
            let template = self.attempt_template(
                &request.id,
                &goal,
                &request.conversation_id,
                &request.native_id,
            );
            self.tasks
                .production_begin_attempt(&request.task_id, None, template.clone())?;
            self.tasks
                .production_attempt_runtime(&request.task_id, &request.id, &template)?;
            self.tasks.production_attempt_status(
                &request.task_id,
                &request.id,
                "sending",
                None,
                "",
            )?;
            request.attempt_id = Some(request.id.clone());
            request.goal_id = Some(goal);
        }
        request.status = "sending".into();
        save(directory, request)?;
        let stage_contract = production.record.as_ref().filter(|r|r.policy["stageContractsVersion"]==1)
            .map(|r|format!("当前阶段 {}，流程 {}。resources/plan 交付可审核设计或计划，技术疑点记录后续待验证，不提前绑定Lua/架构验证或启动Player。隔离运行小样仅使用APP当前有效授权。先读取 production current 和当前目标已有成果。",r.current_stage,r.workflow_version)).unwrap_or_default();
        let goal_prompt=request.goal_id.as_ref().map(|goal|format!("{stage_contract}继续原子目标 {goal}，执行尝试 {}。保留ID与成果，先对账未知写入，仅推进剩余工作。达到当前阶段结果即提交；遇到真实人工决定或用户暂停时等待，不重复已完成请求。",request.id));
        let autonomous_prompt = unit.as_ref().map(|u| format!(
            "Lua 自主执行：目标 {}，工作项 {}：{}，路线 {}。读取 production current；策略变化时按需读取本任务 references/autonomous-implementation.md。核对未知写入后完成本项，以 report-work-unit 记录实际结果，再结束本轮，APP 自动派发后续。普通错误自行修复，PB问题有限恢复/等效替代后隔离依赖，继续独立项，不要求用户反复继续。候选与fixture版本分开。先形成真实输入、核心循环和保存重载的可玩检查点，再补齐完整范围。2.8检查失败须report-work-unit标repairable，APP派发repair-plan后用revise-work-unit修订原项并继续；外部/未知结果要附证据和nextCondition。最终核对时先build-verification-package，随后complete-stage或如实conclude-implementation，不重复报告。不得把暂缓、静态校验或调用成功当成运行通过。",u.goal_id,u.id,u.title,u.routes[u.route]));
        let preflight_prompt=production.record.as_ref().filter(|r|crate::modules::tasks::preflight_recovery_active(r)).map(|r|format!(
            "技术预检有限恢复，第{}轮，原目标{}。只处理本任务当前{}阶段的运行加载/CLI/UI前置条件，不进入Lua实现，不批准文档或关闭用户反馈。先production current核对当前问题和已完成操作，历史失败资料不是当前状态。连续只读CLI超时后改查托管进程、磁盘启动报告与日志，不反复堆叠主线程请求。热加载部分成功时保留原存档ID，不重复建档；有未知写入先对账。仅对本任务专用预检实例、已证明无未保存操作且原进程确认退出后，按原存档和确切关卡作干净启动；包含已尝试的首次冷启动，总共最多两次。用户试玩实例、未知写入、未保存状态不允许自动重启或强停。先核对同一启动身份的ready/worldReady和实际存档关卡，再核对所需入口；UI面板前置条件用当前可见原生按钮处理，不猜历史组件ID。恢复后用原问题ID登记证据、更新当前摘要并继续资源/方案预检；根因不明仍标未确认。范围外权限/缺资料或预算耗尽如实保留，不将未测项记通过。每轮至多10分钟，正常审核与用户暂停优先。",
            r.workbench.execution.preflight.attempts,r.workbench.execution.preflight.goal_id,r.current_stage));
        let general_prompt = format!(
            "{stage_contract}继续当前制作目标。按当前记录处理尚未执行的 workbench.requests、caseRequests 和策划反馈，按ID/版本去重。沿用已确认要求及持续略过事项，先查看实际参考和成果。按需要加载本任务Skill；记录实际进展，完成本阶段结果后提交。普通执行连续推进，真实审核/回答/复验或用户暂停时等待，不替用户批准。"
        );
        let sent = if let Some(group) = request.group_id.as_ref() {
            self.continue_questions_locked(&request.task_id, group, request.answer_revision)
        } else {
            let receipt = directory.join(format!("task-continue-{}.json", request.id));
            let mut attempt = intake_receipt::start_attempt(&receipt, None, &request.native_id)?;
            let result=server.request("turn/start",json!({"threadId":request.native_id,"clientUserMessageId":attempt["messageId"],
                "input":[{"type":"text","text":autonomous_prompt.as_deref().or(preflight_prompt.as_deref()).or(repair_spec.as_ref().map(|s|s.prompt.as_str())).or(goal_prompt.as_deref()).unwrap_or(&general_prompt)}]}));
            result.and_then(|v| {
                let id = v["turn"]["id"]
                    .as_str()
                    .ok_or_else(|| AppError::validation("未返回执行标识，请核对历史。"))?;
                attempt["status"] = json!("sent");
                attempt["turnId"] = json!(id);
                intake_receipt::save(&receipt, &attempt, false)?;
                request.turn_id = Some(id.into());
                Ok(request.response(S::Started))
            })
        };
        if gate.epoch() != generation {
            return Ok(request.response(S::Paused));
        }
        match sent {
            Ok(mut response) => {
                request.status = match response.status {
                    S::Started | S::Resumed | S::Running => "sent",
                    S::NeedsConnection => "needsConnection",
                    _ => "settled",
                }
                .into();
                request.turn_id = response.turn_id.clone();
                if let Some(id) = &request.attempt_id {
                    self.tasks.production_attempt_status(
                        &request.task_id,
                        id,
                        "running",
                        response.turn_id.as_deref(),
                        "",
                    )?;
                }
                save(directory, request)?;
                response.request_id = Some(request.id.clone());
                Ok(response)
            }
            Err(error) => {
                request.status = "needsReview".into();
                request.error = Some(error.message.clone());
                if let Some(id) = &request.attempt_id {
                    let _ = self.tasks.production_attempt_status(
                        &request.task_id,
                        id,
                        "needsReview",
                        None,
                        "派发结果未确认，不自动重发。",
                    );
                }
                save(directory, request)?;
                Err(error)
            }
        }
    }

    pub(super) fn queue_event(&self, native: &str, turn: &str, completed: bool) {
        let Some((task, conversation)) = self.thread_conversations.lock().get(native).cloned()
        else {
            return;
        };
        let service = self.clone();
        let turn = turn.to_string();
        std::thread::spawn(move || {
            let gate = service.gate(&conversation);
            let _guard = gate.normal.lock();
            let Ok(task) = service.tasks.get(&task) else {
                return;
            };
            let directory = conversation_directory(Path::new(&task.workspace_path), &conversation);
            if let Ok(Some(mut request)) = read(&directory)
                && request.owner == service.process_epoch
                && request.status == "queued"
                && request.after_turn.as_deref() == Some(&turn)
            {
                if completed {
                    let _ = service.dispatch_or_record(&directory, &mut request);
                } else {
                    request.status = "paused".into();
                    let _ = save(&directory, &mut request);
                }
                return;
            }
            let _ = service.auto_complete_locked(&task.id, &conversation, &turn, completed);
        });
    }

    pub(super) fn pause_queued_locked(&self, conversation: &str) -> AppResult<()> {
        let task_id = self.queue_tasks.lock().get(conversation).cloned();
        if let Some(task_id) = task_id {
            let task = self.tasks.get_readonly(&task_id)?;
            let directory = conversation_directory(Path::new(&task.workspace_path), conversation);
            if let Some(mut q) = read(&directory)? {
                if q.status != "sending" && q.status != "needsReview" {
                    q.status = "paused".into();
                }
                save(&directory, &mut q)?;
            }
        }
        Ok(())
    }
}
