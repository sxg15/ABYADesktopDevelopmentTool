use super::*;
use crate::modules::tasks::DevelopmentTask;
use serde_json::Value;

// Current Codex schema exposes archive storage through Thread.path (unstable).
// Never infer "active" from omission in thread/list: empty threads can be omitted.
fn native_archived_state(thread: &Value) -> AppResult<bool> {
    if let Some(value) = thread["archived"].as_bool() {
        return Ok(value);
    }
    let path = thread["path"].as_str().map(Path::new).ok_or_else(|| {
        AppError::validation(
            "Codex did not expose the stored conversation state; automatic resume was prevented.",
        )
    })?;
    if path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("archived_sessions"))
    {
        return Ok(true);
    }
    if path
        .components()
        .any(|c| c.as_os_str().eq_ignore_ascii_case("sessions"))
    {
        return Ok(false);
    }
    Err(AppError::validation(
        "Unsupported Codex conversation storage layout; history was preserved.",
    ))
}

impl AppServerHandle {
    pub(super) fn ensure_project(&self, task: &DevelopmentTask) -> AppResult<String> {
        let name = format!("ABYA · {}", task.title);
        let mut cursor = Value::Null;
        for _ in 0..100 {
            let page = self.request("project/list", json!({"limit":100,"cursor":cursor}))?;
            for project in page["data"].as_array().into_iter().flatten() {
                let owned = project["metadata"]["abyaTaskId"].as_str() == Some(&task.id);
                let matching = project["roots"].as_array().is_some_and(|roots| {
                    roots.iter().any(|r| {
                        r["path"]
                            .as_str()
                            .is_some_and(|p| Path::new(p) == Path::new(&task.workspace_path))
                    })
                });
                if owned || matching {
                    let id = project["id"]
                        .as_str()
                        .ok_or_else(|| AppError::internal("Invalid Codex project identity."))?;
                    if owned && (project["name"].as_str() != Some(&name) || !matching) {
                        self.request("project/update",json!({"projectId":id,"name":name,"roots":[{"path":task.workspace_path}]}))?;
                    }
                    return Ok(id.into());
                }
            }
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                let result=self.request("project/create",json!({"idempotencyKey":format!("abya-task-{}",task.id),"name":name,"roots":[{"path":task.workspace_path}],"metadata":{"abyaTaskId":task.id,"origin":"ABYA Desktop Development Tool"}}))?;
                return result["project"]["id"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| AppError::internal("Codex did not return a project ID."));
            }
        }
        Err(AppError::validation(
            "Codex project list is too large to safely register this task.",
        ))
    }

    pub(super) fn organize_thread(
        &self,
        project: &str,
        task: &DevelopmentTask,
        conversation: &CodexConversation,
        thread: &str,
    ) -> AppResult<()> {
        let current = self.request(
            "thread/read",
            json!({"threadId":thread,"includeTurns":false}),
        )?;
        if current["thread"]["projectId"].as_str() != Some(project) {
            self.request(
                "thread/metadata/update",
                json!({"threadId":thread,"projectId":project}),
            )?;
        }
        let name = format!("ABYA · {} · {}", task.title, conversation.title);
        if current["thread"]["name"].as_str() != Some(&name) {
            self.request("thread/name/set", json!({"threadId":thread,"name":name}))?;
        }
        Ok(())
    }

    pub(super) fn is_archived(&self, thread: &str) -> AppResult<bool> {
        let value = self.request(
            "thread/read",
            json!({"threadId":thread,"includeTurns":false}),
        )?;
        native_archived_state(&value["thread"])
    }
}

impl CodexTerminalService {
    pub fn project_workspace(&self, task_id: &str) -> AppResult<String> {
        Ok(self.tasks.get(task_id)?.workspace_path)
    }

    pub fn set_conversation_archived(
        &self,
        task_id: &str,
        conversation_id: &str,
        archived: bool,
    ) -> AppResult<CodexConversation> {
        let _guard = self.lifecycle_lock.lock();
        self.set_conversation_archived_locked(task_id, conversation_id, archived)
    }

