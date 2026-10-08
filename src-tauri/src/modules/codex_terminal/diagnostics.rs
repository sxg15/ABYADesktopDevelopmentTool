use super::*;

pub(super) fn record(workspace: &Path, conversation: &str, event: &str, native: Option<&str>) {
    // Fixed event labels and IDs only; never persist arguments, environment, or RPC errors.
    let path = conversation_directory(workspace, conversation).join("connection-events.jsonl");
    if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(path) {
        let entry = json!({"at":Utc::now().to_rfc3339(),"event":event,"conversationId":conversation,
            "nativeSessionId":native,"appVersion":env!("CARGO_PKG_VERSION")});
        let _ = writeln!(file, "{entry}");
    }
}

pub(super) fn usage(directory: &Path, thread: &str, params: &serde_json::Value) {
    let mut total = serde_json::Map::new();
    for key in [
        "totalTokens",
        "inputTokens",
        "outputTokens",
        "cachedInputTokens",
        "reasoningOutputTokens",
    ] {
        if let Some(value) = params["tokenUsage"]["total"][key].as_u64() {
            total.insert(key.into(), json!(value));
        }
    }
    if total.is_empty() {
        return;
    }
    // Native totals are snapshots, not deltas: keep latest per native identity.
    if Uuid::parse_str(thread).is_err() {
        return;
    }
    let entry = json!({"at":Utc::now().to_rfc3339(),"nativeSessionId":thread,"total":total});
    let _ = std::fs::write(
        directory.join(format!("usage-{thread}.json")),
        entry.to_string(),
    );
}

impl CodexTerminalService {
    pub fn metrics(&self, task_id: &str, conversation_id: &str) -> AppResult<serde_json::Value> {
        let conversation = self.conversation(task_id, conversation_id)?;
        let task = self.tasks.get(task_id)?;
        let directory = conversation_directory(Path::new(&task.workspace_path), conversation_id);
        let workflow = self.workflow(task_id, conversation_id)?;
        let duration = |start: &str, end: &str| -> Option<i64> {
            let a = DateTime::parse_from_rfc3339(start).ok()?;
            let b = DateTime::parse_from_rfc3339(end).ok()?;
            Some((b - a).num_milliseconds().max(0))
        };
        let turns = workflow.turns.iter().map(|turn| json!({"id":turn.id,"status":turn.status,
            "startedAt":turn.started_at,"completedAt":turn.completed_at,
            "durationMs":turn.completed_at.as_deref().and_then(|end| duration(&turn.started_at,end))})).collect::<Vec<_>>();
        let questions = self.tasks.production_get(task_id)?.record.into_iter().flat_map(|r| r.question_groups)
            .filter(|g| g.conversation_id == conversation_id && g.provider == "codex")
            .map(|g| json!({"id":g.id,"status":g.status,"answerVersions":g.answers.len(),
                "publishedAt":g.published_at,"firstSubmittedAt":g.answers.first().map(|a|&a.submitted_at),
                "firstAnswerWaitMs":g.answers.first().and_then(|a| duration(&g.published_at,&a.submitted_at))})).collect::<Vec<_>>();
        let events = std::fs::read_to_string(directory.join("connection-events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .collect::<Vec<_>>();
        let usage = conversation
            .native_session_id
            .as_ref()
            .filter(|id| Uuid::parse_str(id).is_ok())
            .and_then(|id| std::fs::read(directory.join(format!("usage-{id}.json"))).ok())
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        Ok(
            json!({"conversationId":conversation_id,"nativeSessionId":conversation.native_session_id,
            "turns":turns,"questions":questions,"connectionEvents":events,"nativeUsage":usage,
            "timingNote":"轮次耗时包含工具与原生等待；答题等待可能与轮次重叠，不可直接相加，也不是模型纯计算时间。缺少结束事件时为 null。历史修复可能包含两个原生会话的旧投影，按原生 ID 核对。"}),
        )
    }
}
