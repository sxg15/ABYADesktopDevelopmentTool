# Foundation Skill

## Game client selection (2026-10-14)

AppSettings/StoredSettings add defaulted gameClient source (local/remote), local path,
installation ID, relative EXE and storage root. Legacy settings retain their local EXE.
PublicSettingsUpdate groups the public settings input; the game-clients service validates
and resolves remote installations before the adapter calls SettingsService::update_public.
gameExecutablePath remains the effective launch path for existing consumers. Typed
gameClients adapters expose only sanitized account metadata and installation/operation DTOs.
secrets::protect/unprotect provides purpose-separated current-user DPAPI; GitHub tokens
are never included in AppSettings, exports, native command results or frontend storage.

## Purpose

Provide shared errors, DTO conventions, SQLite access, paths, secret storage,
and command API helpers.

## Ownership

Owns durable storage initialization and shared contracts. It does not own task,
process, log, or MCP business workflows.

## Public Contracts

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

Shared TypeScript command adapters carry `TerminalProvider` (`codex` or
`grok`) plus typed provider availability, state, event, input, resize,
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
