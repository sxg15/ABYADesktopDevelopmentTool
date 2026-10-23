# Foundation Skill

## Terminal freeze recovery (2026-10-22)
CodexTerminalState exposes an ephemeral inputSessionId. write_codex_terminal requires that exact PTY session identity, rejecting delayed calls after replacement before writing bytes.
rebuild_codex_terminal is an APP-only spawn_blocking adapter to the Codex service. It preserves the task/native binding and cannot replay input or resume inference. terminalApi.rebuild uses the existing command boundary; no credentials or raw terminal input are logged.

## System memory warning (2026-10-22)
MemoryStatus adds defaulted warning text. System RAM/commit headroom now warns independently of pressured; per-game protection remains intact.

## Workbench adapters (2026-10-21)
APP-only mutate_production_workbench, read_production_snapshot and export_developer_package delegate to TaskService. Shared record adds optional workbench goals/drafts/requests/deferrals. No schema migration or raw shell input. Legacy records default safely; read-only snapshot access validates the task and revision.

## Art approval and recheck projections (2026-10-20)
Shared ProductionView adds optional approvalStatuses reason map (empty = intact
accepted current document) and actions.resume reasons. APP-only feedback operation
resume-art delegates to TaskService with expectedRevision; it cannot add an approval
or close feedback and has no agent/CLI equivalent. AttentionTarget adds optional
feedbackId/candidateArtifactId and action recheck; TaskAttention kind recheck shares
human review badges. Legacy views/receipts remain readable. Domain validation and
restoration stay in tasks; notification projection/navigation stays in notifications
and the tasks UI. No database migration, credential or new transport is introduced.

force_stop_codex_conversation(taskId, conversationId) is an APP-only adapter to the
provider's ownership-checked force_stop; cancel scoped bridge operations and run blocking
cleanup on spawn_blocking. stop_codex_terminal also uses spawn_blocking.
TaskControlState.executionControl is additive, distinct from queue state, with sanitized
operation/target/result metadata; stopFailed never means native execution has stopped.
No CLI/MCP capability or credential exposure is introduced.

interruptCodexTurn(taskId, conversationId) and terminalApi.interrupt route only Codex
to the APP-only interrupt_codex_turn command. The adapter validates task binding,
carries blocking native interruption on spawn_blocking to keep IPC responsive,
cancels active runtime-bridge operations for that conversation and delegates native
turn interruption. This preserves scoped access for future input and creates no CLI
or MCP capability. TaskControlState may project awaitingInput after interruption.

## 内存保护适配接口（2026-10-19）
共享 memory.ts 提供实例采样、阈值、单实例自动停止授权和报告入口。策略与授权仅为
Tauri 用户操作，CLI 的 game_instance_get 只得到 InstanceRuntimeInfo.memory 投影。
没有 raw command/path 输入；报告打开只能由 instances 服务解析固定数据目录中的路径。
ABYA_TEST_MODE=1 与 Rust 测试一样使用数据目录内 IsolatedGameData 启动测试 Player；
正常发布运行不修改游戏数据根。日志字段区分私有提交/工作集/系统提交，不能混称物理内存。

## Codex-only provider contract (2026-10-19)
The active terminal provider type accepts only codex. Shared adapters expose no
Grok commands. Persisted intake question provenance remains a string to preserve
old records; it is not authorization to start a provider. Removed provider values
are rejected by terminal/CLI dispatch, never silently converted into Codex IDs.
No task/account database or credential files are deleted for this retirement.

## Workspace library contracts (2026-10-17)
ArtDesign defaults an additive caseRequests array; older records remain readable.
artLibraryApi.request uses the existing APP-only mutate_art_library adapter and returns
the saved production revision and fixed domain prompt. No new transport, approval API
or credential is introduced. TaskSkill kind=design uses the existing id/path/hash fields
for exact nested resources; task pinning still owns the parent workflow tree.

## Art design contracts (2026-10-16)
Additive art_library_cases stores immutable versioned curated case bodies and separately
managed availability, without a task foreign-key cascade. The tasks module owns all
case/domain rules and checked media snapshots. Production DTOs add defaulted artDesign,
document artDesignHash and optional view artDesignStatus; legacy records remain readable.
Shared artDesign adapters expose typed Tauri list/read/media/user-only library actions.
Validate database creation/reopen, legacy JSON and task-deletion independence.

