# Foundation Skill

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
