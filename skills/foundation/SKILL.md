# Foundation Skill

## Purpose

Provide shared errors, DTO conventions, SQLite access, paths, secret storage,
and command API helpers.

## Ownership

Owns durable storage initialization and shared contracts. It does not own task,
process, log, or MCP business workflows.

## Public Contracts

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
clipboard::read uses Windows clipboard APIs only on explicit terminal paste. Prefer bounded CF_UNICODETEXT (1 MiB UTF-8), otherwise identify bitmap/DIB/PNG without copying image bytes. Never log clipboard contents, monitor clipboard changes or modify clipboard data. Return typed text/image/empty results; close/unlock native handles on exit.

