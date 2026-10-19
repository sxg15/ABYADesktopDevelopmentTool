use super::CodexTerminalService;
use serde_json::{Value, json};

impl CodexTerminalService {
    // 只读取已有工作流的有界元数据，不请求模型、不复制原始终端或工具参数。
    pub fn memory_incident_context(&self, task_id: &str) -> Value {
        let Ok(task) = self.tasks.get_readonly(task_id) else {
            return json!({"unavailable":"任务不可读"});
        };
        let root = super::conversations_root(std::path::Path::new(&task.workspace_path));
        let Ok(entries) = std::fs::read_dir(&root) else {
            return json!({"unavailable":"会话目录不存在"});
        };
        let mut conversations = Vec::new();
        for entry in entries.flatten().take(64) {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let file = entry.path().join(super::CONVERSATION_METADATA_FILE);
            if !file
                .metadata()
                .is_ok_and(|metadata| metadata.len() <= 65536)
            {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&file)
                && let Ok(conversation) = serde_json::from_slice::<super::CodexConversation>(&bytes)
                && conversation.task_id == task_id
                && uuid::Uuid::parse_str(&conversation.id).is_ok()
            {
                conversations.push(conversation);
            }
        }
        conversations.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        let mut values = Vec::new();
        for conversation in conversations.iter().take(3) {
            let directory = super::conversation_directory(
                std::path::Path::new(&task.workspace_path),
                &conversation.id,
            );
            if !directory
                .join("workflow.json")
                .metadata()
                .is_ok_and(|metadata| metadata.len() <= 4 * 1024 * 1024)
            {
                values.push(json!({"conversationId":conversation.id,"unavailable":"工作流缺失或超过异常采集预算"}));
                continue;
            }
            match super::load_workflow(&directory, task_id, &conversation.id) {
                Ok(workflow) => {
                    let turns: Vec<_> = workflow.turns.iter().rev().take(3).map(|turn| {
                        let activities: Vec<_> = turn.activities.iter().rev().take(32).map(|activity| json!({
                            "id":activity.id,"kind":activity.kind,"status":activity.status,
                            "source":activity.source,"stepId":activity.step_id,
                            "startedAt":activity.started_at,"completedAt":activity.completed_at,
                            "summary":crate::modules::development_terminal::diagnostic_summary(&activity.summary)
                        })).collect();
                        json!({"id":turn.id,"status":turn.status,"startedAt":turn.started_at,
                            "completedAt":turn.completed_at,"activities":activities})
                    }).collect();
                    values.push(json!({"conversationId":conversation.id,"nativeSessionId":conversation.native_session_id,
                        "updatedAt":workflow.updated_at,"turns":turns}));
                }
                Err(_) => values
                    .push(json!({"conversationId":conversation.id,"unavailable":"工作流不可读"})),
            }
        }
        json!({"taskId":task_id,"capturedAt":chrono::Utc::now().to_rfc3339(),"conversations":values,
            "scope":"最近三个会话，每个三轮、每轮32项；操作顺序不证明内存异常因果；原始参数与终端正文未复制"})
    }
}