    pub(super) fn set_conversation_archived_locked(
        &self,
        task_id: &str,
        conversation_id: &str,
        archived: bool,
    ) -> AppResult<CodexConversation> {
        let task = self.tasks.get(task_id)?;
        let mut conversation = self.conversation(task_id, conversation_id)?;
        if let Some(id) = &conversation.native_session_id {
            let codex = (self.resolver)().ok_or_else(|| {
                AppError::validation("Codex is unavailable; conversation history was preserved.")
            })?;
            let server = self.ensure_app_server(&codex)?;
            let project = server.ensure_project(&task)?;
            server.organize_thread(&project, &task, &conversation, id)?;
            let native_archived = server.is_archived(id)?;
            if archived {
                if !native_archived {
                    let workflow = self.workflow(task_id, conversation_id)?;
                    if let Some(turn) = workflow.turns.iter().find(|turn| {
                        Some(&turn.id) == workflow.current_turn_id.as_ref()
                            && matches!(
                                turn.status,
                                CodexWorkflowTurnStatus::WaitingForPlan
                                    | CodexWorkflowTurnStatus::InProgress
                            )
                    }) {
                        server
                            .request("turn/interrupt", json!({"threadId":id,"turnId":turn.id}))?;
                    }
                    self.stop(conversation_id)?;
                    server.request("thread/unsubscribe", json!({"threadId":id}))?;
                    server.request("thread/archive", json!({"threadId":id}))?;
                } else {
                    self.stop(conversation_id)?;
                }
            } else if native_archived {
                server.request("thread/unarchive", json!({"threadId":id}))?;
            }
        } else if archived {
            self.stop(conversation_id)?;
        }
        conversation.archived = archived;
        conversation.updated_at = Utc::now().to_rfc3339();
        write_conversation(
            &conversation_directory(Path::new(&task.workspace_path), conversation_id),
            &conversation,
        )?;
        Ok(conversation)
    }

