# Development Terminal Skill

内存异常上下文只输出活动身份、状态、时间及保守过滤后的摘要。diagnostic_summary
过滤凭据相关内容、原始命令运算符和长值；不复制完整终端或参数。缺失活动与迟到
回执不推定为成功；原工作流仍为操作内容的权威记录。

## Codex-only terminal (2026-10-19)
Grok execution and its module are removed. Retain only Codex terminal routing and
clipboard/IME/history/reconnect behavior. Reject unsupported provider values at
native/API boundaries rather than forwarding old IDs to Codex. Historical timing
may still read grok-conversations without starting, updating or deleting sessions.
Regress stale provider preferences, preserved mounts and Codex empty/full history.

## Context and loaded-history search (2026-10-17)
TerminalContextPanel reads the already-selected conversation workflow/control only.
Keep the original conversation list, provider controls and top workflow strip; this
optional side panel never infers success from text and labels prior execution explicitly.
Insufficient task-pane width/height collapses the optional side panel; explicitly
opening context there uses the existing modal, preserving usable terminal geometry.
Focus/context changes preserve PTY nodes, buffers, IME and independent scroll positions.
History search is user-opened and snapshots the bounded recent DOM history (about
2 MiB), not the complete transcript. Refresh only on explicit request; literal Unicode
search supports case matching, caps results at 500 and renders small text-only excerpts.
It never writes PTY input, changes terminal scroll or reads the system clipboard.
The existing native copy-all action remains the way to obtain earlier full history.
Regress literal hostile text, large match sets, stable snapshots, focus restoration,
provider/conversation identity and unchanged mounting/connection behavior.

## One-click continuation (2026-10-19)
Continue remains available for stopped/failed terminals with an existing native ID.
Create/reuse the durable intent first, attach that same conversation only when the
backend returns needsConnection, then flush the exact request ID after successful
attachment. Keep one synchronous action guard across request, attachment and flush;
never ask the user to type Continue or click it again after a successful attachment.
Attachment failure, timeout or unmount prevents the pending flush and permits an
explicit retry. Ordinary reconnect and passive viewing remain attachment-only.
Native working state still comes from actual task control, not connection success.
Preserve human waits, receipt review and running-turn deduplication in the backend.
Regress stopped/failed attachment, exact request identity, duplicate clicks during
attachment/flush, retry after failure, and question/approval/review/running results.

## IME anchor stability (2026-10-13)
terminalComposition pins the public xterm helper textarea and composition view to
their compositionstart position while a native IME is choosing text. Streaming TUI
cursor movement must not relocate the candidate window. Keep xterm's own input,
composition value and commit pipeline intact; do not buffer output or synthesize
Enter/committed characters. Release after xterm's deferred compositionend commit,
on blur, or on disposal; a new composition cancels the preceding release timer.
Codex install the same guard once per terminal. Validate real xterm
redraw/commit behavior plus rapid next-composition, blur and unmount cleanup.

## Purpose

Provide the shared task-terminal experience and provider-neutral workflow
model used by Codex.

## Ownership

Owns the shared terminal React view, task-local Codex conversation selection,
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

`TerminalProvider` accepts only `codex`. The typed API retains the Codex wire
contracts for availability, conversation CRUD, PTYs, workflows and terminal events.
No provider selector or removed-provider adapter exists. Old UI provider preferences
are ignored unless they equal codex; a task opens its own Codex conversation list.
Switching tasks/tabs/conversations preserves mounted terminals. Task deletion
unmounts that task's terminals before cleanup.
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
works in terminal/history modes and after exit for Codex. The async
copy_terminal_history adapter returns true only after native clipboard success;
false means no text and preserves the clipboard. Disable concurrent clicks and
show bilingual success, empty-history and retryable failure feedback. Read a
fixed file-length snapshot in chunks and strip ANSI/control strings with parser
state across chunk boundaries. Do not send the complete transcript through IPC
or mount it in the DOM. This is terminal output, including provider redraws;
it does not expand tool results hidden by the provider UI. Tests cover multi-MiB
history, Unicode, split escape sequences, empty files, errors and provider routing.

PTY open and live resize clamp columns to 20–500 and rows to 5–200, matching
Codex backend limits. The PTY is not opened until the selected host is
visible and has a non-zero laid-out box. A later resize or visibility change
completes a pending first open. Restoring an already-opened panel only refits;
it does not start a second provider process.

## Dependencies

Depends on foundation contracts and development-task identity. Provider process
construction remains in the Codex terminal modules.

## Validation

Test Codex-only selection and task-scoped conversation retention,
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

## Workflow timing

Timing merges overlapping recorded execution intervals, and subtracts known human-wait, blocked and pause intervals for recorded active progress. It is not pure model time. Unknown/incomplete turns remain counted as unknown; no synthetic completion is written to native history.
Stage events provide total elapsed, first playable, observed review wait and blocked spans; feedback history provides rework and recheck wait. Tools and rework overlap execution and cannot be summed as independent totals.

## Interaction guidance (2026-10-12)
Terminal focus after attachment/Continue/view restoration is conditional on no active
modal. Snapshot terminalInteraction defaults false for old records; native status
notifications map only waitingOnApproval/waitingOnUserInput flags, without storing
request contents. New/completed turns clear the flag. Guidance displays terminal
interaction separately from production document approval and ordinary AI execution.
Failed control polling clears stale state rather than continuing to claim running.

## Original-conversation reconnect (2026-10-13)
Render physical connection from TaskControlState.connection independently of task
state. awaitingContinue means connected with a saved continuation awaiting explicit
user action. Refresh control immediately after open success/failure. Failed Codex
open exposes the complete diagnostic message in an accessible wrapping alert and
an explicit Reconnect original conversation button. Resize/poll must not retry it.
A manual reconnect only attaches the exact ID; it neither creates nor flushes an
intent. Continue task also handles failed attachment via the explicit same-ID flow above.
Validate busy-error visibility, same-ID manual retry and no archive/new-chat calls.

## Conversation recreation (2026-10-13)
Serialize create/delete/archive UI actions with an immediate ref guard and invalidate
pending list responses. Focus refresh cannot auto-create while deletion is pending.
Unmount the deleted terminal view before awaiting backend shutdown. Task/provider
changes clear the guard, and stale action completions cannot overwrite new selection.
Native workflow startup warnings remain visible in the notice row while connected;
they do not disable permission controls or claim that tools have executed successfully.

## Background task notifications (2026-10-14)

ConversationAttention is a public readonly provider-neutral DTO (task/provider/conversation/state/turn/request/interaction). Workflow interactionId is optional with serde defaults for old records; begin/end clear it and snapshot writes signal changes. Codex sets it only on entering a native wait. Original terminal connectionRequest remains attachment-only and never creates a replacement conversation.
