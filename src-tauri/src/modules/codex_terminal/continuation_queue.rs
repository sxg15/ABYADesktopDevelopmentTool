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
                            && !(production
                                .record
                                .as_ref()
                                .is_some_and(crate::modules::tasks::autonomous_implementation)
                                && (t["status"] == "failed"
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
            request.goal_id = Some(unit.goal_id.clone());
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
        let goal_prompt=request.goal_id.as_ref().map(|goal|format!("继续唯一子流程 {goal}（原ID保留）。先读取该子流程与执行尝试 {}，核对已有成果和结果未知的写入，只推进本目标剩余工作，不重新创建‘继续’目标，不遍历重放其他请求。正常无人工待办时自动推进；审核、回答、复验、阻塞或暂停仍需遵守。",request.id));
        let autonomous_prompt = unit.as_ref().map(|u| format!(
            "Lua 自主执行。原子目标 {}，当前工作项 {}：{}。当前路线：{}。先 production current 核对实际结果与未知写入，再执行本工作项。用 save-work-unit 拆解独立工作和依赖，用 report-work-unit 记录实际结果文件。报告当前工作项结果后结束本轮输出；APP 会自动派发下一项，不要报告未派发工作项或询问是否继续。普通错误自行修复；疑似 PB 问题先排除配置/测试污染，再尝试保持原玩法要求的替代方案；失败则 record-issue-handling 保留影响并暂缓依赖工作，继续独立项。不要求用户继续，不把 AI 自测问题交人工复验，不把暂缓当通过。相同条件下禁止重复已失败路线。所有工作项收敛后核对必要自测，满足原门槛再 complete-stage；有缺口使用 conclude-implementation 保留差异。不得改批准或隐藏交付缺口。",u.goal_id,u.id,u.title,u.routes[u.route]));
        let sent = if let Some(group) = request.group_id.as_ref() {
            self.continue_questions_locked(&request.task_id, group, request.answer_revision)
        } else {
            let receipt = directory.join(format!("task-continue-{}.json", request.id));
            let mut attempt = intake_receipt::start_attempt(&receipt, None, &request.native_id)?;
            let result=server.request("turn/start",json!({"threadId":request.native_id,"clientUserMessageId":attempt["messageId"],
                "input":[{"type":"text","text":autonomous_prompt.as_deref().or(repair_spec.as_ref().map(|s|s.prompt.as_str())).or(goal_prompt.as_deref()).unwrap_or(if request.automatic_after.is_some() { "当前目标尚未完成且没有人工待办，自动推进下一子阶段。读取最新制作记录和已有成果，直接继续实现、检查与验证；不把一次回复结束当作需要用户点击继续。更新具体目标与进展；仅在需要审核、用户回答/复验、实际阻塞、执行失败或任务完成时停下。不得重复已完成操作，不得替用户批准。" } else { "用户在 APP 点击继续任务。读取最新制作记录，核对已有成果后继续当前阶段。若 workbench.requests 有用户请求，按 id 和 revision 核对已有执行记录，处理尚未执行的请求，开始/完成时用 update-workbench-request 记录 status 与 summary，禁止重复执行；workbench.deferrals 是本任务持续略过的可选建议，保留影响说明，不重复打断，不能把略过当成修复或批准。若 artDesign.caseRequests 有本任务本轮 APP 案例请求，先按 selection 的具体版本与 prompt 核对已有处理记录，再查看候选图像、评估参考或整理草稿；请求不等于已采用或已认可，不重复执行已完成请求，不绕过人工确认。若 artifacts 中存在 plannerReference=true 且 status 不是 withdrawn 的策划参考图片，读取其路径、说明并查看图片后用于美术需求；它们不是验收成果，变更方案仍需修订文档并请求人工确认。先在顶部对话步骤登记本轮工作。遇到已知问题时检查是否有新进展，说明问题、处理方式和下一步，并继续可完成的相关工作。进展用简短自然的中文说明；需求、执行计划和交付通过原有文档确认流程处理。" }) }]}));
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