ProductionView.actions adds optional recheck reasons keyed by visual feedback ID.
Empty reason means the current candidate is eligible for user recheck; submission
still revalidates task revision and candidate/files in TaskService. No new command,
table, credential or approval authority is added.
ProductionFeedback optionally carries reopenReason. APP feedback reopen accepts an
reason up to 2000 characters (required for art reopen); task ownership and revision checks still apply.

productionApi.exportProgrammerIssues(taskId, expectedRevision, issueIds) invokes
the APP-only export_programmer_issues adapter and returns the saved relative .md
path. TaskService owns snapshot selection/validation/rendering/file writes; the
adapter delegates blocking work. Existing revealDocument selects the saved file.
No database schema, desktop CLI/MCP command or provider session changes are added.

ProductionArtifact adds optional plannerReference. APP-only productionApi.feedback
accepts attach-art-reference (name/base64/summary) and remove-art-reference (id).
The task module validates and persists inputs; old records need no migration.

ProductionIssue optionally carries repairTarget (abya-source, abya-build, lua, other)
for programmer handoff display. Legacy records remain readable without it. Ownership
classification and validation remain in development-tasks; no database migration.

## Game client selection (2026-10-14)

2026-10-16: typed adapters add APP-only activate/discardPending/maintain and retention
reason DTOs. ClientUpdate includes a candidate key, installable assets and optional
preferred asset ID for a direct confirmation dialog. Activation saves fresh client
fields while preserving other settings and the previous local path. Public
EXECUTABLE_FILES is an in-process RwLock: instance launch and task version capture
hold shared access; client cleanup/settings activation take exclusive access. Settings
updates hold shared access across remote resolution and saving. It coordinates file
ownership only; foundation does not select packages, inspect task pins or delete files.
SettingsService clones share a reentrant gate across get/save and each read-modify-save
operation, including token rotation and update_game_client. This prevents get's legacy
normalization writeback from overwriting a concurrent client selection. Client-only
updates preserve the freshly read local fallback and unrelated settings; callers still
own installation validation. Validate concurrent activation/token rotation without loss.

AppSettings/StoredSettings add defaulted gameClient source (local/remote), local path,
installation ID, relative EXE and storage root. Legacy settings retain their local EXE.
PublicSettingsUpdate groups the public settings input; the game-clients service validates
and resolves remote installations before the adapter calls SettingsService::update_public.
gameExecutablePath remains the effective launch path for existing consumers. Typed
gameClients adapters expose only sanitized account metadata and installation/operation DTOs.
Map clientDownloadResponse to a dedicated localized non-file response error, distinct from
clientIntegrity. Neither error exposes response bodies, credentials or signed download URLs.
OAuth start/poll/cancel adapters accept no credentials; login DTOs contain only the opaque
attempt ID, one-time user code, fixed verification URL, expiry and polling interval.
secrets::protect/unprotect provides purpose-separated current-user DPAPI; GitHub access/refresh tokens
are never included in AppSettings, exports, native command results or frontend storage.

## Purpose

Provide shared errors, DTO conventions, SQLite access, paths, secret storage,
and command API helpers.

## Ownership

Owns durable storage initialization and shared contracts. It does not own task,
process, log, or MCP business workflows.

## Public Contracts

The standalone CLI HTTP transport explicitly disables proxies: its validated 127.0.0.1
endpoint and bearer token must stay on the local connection even when the application
enables reqwest system-proxy support for remote GitHub requests. Named pipes are unchanged.

Shared adapters expose typed intake groups/answer versions/player mode, APP-only answer actions,
native-bound Codex continuation, and scoped conversation recovery. They never transport session
credentials. Intake answers and approvals remain separate domain actions; business logic stays
inside task and provider services.
IntakeContinuation includes queued, paused, waitingForAnswers, waitingForApproval, needsReview,
needsConnection, started, resumed, running, completed and superseded, with request/native/turn IDs.
TaskControlState separates connection, native execution, queue, human gates and last event time.
BuildInfo validates the embedded release/version against the adjacent manifest and executable hash.
Before database initialization, normal launches from the OpenAI.Codex Windows package are brokered
through Explorer to avoid virtualized app data. ABYA_TEST_MODE explicitly permits isolated fixtures.
A per-user canonical-data-directory Global mutex prevents concurrent APP owners and focuses the
existing PID only after checking process creation time. Separate test directories remain independent.
productionApi.revealReport sends a task ID and a fixed report phase to the native adapter;
path validation and export remain in TaskService, and the adapter reveals the file in its folder.
productionApi.revealDocument calls open_production_artifact with optional reveal=true, selecting a
validated task artifact in the OS file manager. Omitted/false retains normal artifact opening.

