# Grok Terminal Skill

## Purpose

Provide embedded interactive Grok CLI terminals with durable task-local
conversation indexes and real-time plan visibility.

## Ownership

Owns Grok CLI discovery, PTY creation, fixed launch arguments, terminal
transport, process-tree cleanup, Grok conversation files, native UUID session
creation/resume, and ACP session-update observation.

## Public Contracts

Grok discovery checks process and persisted Windows PATH/PATHEXT plus
`%USERPROFILE%\.grok\bin`. The UI never accepts an executable, shell, working
directory, command, or raw argument. Grok starts in the task workspace with
`--cwd`, `--no-alt-screen`, and `--minimal`.

Each conversation is stored below
`<task workspace>/grok-conversations/<conversation-id>/` with
`conversation.json`, `transcript.log`, `workflow.json`, and the shared bounded
compact-revision `workflow-events.jsonl` journal. New conversations pass their UUID through
`--session-id`; existing native sessions resume through `--resume <UUID>`.
Authentication, configuration, and native session history remain in the
normal user `.grok` directory.

The complete PTY transcript remains on disk. Terminal attachment replays only
the recent 256 KiB in 64 KiB events and reports omitted older bytes; reattaching
a live session serializes this replay before the replacement subscriber starts
receiving live output.

Stop and task deletion remove the live session, drop its UI Channel and PTY
handles, then terminate the process tree without sending terminal events from
the command thread. Windows `taskkill /T /F` waits at most two seconds
before falling back to the PTY killer. Delete-task, stop, and conversation
delete commands run off the UI thread.

The process receives `ABYA_DEVELOPMENT_PROVIDER=grok`, task, conversation, and
workspace environment variables plus managed rules requiring a plan before
multi-operation work. A background watcher locates the native session's
`updates.jsonl` and maps ACP `plan`, `tool_call`, `tool_call_update`, and
`turn_completed` events into the shared workflow. Raw commands, output, patch
bodies, reasoning, and secrets are excluded. Missing or incompatible ACP
updates degrade to an explicit compatibility warning while the PTY remains
usable.

Every task workspace contains the bundled Grok-native
`.grok/skills/abya-game-development-task/SKILL.md` and its references. Grok
discovers it from the task working directory. The Skill explicitly binds
Desktop MCP activity with `provider: "grok"` and requires native execution-plan
updates without forcing formal read-only plan mode.

## Dependencies

Depends on foundation, development tasks, and the shared development-terminal
workflow model. It does not depend on Codex, game instances, logs, Runtime MCP,
or Desktop MCP.

## Validation

Test PATH and standard-directory discovery, native executable preference,
fixed workspace and arguments, UUID creation/resume, bounded transcript-tail
replay and live reattachment ordering,
ACP plan/tool/turn mapping, bounded legacy event-journal compaction, redaction,
provider environment variables, all
task statuses, bundled Grok Skill discovery/deployment, resize/input limits,
natural exit, explicit stop, task deletion without UI-channel waits, bounded
process-tree cleanup, and application shutdown.

## LLM Maintenance Rule

When changing Grok discovery, launch flags, sessions, ACP mapping, PTY
behavior, cleanup, or tests, update this Skill in the same change.