    pub(super) fn refresh_native_conversations(
        &self,
        task: &DevelopmentTask,
        conversations: &mut [CodexConversation],
    ) -> AppResult<()> {
        if !conversations.iter().any(|c| c.native_session_id.is_some()) {
            return Ok(());
        }
        let _guard = self.lifecycle_lock.lock();
        let codex =
            (self.resolver)().ok_or_else(|| AppError::validation("Codex is unavailable."))?;
        let server = self.ensure_app_server(&codex)?;
        let project = server.ensure_project(task)?;
        for conversation in conversations.iter() {
            if let Some(id) = &conversation.native_session_id {
                server.organize_thread(&project, task, conversation, id)?;
            }
        }
        for conversation in conversations {
            let native_archived = match &conversation.native_session_id {
                Some(id) => server.is_archived(id)?,
                None => conversation.archived,
            };
            if native_archived != conversation.archived {
                self.stop(&conversation.id)?;
                conversation.archived = native_archived;
                conversation.updated_at = Utc::now().to_rfc3339();
                write_conversation(
                    &conversation_directory(Path::new(&task.workspace_path), &conversation.id),
                    conversation,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Starts installed Codex PTYs; run alone with ABYA_TEST_CODEX"]
    fn real_terminal_archive_restore_and_project_registration() {
        use crate::{
            foundation::{AppPaths, Database},
            modules::tasks::TaskInput,
        };
        let parent = PathBuf::from(std::env::var_os("USERPROFILE").unwrap())
            .join("ABYA Desktop Development ToolWorkspaces");
        let root = parent.join(format!("lifecycle-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = AppPaths {
            data_dir: root.clone(),
            database_path: root.join("db.sqlite"),
            bootstrap_logs_dir: root.join("logs"),
            reports_dir: root.join("reports"),
            archive_transfers_dir: root.join("transfers"),
            default_archive_root: root.join("archives"),
            default_workspace_root: root.join("workspaces"),
        };
        let tasks = TaskService::new(
            Database::open(&paths).unwrap(),
            paths.default_workspace_root.clone(),
        );
        let task = tasks
            .create(TaskInput {
                title: "ABYA temporary lifecycle validation".into(),
                description: String::new(),
            })
            .unwrap();
        let service = CodexTerminalService::for_test(
            tasks,
            root.clone(),
            ResolvedCodex {
                path: std::env::var("ABYA_TEST_CODEX").unwrap().into(),
                kind: CodexLaunchKind::Executable,
            },
        );
        let conversation = service
            .create_conversation(&task.id, Some("Lifecycle validation".into()))
            .unwrap();
        let opened = service
            .open(
                &task.id,
                &conversation.id,
                100,
                30,
                Channel::new(|_| Ok(())),
            )
            .unwrap();
        assert_eq!(opened.status, CodexTerminalStatus::Running);
        let native = service
            .conversation(&task.id, &conversation.id)
            .unwrap()
            .native_session_id
            .unwrap();
        let server = service.app_server.lock().clone().unwrap();
        let project = server.ensure_project(&task).unwrap();
        assert_eq!(server.ensure_project(&task).unwrap(), project);
        let metadata = server
            .request("thread/read", json!({"threadId":native}))
            .unwrap();
        assert_eq!(metadata["thread"]["projectId"], project);
        assert!(
            metadata["thread"]["name"]
                .as_str()
                .unwrap()
                .contains("Lifecycle validation")
        );
        assert!(
            service
                .set_conversation_archived(&task.id, &conversation.id, true)
                .unwrap()
                .archived
        );
        assert!(server.is_archived(&native).unwrap());
        assert!(service.sessions.lock().is_empty());
        assert!(
            service
                .open(
                    &task.id,
                    &conversation.id,
                    100,
                    30,
                    Channel::new(|_| Ok(()))
                )
                .is_err()
        );
        let restored = service
            .set_conversation_archived(&task.id, &conversation.id, false)
            .unwrap();
        assert_eq!(restored.native_session_id.as_deref(), Some(native.as_str()));
        assert!(!restored.archived);
        service
            .open(
                &task.id,
                &conversation.id,
                100,
                30,
                Channel::new(|_| Ok(())),
            )
            .unwrap();
        service.stop(&conversation.id).unwrap();
        server
            .request("thread/archive", json!({"threadId":native}))
            .unwrap();
        let mut conversations = service.list_conversations(&task.id).unwrap();
        service
            .refresh_native_conversations(&task, &mut conversations)
            .unwrap();
        assert!(conversations[0].archived);
        server
            .request("thread/unarchive", json!({"threadId":native}))
            .unwrap();
        service
            .refresh_native_conversations(&task, &mut conversations)
            .unwrap();
        assert!(!conversations[0].archived);
        service
            .delete_conversation(&task.id, &conversation.id)
            .unwrap();
        assert!(server.is_archived(&native).unwrap());
        server
            .request("project/delete", json!({"projectId":project}))
            .unwrap();
        service.stop_all();
        drop(service);
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(parent.canonicalize().unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
        println!(
            "real PTY: project registration, title, archive, restore, no resurrection, delete/history preservation passed"
        );
    }
    #[test]
    fn archive_state_is_not_inferred_from_empty_lists_or_unknown_paths() {
        assert!(
            native_archived_state(
                &json!({"path":r"C:\Users\user\.codex\archived_sessions\rollout.jsonl"})
            )
            .unwrap()
        );
        assert!(
            !native_archived_state(
                &json!({"path":r"C:\Users\user\.codex\sessions\2026\09\24\rollout.jsonl"})
            )
            .unwrap()
        );
        assert!(native_archived_state(&json!({"archived":true})).unwrap());
        assert!(native_archived_state(&json!({"path":null})).is_err());
        assert!(native_archived_state(&json!({"path":r"C:\unknown\rollout.jsonl"})).is_err());
    }
}
