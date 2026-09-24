# Development Tasks Skill

## Purpose

Manage persistent development work items and their lifecycle.

## Ownership

Owns task CRUD, task status transitions, task workspace allocation, and
task-to-instance history relationships.

## Public Contracts

`DevelopmentTask` includes a durable `workspacePath`. Create/update/archive/
delete commands and task list queries expose that path. New tasks create a
unique, title-derived directory below the configured foundation workspace root.
Legacy tasks with an empty path receive a workspace lazily on first read.

Every created or read task receives provider-specific copies of the bundled
managed Skill at `.codex/skills/abya-game-development-task` and
`.grok/skills/abya-game-development-task`. Synchronization is idempotent and
may replace only those exact managed Skill directories. User-owned files and
other Skills under either provider root are never removed or overwritten. Each
bundled source is resolved from the matching project-root directory in the
development repository or beside the portable executable.

The additional managed template namespace is `abya-task-template-*`. On each
create/read, discover sibling bundled directories with `SKILL.md` and a valid
`abya-task-template.json`: schemaVersion 1, kind `abya-task-template`, matching
id, nonempty displayName, description and sourceSkillName. Copy complete
resources to the matching provider root. Existing destinations are replaceable
only if they have that valid identity marker; an unmarked same-name user Skill
causes a validation error and is preserved. Ignore unmarked source Skills and
invalid template markers. Templates absent from the bundle are not deleted
from existing workspaces. The importer itself is not synced into tasks.

Before gameplay intake the task Skill reads marker summaries and offers all
templates plus `不使用模板（通用开发流程）`. No templates means the original
workflow. Explicit selection/decline is reused across follow-ups. Load only
the chosen template, use its stages, and retain shared provider binding,
managed-instance, feasibility, authorization, architecture and acceptance
contracts. Selection is conversation/specification context, not a database
field. Already supplied facts and authorization are not requested again.

Art templates use the separate `abya-art-template-*` namespace and
`abya-art-template.json` marker, kind `abya-art-template`, with the same v1
identity fields, synchronization and collision protection as task templates.
Complete art resources (including images and nested directories) are deployed.
They never appear in the initial task-template menu. After reporting
feasibility, offer use/no-art-template; a use response without another named
style selects `abya-art-template-comic-arcade-ui`. Multiplayer also recommends
this default, while still allowing decline or another style. Wait for an
actual choice unless the user already supplied one. Persist the choice in the
conversation/specification and reuse it, not in a database field. No installed
styles means continue without one; a missing requested/default style must be
reported, never silently replaced. The selected art template governs visual
design and asset reuse, not gameplay, and cannot override the multiplayer
template's required DingTalk font. Additional style constraints can require
updating the feasibility result before implementation.

The complete managed Skill directory is deployed, including its gameplay
architecture reference and manifest template. During an approved gameplay
implementation the provider writes the concrete manifest to the task-owned
`artifacts/gameplay-architecture.v1.json`; this artifact belongs to the task
workspace and is not persisted in the desktop database or the game archive.
The bundled Skill also makes agent-created, imported, and assigned gameplay
assets the default when the user does not provide or require existing assets,
and requires CustomUI text contrast plus parent-contained layout acceptance.

The task workspace keeps persistent active, completed, and archived tasks.
Only active tasks may launch new managed instances, and deleting a task is
blocked while any owned process is launching, running, or stopping. External
game sources never belong to tasks. The task instance history table exposes
delete actions through the game-instances module and does not implement
instance deletion rules itself.

Task details expose `游戏实例` and `终端` tabs. Game instances remain the
default tab. A persistent workflow strip sits between the task header and tabs,
showing the selected task conversation's current-turn plan as horizontal nodes
and arrows in both tabs. Hover/focus exposes recent step activity and clicking
a node opens the complete timeline drawer. Each task retains its explicitly
selected Codex or Grok provider and the last selected conversation for each
provider while switching tasks. Stale workflow reads or terminal events must
not replace that selection. Every task status may open either provider's
conversations. Switching tabs, tasks, providers, or conversations does not stop
existing sessions.
Conversation metadata and terminal history remain usable after the desktop
tool restarts, so selecting an existing task conversation does not silently
replace it with a new conversation.
Task deletion unmounts that task's opened terminals first, then runs off the
UI thread. It validates existing game-instance protection, disconnects and
stops all task-owned Codex and Grok sessions without sending live terminal
events, and only then deletes the durable task record. The task workspace is
intentionally left on disk so source files and conversation transcripts are
not destroyed by deleting the database record. Session stop drops the UI
channel and PTY handles before process-tree termination so ConPTY output
cannot block webview IPC. Windows `taskkill` waits at most two seconds
before falling back to the PTY killer.

Desktop CLI exposes list, create, update, and status operations by delegating to
`TaskService`. MCP status filtering is an orchestration concern and does not
change task persistence rules.

## Dependencies

Depends only on foundation. It must not launch processes or parse logs.

## Validation

Test required titles, state transitions, timestamps, both-provider managed
template discovery, resource deployment and refresh, invalid marker filtering,
unmanaged destination collisions, generic fallback and template routing,
Skill deployment, architecture resources, and replacement boundaries, archive behavior, task
deletion protection while instances run, idle-task deletion that keeps the
workspace, frontend unmount of opened terminals before the delete command,
both-provider terminal cleanup on successful task deletion without UI-channel
waits, tab and provider selection retention, workflow visibility across tabs,
provider-scoped conversation/workflow synchronization, and task-workspace
refresh after instance deletion.

## LLM Maintenance Rule

When changing task fields, lifecycle rules, persistence, commands, or tests,
update this Skill in the same change.

Runtime image paths in completed CLI activity details are previewable in the existing workflow drawer. Image bytes are read only through a backend path check restricted to the selected task runtime artifact directory.

## Legacy default workspace repair
prepare_terminal_workspace runs before a new provider terminal opens. Only direct children of the old LOCALAPPDATA default root qualify; active session leases prevent migration. Copy into a unique staging directory under the new USERPROFILE default root, reject reparse points, then rename and conditionally update the database path. Keep the original directory as a recovery copy. Conflicts/errors preserve both source and any staging data for inspection; never overwrite or recursively delete user content. Task/conversation IDs and native session metadata remain unchanged. Repeated opens are idempotent. Test binary assets, conversation preservation, custom roots and database path updates.

