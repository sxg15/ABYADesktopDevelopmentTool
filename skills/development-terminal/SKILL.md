# Development Terminal Skill

## Purpose

Provide the shared task-terminal experience and provider-neutral workflow
model used by Codex and Grok.

## Ownership

Owns the shared terminal React view, explicit per-task provider selection,
conversation-list presentation, xterm/history scrolling behavior, workflow
DTO implementation, title validation, sanitization, and workflow persistence.

## Public Contracts

TerminalConnectionRequest connects an exact existing conversation requested by intake UI.
It selects/mounts that terminal, waits for successful open/reattach, then resolves once. Missing
or archived conversations fail; never create a replacement. A request waits for real layout,
preserves normal native trust/approval handling and does not itself start a model turn.
Do not send PTY resizes while attachment is unresolved; apply the latest measured size after it
resolves, and focus the visible terminal after attach/Continue. A live connection can still be
stopped while the model state is paused. PTY input/resize adapters use spawn_blocking, allowing
cursor-query replies and UI commands to proceed while Windows terminal operations are blocked.
Historical replay is plain text with CRLF; strip old ANSI queries and modes before xterm parsing.
Only live output may request terminal replies. Preserve raw transcripts on disk for diagnostics.
The pinned portable-pty patch disables Windows parent-cursor inheritance: each embedded terminal
starts independently, without a ConPTY create/close handshake tied to old frontend coordinates.

Codex controls distinguish Pause task, Continue task and Reconnect terminal. Continue goes through
the persistent queue API; reconnection only restores the terminal attachment. Pause cancels pending
intents and interrupts the native turn. useTaskControl polls one backend projection with stale
response guards; the toolbar and workflow bar use it rather than historical TUI text. Inactive
transcripts have an explicit historical label; elapsed time is time since observed progress,
not an inferred networking error. Completed/archived tasks disable Continue until restored instead
of showing ready. TUI closure alone may leave work running. Failed reattachment retains
visible history; replay replacement occurs only after successful open. Project help shows native
ID and workspace and offers scoped history recovery after pause, with metadata backup. It does
not guarantee sidebar enrollment or simultaneous execution from another client. Recovery does
not merge native histories or auto-resume. Refresh list is separate from continue execution.

`TerminalProvider` is `codex` or `grok`. Each selected provider exposes
availability, conversation CRUD, PTY open/write/resize/stop, workflow reads,
and terminal events through typed frontend adapters. The user explicitly
selects the provider with a segmented control; the last choice is retained per
task in local UI state. Switching providers does not stop or unmount an opened
provider terminal. Task deletion unmounts that task's opened terminals first.
An explicit stop command marks the session exited in the UI after the backend
returns because process-tree cleanup no longer emits a live Channel state
event from the command thread.

The shared workflow snapshot contains turns, plans, sanitized activities, and
native/compatibility observability. `workflow.json` is the authoritative latest
projection. `workflow-events.jsonl` is a bounded 8 MiB revision journal whose
schema-v2 records contain only compact revision metadata; it never embeds the
complete projection. Oversized legacy journals use serialized temp-file
replacement with a compaction marker on the next workflow save. Activity
persistence excludes
reasoning text, raw commands, command output, patches, MCP results, and
secret-valued fields.

The xterm buffer and output-history view each retain independent scrolling.
New output follows only when the user is already at the bottom. The UI history
keeps a rolling recent window of at most 2 MiB of normalized text so sustained
PTY redraw traffic cannot exhaust the WebView; the provider transcript remains
persisted on disk.

The toolbar's copy-all action copies the selected conversation's entire persisted
PTY transcript, including output older than replay and UI retention windows. It
works in terminal/history modes and after exit for both providers. The async
copy_terminal_history adapter returns true only after native clipboard success;
false means no text and preserves the clipboard. Disable concurrent clicks and
show bilingual success, empty-history and retryable failure feedback. Read a
fixed file-length snapshot in chunks and strip ANSI/control strings with parser
state across chunk boundaries. Do not send the complete transcript through IPC
or mount it in the DOM. This is terminal output, including provider redraws;
it does not expand tool results hidden by the provider UI. Tests cover multi-MiB
history, Unicode, split escape sequences, empty files, errors and provider routing.

PTY open and live resize clamp columns to 20–500 and rows to 5–200, matching
Codex/Grok backend limits. The PTY is not opened until the selected host is
visible and has a non-zero laid-out box. A later resize or visibility change
completes a pending first open. Restoring an already-opened panel only refits;
it does not start a second provider process.

## Dependencies

Depends on foundation contracts and development-task identity. Provider process
construction remains in the Codex and Grok terminal modules.

## Validation

Test provider selection retention, provider-scoped conversation selection,
mounted-session preservation, deferred PTY open until the host is laid out,
dimension clamping, bounded long-output scrolling,
workflow persistence, compact bounded event-journal migration, title
validation, sanitization, compatibility
warnings, and task-delete unmount of opened terminals before the backend
command.

## LLM Maintenance Rule

When changing shared terminal UI, provider contracts, workflow schemas,
sanitization, or persistence, update this Skill in the same change.

Desktop semantic activity sources are desktopCli and desktopCliReport. Older source names remain displayable in existing journals. Runtime file artifacts are returned through CLI output and inspected by the selected provider.

## Paste and archived conversations
The terminal intercepts Ctrl+V/Shift+V before forwarding keystrokes. Read native clipboard via the typed adapter; text requires bracketed-paste readiness and strips control sequences, images alone forward the provider image-paste key. Right-click paste follows the same routing. Serialized Unicode-safe chunks stay below the backend IPC input limit. No automatic Enter is sent. Register/remove listeners with the terminal lifecycle. Codex rows expose archive/restore and active/archived filters; archived rows never mount a terminal. Empty active lists do not create replacements when archived records exist. Refresh revisions prevent stale reads from undoing an archive. Refocus/manual refresh reconcile native state; failures preserve local history and show errors.


The Codex project-folder icon opens a read-only task path and one-time saved-project instructions for desktop versions that do not show CLI-native projects. The user can select/copy the path; this never accepts an arbitrary cwd.

## Stage workspace update

The existing top conversation steps remain the per-turn plan surface. CLI conversation report can carry a validated plan (1-16 unique named steps, at most one inProgress). Plans preserve activity-linked historical IDs on edits. All-plan-completed does not imply native turn completed. Stage navigation and conversation plans remain separate. Execution settings are displayed above the terminal and use typed provider APIs, with pending edits, failure feedback and explicit next-turn timing.
