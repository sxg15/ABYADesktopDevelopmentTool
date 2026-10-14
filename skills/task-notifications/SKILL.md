# Task Notifications Skill

## Purpose

Project all active tasks' human actions into one ordered source for guidance and Windows background notifications.

## Ownership

Owns attention projection, receipts, background monitor, activation revalidation and notification settings panel. Foundation owns Toast/COM, shared DTOs and the React subscription adapter. App shell composes navigation. Never submit answers, approve, resume a turn or create a conversation from a notification.

## Public Contracts

TaskNotificationService uses only TaskService list_readonly/get_readonly/production_attention and provider attention_states; no workspace creation, Skill sync, native sync, PTY or model startup. Content-free signals wake the monitor; a 3-second compensation scan works without WebView. Guidance shares ordered TaskAttentionSnapshot: terminal wait, questions, valid reviews, execution errors, playtest feedback, other blockers.

get_task_attention(taskId) returns cached {taskId,items,unavailable}. Items include SHA-256 key, task/title, kind, tone, stageName, bilingual copy and target (taskId, action, stage, optional provider/conversationId/groupId/documentRevision/documentHash). Keys bind question group/cycle/answer revision, review stage/cycle/revision/hash, terminal provider/conversation/turn/interaction, error request/turn, playtest cycle/version. Drafts and timestamps never re-key actions.

Tauri-only get_task_notification_settings, set_task_notification_enabled(enabled), test_task_notification manage default-on reminders and report availability/reason/test identity. The explicit test works in foreground and opens settings without task changes. No CLI/MCP or secrets are added.

task-attention-changed carries snapshots. task-notification-activated signals the queue. take_task_notification_activations reads (does not delete) up to 32 {id,target,status} entries; acknowledge_task_notification_activations(ids) removes consumed IDs. Status is current, resolved, missing, unavailable or test. Click restores the app; TasksView checks current questions/doc hashes and attaches only the original terminal. Stale clicks show the latest stage; missing tasks give feedback.

SQLite reserves keys before delivery, including unknown outcomes. Baseline/foreground/disabled/unavailable observations never replay after blur/restart. Each start baselines existing tasks; later tasks participate immediately. Failed reads expose unavailable without generating errors or retiring notices. Resolve/archive/complete/delete/disable withdraw notices. Receipts cascade with deletion; in-memory shown keys still withdraw deleted tasks' notices. History clears on startup/exit. No tray/keep-alive is added.

## Dependencies

Only public task/provider/foundation interfaces. Platform libraries stay in foundation. React consumers use shared adapters to avoid a dependency cycle with development-tasks.

## Validation

Run npm run check, Rust fmt, Clippy and tests. Regressions cover baseline, foreground/disabled/OS-disabled suppression, all tasks, draft/restart dedupe, versioned identities, unknown delivery, read failures, deletion, stale clicks, unobservable Grok and UI lifecycle races. Isolated portable acceptance checks name/icon, foreground/minimized/other-task behavior, toast and Action Center clicks, unchanged decisions, spaces/Chinese paths and test identity cleanup. Never synthesize waits in real AI sessions.

## LLM Maintenance Rule

Update this Skill with identity, suppression, priority, delivery, navigation, settings, tests or lifecycle changes. Notification failures must not alter execution or block guidance.