`AppError`, database access, application paths, settings values, and shared
serialization helpers. Settings include the game gateway port, LAN broadcast
switch, preferred adapter ID, and persistent discovery tool ID.

Shared TypeScript command adapters carry `TerminalProvider` (`codex` only) plus typed provider availability, state, event, input, resize,
conversation, and workspace contracts. They expose provider-specific commands
through one typed terminal API while preserving the existing Codex wire
contracts. Workflow turns, plan steps, activities, observability, rename, and
workflow reads are shared. Settings persist a user-selected task workspace
root and expose the default root through `AppPaths`. Foundation does not
discover a provider, create PTYs, map native events, or own terminal/workflow
lifecycle.

Shared instance contracts include the `background` and `visible` window
visibility modes used by Tauri commands and React features. Business rules and
Windows HWND control remain owned by `game-instances`.

Durable data lives below
`%LOCALAPPDATA%\ABYA Desktop Development Tool\Data`. Desktop CLI secrets are encrypted for the current Windows user with DPAPI. Legacy settings are read without reusing the old token, then saved in the new schema. Public settings never serialize the CLI token. cli_environment publishes an encrypted connection descriptor and resolves the bundled CLI/Node paths. The versioned runtime-table
migration preserves existing task, instance, log-session, and event IDs while
generalizing instances into managed and external origins with separate process
and connection states. `AppPaths.archive_transfers_dir` stores temporary
desktop ZIP packages; durable `archive_transfers` rows store status and
progress. Active transfer rows are marked interrupted on restart.

SQLite uses a 1,000-page WAL auto-checkpoint, a 64 MiB retained-journal limit,
startup checkpoint truncation, and incremental auto-vacuum for new databases.
Small legacy databases and legacy databases with at least 256 MiB and 25% free
pages are migrated through a one-time vacuum; later startup maintenance
reclaims incremental-vacuum free pages without changing durable row IDs.

## Dependencies

May depend on platform and persistence libraries. Product modules may depend on
foundation; foundation must not depend on product modules.

## Validation

Additive task_production and task_production_history tables retain authoritative
versioned production JSON and prior revisions, with task-ID foreign keys. Existing
task IDs and archives are not migrated or rewritten. Domain transitions stay in
TaskService; shared TypeScript production adapters expose typed read/mutate/user
decision, Skill browsing and artifact-opening commands. JSON exports do not import
themselves back into the database.

Run Rust unit tests, migration and storage-maintenance tests, TypeScript contract checks, and
`npm run check:skills`, including workflow DTO serialization compatibility.

## LLM Maintenance Rule

The public `repair_storage` operation performs an explicit checkpoint, VACUUM,
and `quick_check`, returning before/after byte and WAL metrics for the UI.

When changing foundation behavior, schemas, contracts, paths, or dependencies,
update this Skill in the same change.

## Managed sandbox transport
cli_sessions owns in-memory 12-hour capabilities bound to provider/task/conversation. Never serialize them into ABYA settings, workspace files or logs. cli_transport selects the managed pipe when its session environment is present and never falls back after a pipe failure. Same-user standalone access retains DPAPI HTTP. The default workspace is USERPROFILE/ABYA Desktop Development ToolWorkspaces; settings normalize only the exact former default root.


## User-initiated clipboard access
clipboard::read uses Windows clipboard APIs only on explicit terminal paste. Prefer bounded CF_UNICODETEXT (1 MiB UTF-8), otherwise identify bitmap/DIB/PNG without copying image bytes. Never log clipboard contents or monitor clipboard changes. Return typed text/image/empty results; close/unlock native handles on exit.

