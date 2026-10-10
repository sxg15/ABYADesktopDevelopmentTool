use super::*;

pub(super) fn player_mode(data: &Value) -> AppResult<Option<&str>> {
    if data.get("playerMode").is_none() {
        return Ok(None);
    }
    match data["playerMode"].as_str() {
        Some(value @ ("unspecified" | "single" | "multiplayer")) => Ok(Some(value)),
        _ => Err(AppError::validation("请选择未明确、单人或多人。")),
    }
}

pub(super) fn invalidate(r: &mut ProductionRecord) {
    invalidate_from(r, "requirements");
}

fn invalidate_from(r: &mut ProductionRecord, stage: &str) {
    let stages = [
        "requirements",
        "resources",
        "plan",
        "implementation",
        "review",
        "delivery",
        "closeout",
    ];
    for id in stages.iter().skip_while(|s| **s != stage) {
        r.stages.insert((*id).into(), "not-started".into());
    }
    r.stages.insert(stage.into(), "in-progress".into());
    r.current_stage = stage.into();
}

fn answer(r: &mut ProductionRecord, operation: &str, data: &Value) -> AppResult<()> {
    if operation == "set-player-mode" {
        let mode = player_mode(data)?.ok_or_else(|| AppError::validation("缺少人数模式。"))?;
        if r.player_mode != mode {
            r.player_mode = mode.into();
            for group in &mut r.question_groups {
                if group.status == "pending" {
                    group.status = "cancelled".into();
                    group.updated_at = now();
                }
            }
            invalidate(r);
        }
        return Ok(());
    }
    let id = mutation::text(data, "id")?;
    if operation == "amend"
        && r.question_groups
            .iter()
            .any(|g| g.id != id && g.status == "pending")
    {
        return Err(AppError::validation(
            "请先提交或取消当前待答问题，再修改旧答案。",
        ));
    }
    let group = r
        .question_groups
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or_else(|| AppError::validation("问题组不存在。"))?;
    match operation {
        "amend" if group.status == "submitted" => {
            group.status = "pending".into();
        }
        "cancel" if group.status == "pending" => {
            group.status = "cancelled".into();
        }
        "save-draft" | "submit-answers" if group.status == "pending" => {
            let answers: BTreeMap<String, String> =
                serde_json::from_value(data["answers"].clone())?;
            if answers
                .iter()
                .any(|(id, text)| text.len() > 8000 || !group.questions.iter().any(|q| q.id == *id))
            {
                return Err(AppError::validation("答案过长或包含未知题目。"));
            }
            if operation == "submit-answers" {
                if group
                    .questions
                    .iter()
                    .any(|q| !q.optional && answers.get(&q.id).is_none_or(|a| a.trim().is_empty()))
                {
                    return Err(AppError::validation("请回答所有必答题后提交。"));
                }
                group.answers.push(AnswerRevision {
                    revision: group.answers.len() as u64 + 1,
                    answers: answers.clone(),
                    submitted_at: now(),
                });
                group.status = "submitted".into();
            }
            group.draft = answers;
        }
        _ => return Err(AppError::validation("问题状态已变化，请刷新后操作。")),
    }
    group.updated_at = now();
    let stage = group.stage.clone();
    if operation != "save-draft" {
        invalidate_from(r, &stage);
    }
    Ok(())
}

pub(super) fn publish(r: &mut ProductionRecord, data: &Value) -> AppResult<()> {
    let stage = data["stage"]
        .as_str()
        .unwrap_or(&r.current_stage)
        .to_string();
    if r.question_mode != "ask" || stage != r.current_stage || !r.stages.contains_key(&stage) {
        return Err(AppError::validation(
            "请在当前制作阶段发布问题，并开启允许追问。",
        ));
    }
    let id = mutation::text(data, "id")?;
    if id.len() > 80
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::validation(
            "问题组 ID 仅允许 1–80 位字母、数字、横线或下划线。",
        ));
    }
    if r.question_groups.iter().any(|g| g.id == id) {
        return Err(AppError::validation(
            "问题组已存在，请先读取记录，不要重复发布。",
        ));
    }
    if r.question_groups.len() >= 100 || r.question_groups.iter().any(|g| g.status == "pending") {
        return Err(AppError::validation("请先完成或取消当前问题组。"));
    }
    let questions: Vec<IntakeQuestion> = serde_json::from_value(data["questions"].clone())?;
    let mut ids = std::collections::HashSet::new();
    if questions.is_empty()
        || questions.len() > 12
        || questions.iter().any(|q| {
            q.id.is_empty()
                || q.id.len() > 80
                || !ids.insert(&q.id)
                || q.text.trim().is_empty()
                || q.text.len() > 4000
                || q.options.len() > 10
                || q.options
                    .iter()
                    .any(|s| s.trim().is_empty() || s.len() > 1000)
        })
    {
        return Err(AppError::validation(
            "每组需有 1–12 道有效问题，题目 ID 不可重复。",
        ));
    }
    let title = mutation::text(data, "title")?;
    if title.len() > 300 {
        return Err(AppError::validation("问题组标题过长。"));
    }
    let provider = mutation::text(data, "provider")?;
    let conversation = mutation::text(data, "conversationId")?;
    if !["codex", "grok"].contains(&provider) || uuid::Uuid::parse_str(conversation).is_err() {
        return Err(AppError::validation("问题必须关联有效的任务会话。"));
    }
    r.question_groups.push(QuestionGroup {
        stage,
        cycle: r.cycle,
        id: id.into(),
        title: title.into(),
        provider: provider.into(),
        conversation_id: conversation.into(),
        native_session_id: data["nativeSessionId"].as_str().map(str::to_owned),
        questions,
        draft: BTreeMap::new(),
        answers: vec![],
        status: "pending".into(),
        published_at: now(),
        updated_at: now(),
    });
    Ok(())
}

impl TaskService {
    /// User-only entrypoint. Agent mutations cannot submit planner answers.
    pub fn production_answer(&self, input: ProductionMutation) -> AppResult<ProductionView> {
        self.production_commit(&input.task_id, input.expected_revision, |r| {
            answer(r, &input.operation, &input.data)
        })?;
        self.production_export(&input.task_id)?;
        self.production_get(&input.task_id)
    }
}
