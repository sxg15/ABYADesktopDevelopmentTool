# Codex Terminal Skill

## Purpose

Provide embedded interactive Codex CLI terminals for multiple conversations
within each development task.

## Ownership

Owns Codex CLI discovery, Windows PTY creation, terminal transport, task
conversation files, per-conversation live sessions, process-tree termination,
authenticated loopback Codex app-server lifecycle, and native event mapping
into the shared development-terminal workflow model.

## Public Contracts

`CodexTerminalAvailability`, `CodexConversation`, `CodexTerminalState`,
`CodexTerminalStatus`, `CodexTerminalEvent`, conversation list/create/delete
commands, conversation rename, `CodexWorkflowSnapshot`, workflow read, and the
open, write, resize, and stop Tauri commands.

The terminal resolves `codex.exe`, `codex.com`, `codex.cmd`, or `codex.bat`
from the desktop process environment, the current Windows user and system
registry `PATH` values, and the standard `%APPDATA%\npm` global package
directory. The persisted and standard-directory fallbacks allow an
Explorer-launched desktop process to discover Codex installed after Explorer
started. Command scripts run through `%ComSpec%`; native executables run
directly. Both forms receive the task workspace through `--cd <task workspace>`,
use it as their process current directory, and receive `--no-alt-screen` so
xterm can retain scrollback. The PTY advertises `TERM=xterm-256color` and
true-color support so Codex does not enter dumb terminal mode. The UI never
accepts a command, executable, shell type, working directory, or raw Codex
arguments.

Each visible Codex TUI process receives `ABYA_DEVELOPMENT_TASK_ID`,
`ABYA_DEVELOPMENT_CONVERSATION_ID`, `ABYA_DEVELOPMENT_WORKSPACE`, and
`ABYA_DEVELOPMENT_PROVIDER=codex`. Task-local
Skills use these values to restrict managed game-instance operations to the
current development task and associate CLI activity to the current
conversation.

Each conversation has a directory below
`<task workspace>/conversations/<conversation-id>/`, with metadata in
`conversation.json`, output-only PTY history in `transcript.log`, the latest
workflow projection in `workflow.json`, and the shared bounded compact-revision
`workflow-events.jsonl` journal. Conversation titles are trimmed, required, limited to
100 characters, and rename in place without replacing the native session.
Metadata also keeps the native Codex session ID when it is available. Each
conversation may own one live session regardless of task status. Opening an
existing live session replaces its output subscriber instead of starting
another process; opening a stopped or restarted conversation replays its saved
transcript and uses `codex resume <session-id>` when the native session ID is
known. Legacy metadata is backfilled when the conversation list is read by
matching native sessions in the same task workspace one-to-one by creation
time, with a single-candidate legacy fallback; opening also performs the same
recovery.
Sessions remain in memory, survive view and task switching, and stop on
explicit user action, successful task deletion, or application exit.
Stop and task deletion remove the live session, drop its UI Channel and PTY
handles, then terminate the process tree without sending terminal events from
the command thread. Windows `taskkill /T /F` waits at most two seconds
before falling back to the PTY killer. Delete-task, stop, and conversation
delete commands run off the UI thread.
Deleting one conversation also removes its native app-server thread binding
before its directory is deleted, so delayed notifications cannot recreate
deleted workflow files.
Codex authentication and configuration remain owned by the installed Codex CLI
and must never be read, returned, copied, or persisted in task workspaces. The
desktop metadata and transcript are the local conversation index and terminal
replay while native authenticated session state remains in the normal user
state directory.

When supported, one hidden Codex app-server binds to a random loopback
WebSocket port with a temporary capability-token file. The visible TUI connects
through `--remote` while retaining its PTY presentation. Thread start/resume
receives developer instructions requiring a per-user-turn plan before
multi-operation work and CLI diagnosis and capability discovery before domain operations. A new native thread receives the same instructions through
`thread/inject_items` before the TUI resumes it; this creates its durable rollout
without a hidden model turn, so a newly created conversation is immediately
recoverable after application restart. Native `turn/started`,
`turn/plan/updated`, `item/started`,
`item/completed`, and `turn/completed` notifications update the workflow
snapshot and live terminal channel. Commands, file changes, MCP calls, web
activity, tests, game-instance actions, and agents are attached to the current
in-progress plan step. Activity before a plan is still allowed but is placed
under an explicit unplanned step. Workflow writes are serialized.

Workflow detail is whitelist-based. It may contain file paths, file-change
kinds, MCP tool/server names, sanitized arguments, web-search metadata, and
agent lifecycle IDs. It must omit reasoning text, raw command strings, command
output, patch bodies, MCP results, credentials, and secret-valued fields. If
app-server is unavailable or fails, the normal PTY remains available and the
workflow reports compatibility observability with a visible warning.