clipboard::write_text is only called for explicit copy-all history. Use the
invoking desktop window as clipboard owner, allocate null-terminated Unicode
text before opening/emptying the clipboard, retry temporary contention, and
release handles on failure. Successful SetClipboardData transfers allocation
ownership to Windows. Copy has no paste-size/replay-tail limit. The typed
terminalApi.copyHistory sends provider/task/conversation IDs and returns a bool;
it never transports the transcript to the WebView.


## Stage workspace update

ProductionView adds effective stageStatuses and registered/legacy artifacts; production records default additive artifacts, stageUpdates and stage transition events. Question groups default to requirements and carry cycle. ExecutionSettingsView belongs to shared types; typed native adapters read/update per-conversation model, effort and permissions without credentials.

## Workflow 2.0 shared contracts

Production DTOs add optional acceptanceConfig/acceptanceSessions, selfTests, feedback, visual artifact metadata and document artifactBindings. Serde defaults preserve 1.x records without creating approvals or test results.
Typed frontend adapters expose acceptance start/status, registered media reads and APP-only feedback. Domain rules remain in owning services; no new database table or secret persistence is introduced.
Timing adds elapsed/first-playable/human wait/blocked/feedback rework/recorded active progress; categories overlap and incomplete execution remains unknown.

## Review availability contracts (2026-10-12)
ProductionView exposes additive actions maps (review/approve/feedback), keyed by
stage or artifact ID/gameplay. Empty reason means preliminarily available; missing
entries fail closed. Full file/version/gate checks still run at submission.
Workflow snapshots add optional terminalInteraction; control state adds waitingForTerminal.
Document artScope is additive, preserving old serialized records. No new secrets or tables.

## Background task notifications (2026-10-14)

Foundation exposes content-free change_signal revisions and typed TaskAttention DTO/React subscriptions. The additive task_notification_receipts table stores key/target/disposition/active state with task FK cascade; no task content or credentials. SettingsService::locale reads locale alone without secret decoding. notifications::NotificationBackend encapsulates native ToastGeneric, user-scoped AUMID/CLSID registration, Start Menu shortcut with AUMID/ToastActivatorCLSID, foreground PID check and COM activation on one MTA worker. windows/windows-core 0.62.2 provide WinRT/COM; winreg owns HKCU. Payload is bounded escaped text plus opaque SHA key only; system audio/duration/DND apply. Debug or ABYA_TEST_MODE identities include a data-directory hash; test COM cold activation exits before real data is opened. Normal shutdown revokes COM, clears history and removes test registration. No installer, tray or shell command is used.
A registration guard also cleans partially initialized test AUMID/CLSID/shortcut resources on native initialization failure.
Fresh unpackaged AUMIDs return ERROR_NOT_FOUND from ToastNotifier.Setting until first registration. Preregister only for 0x80070490 using a suppressed, silent, 15-second-expiry toast and immediately remove it, following the Windows Community Toolkit behavior. Actual task notifications keep default system audio/duration. Other HRESULTs are reported without masking them. The opt-in isolated_windows_toast_registration_history_and_com test validates actual WinRT history and COM activation; ABYA_TEST_WINDOWS_TOAST=1 is required. Release COM references before RoUninitialize.

productionApi.repairIssue invokes repair_codex_issue with taskId, conversationId,
issueId and expectedRevision. It returns the existing IntakeContinuation DTO; no
arbitrary prompt/command input or new credential surface. Task/provider services own
validation, prompt construction, queue and receipts; Tauri remains an adapter.

## Execution ownership contracts (2026-10-22)
TaskControlState and CodexTerminalState expose generation; force_stop_codex_conversation
requires expectedGeneration. resume_workbench_goal accepts exact task/goal/conversation,
expectedRevision, resumeIntentId and optional reconciliationNote. reconcile_task_execution
is an explicit mutation coordinator, separate from readonly get_task_production.
Continuation requests/production attempts retain exact IDs across restart. Scoped CLI
lease revocation also blocks reissue by stale workers; only a fresh authorized session
may release that fence. Other conversations retain independent credentials.

## Autonomous implementation presentation (2026-10-23)
Shared DTOs expose policy implementationExecution and workbench.execution (units, state,
events) plus issue executionHandling. workSettled is execution convergence, not acceptance.
The UI retains issue severity/history, shows current route and deferred dependencies,
and does not route autonomous PB issues to repeated manual repair. No new credential fields.
