# Development Terminal Skill

## Purpose

Provide the shared task-terminal experience and provider-neutral workflow
model used by Codex and Grok.

## Ownership

Owns the shared terminal React view, explicit per-task provider selection,
conversation-list presentation, xterm/history scrolling behavior, workflow
DTO implementation, title validation, sanitization, and workflow persistence.

## Public Contracts

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