The xterm viewport retains 100,000 lines of scrollback and exposes a persistent
vertical scrollbar. Mouse-wheel scrolling and dragging the scrollbar operate
on xterm's own buffer; incoming output preserves a user's non-bottom position,
and an icon action returns to the latest output without changing the PTY row
calculation or discarding the live terminal session. Codex full-screen ANSI
redraws can still replace the visible xterm screen, so the terminal also
provides an output-history view built from persisted and live output. That view
has its own scrollbar, preserves a user's historical position while output
continues, and retains a rolling recent 2 MiB window. The complete transcript
remains on disk, while terminal attachment replays at most its recent 256 KiB in
64 KiB events and reports when older bytes were omitted. Reattaching a live
session serializes replay before the new subscriber receives live output so
history and current output cannot interleave.

## Dependencies

Depends on foundation error and DTO conventions, development-task identity,
and the development-terminal workflow model. It must not depend on game
instances, logs, runtime MCP, desktop MCP, or settings.

## Validation

Test process and persisted PATH/PATHEXT resolution, stale Explorer environment
fallback through `%APPDATA%\npm`, `.cmd` launch through ConPTY, fixed working
directory, task/conversation identity environment variables, native session
matching and resume arguments, durable new-thread instruction injection, title
validation and rename, bounded transcript-tail replay and live reattachment
ordering, app-server capability/authentication, native event mapping,
serialized Windows snapshot replacement, bounded legacy event-journal compaction,
unplanned-operation handling,
workflow redaction, Desktop CLI activity correlation, all task statuses,
per-task uniqueness, Unicode and ANSI output, visible long-output scrolling
while output continues, natural exit, restart, explicit stop, task deletion without UI-channel waits,
bounded process-tree cleanup, and application shutdown.

## LLM Maintenance Rule

When changing Codex discovery, arguments, PTY behavior, terminal transport,
session lifecycle, process cleanup, or tests, update this Skill in the same
change.

ABYA_DESKTOP_CLI carries the absolute desktop CLI executable path. Commands automatically include task, conversation and provider context. ABYA operations use CLI exclusively; native provider transport remains unchanged.

The cached app-server records the resolved executable path, size and modification time before launch and checks process liveness before reuse. After a CLI update or backend exit, an idle cached backend is stopped and recreated; thread routing is rebuilt on resume. Other live terminals are not interrupted: new opens use the existing compatibility path with a restart explanation until those terminals are stopped. Backend creation is serialized to avoid duplicate instances.

Remote thread/start and thread/resume also receive the five public ABYA context variables through per-thread dotted shell_environment_policy.set overrides. This is required because the shared app-server executes tools in a different process from the TUI. It never sets task IDs globally on the shared backend, never changes sandbox/approval/trust/model settings, and preserves unrelated environment overrides. New and resumed conversations use the same builder.

## Sandbox-compatible CLI startup
Before opening a stopped terminal, ask TaskService to repair the former default workspace; preserve the native session ID and pass its new cwd on resume. Run a hidden, bounded Codex sandbox doctor preflight with ordinary configured permissions. Distinguish error 267 (workspace_bootstrap_failed), timeout and connection failure; never start the terminal after failed preflight. Thread start/resume and TUI environment receive ABYA_DESKTOP_PIPE and ABYA_DESKTOP_SESSION_TOKEN in addition to the five public context values. Credentials expire after 12 hours; reopen to renew. Explicit stop and natural terminal exit revoke the session. No approval, trust, login or sandbox policy overrides are introduced.


ABYA_DESKTOP_PID is injected alongside the pipe/session token so the CLI can authenticate the pipe server before sending credentials.

## Native project and archive lifecycle
Codex 0.156.1 experimental project/list/create/update and thread/metadata/update register one project per task, using stable idempotency and abyaTaskId metadata. Reuse matching roots, update only ABYA-owned project names/roots, and assign only locally recorded native IDs. Name conversations ABYA · task · conversation, avoiding redundant writes. Serialized open/rename/archive/delete operations prevent resurrection races. Archival interrupts an active owned turn, stops the PTY/revokes CLI access, unsubscribes, then calls thread/archive; persist local archived=true only on success. Restore calls thread/unarchive only when needed, retaining the same native ID. Deleting local history first archives the native thread; never silently leave active orphan histories. Refresh reconciles external archive/restore. Empty native threads may be absent from thread/list: thread/read's archived field, or the verified sessions/archived_sessions layout in its current unstable path field, is authoritative. Unknown/missing layouts fail closed. Native sync errors remain visible alongside the local list. Live test: real_terminal_archive_restore_and_project_registration (opt-in, ABYA_TEST_CODEX). No model or game turn is needed.


The current Codex desktop saved-project list may not expose projects registered by CLI app-server. The project_workspace query returns the task-owned folder for a one-time add-project guide; do not claim native registration guarantees desktop sidebar enrollment.
