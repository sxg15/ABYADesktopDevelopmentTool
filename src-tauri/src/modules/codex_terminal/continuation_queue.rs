use super::intake_receipt::{self, IntakeContinueStatus};
use super::*;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ContinueRequest {
    pub id: String,
    pub task_id: String,
    pub conversation_id: String,
    pub native_id: String,
    pub group_id: Option<String>,
    pub answer_revision: u64,
    #[serde(default)]
    pub after_revision: Option<u64>,
    pub status: String,
    pub owner: String,
    pub revision: u64,
    pub updated_at: String,
    pub after_turn: Option<String>,
    pub turn_id: Option<String>,
    pub error: Option<String>,
}

impl ContinueRequest {
    fn response(&self, status: IntakeContinueStatus) -> IntakeContinuation {
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
        let _guard = self.lifecycle_lock.lock();
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
                status: "queued".into(),
                owner: self.process_epoch.clone(),
                revision: previous.as_ref().map_or(0, |q| q.revision),
                updated_at: String::new(),
                after_turn: None,
                turn_id: None,
                error: None,
            }
        };
        self.cancelled.lock().remove(conversation);
        save(&directory, &mut request)?;
        self.dispatch_or_record(&directory, &mut request)
    }

    pub fn flush_continuation(
        &self,
        task: &str,
        conversation: &str,
        request_id: &str,
    ) -> AppResult<IntakeContinuation> {
        let _guard = self.lifecycle_lock.lock();
        let t = self.tasks.get(task)?;
        self.conversation(task, conversation)?;
        let directory = conversation_directory(Path::new(&t.workspace_path), conversation);
        let mut request =
            read(&directory)?.ok_or_else(|| AppError::validation("没有待续接请求。"))?;
        if request.id != request_id {
            return Err(AppError::validation("继续请求已更新，请读取最新状态。"));
        }
        self.dispatch_or_record(&directory, &mut request)
    }

    fn dispatch_or_record(
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
            request.status = if status == S::WaitingForAnswers {
                "waitingForAnswers"
            } else {
                "waitingForApproval"
            }
            .into();
            save(directory, request)?;
            return Ok(request.response(status));
        }
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
        let runtime = self
            .runtime_servers
            .lock()
            .get(&request.conversation_id)
            .cloned();
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
            .into_iter()
            .flatten()
            .find(|t| t["status"] == "inProgress")
        {
            if request.group_id.is_none() && request.after_revision.is_none() {
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
        request.status = "sending".into();
        save(directory, request)?;
        let sent = if let Some(group) = request.group_id.as_ref() {
            self.continue_questions_locked(&request.task_id, group, request.answer_revision)
        } else {
            let receipt = directory.join(format!("task-continue-{}.json", request.id));
            let mut attempt = intake_receipt::start_attempt(&receipt, None, &request.native_id)?;
            let result=server.request("turn/start",json!({"threadId":request.native_id,"clientUserMessageId":attempt["messageId"],
                "input":[{"type":"text","text":"用户在 APP 点击继续任务。读取最新制作记录，核对已有成果后继续当前阶段。先在顶部对话步骤登记本轮工作。遇到已知问题时检查是否有新进展，说明问题、处理方式和下一步，并继续可完成的相关工作。进展用简短自然的中文说明；需求、执行计划和交付通过原有文档确认流程处理。"}]}));
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
        match sent {
            Ok(mut response) => {
                request.status = match response.status {
                    S::Started | S::Resumed | S::Running => "sent",
                    S::NeedsConnection => "needsConnection",
                    _ => "settled",
                }
                .into();
                request.turn_id = response.turn_id.clone();
                save(directory, request)?;
                response.request_id = Some(request.id.clone());
                Ok(response)
            }
            Err(error) => {
                request.status = "needsReview".into();
                request.error = Some(error.message.clone());
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
            let _guard = service.lifecycle_lock.lock();
            let Ok(task) = service.tasks.get(&task) else {
                return;
            };
            let directory = conversation_directory(Path::new(&task.workspace_path), &conversation);
            let Ok(Some(mut request)) = read(&directory) else {
                return;
            };
            if request.owner != service.process_epoch
                || request.status != "queued"
                || request.after_turn.as_deref() != Some(&turn)
            {
                return;
            }
            if completed {
                let _ = service.dispatch_or_record(&directory, &mut request);
            } else {
                request.status = "paused".into();
                let _ = save(&directory, &mut request);
            }
        });
    }

    pub(super) fn pause_queued_locked(&self, conversation: &str) -> AppResult<()> {
        let task_id = self.queue_tasks.lock().get(conversation).cloned();
        if let Some(task_id) = task_id {
            let task = self.tasks.get(&task_id)?;
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
