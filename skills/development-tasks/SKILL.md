# Development Tasks Skill

## Current scope

Workflow 2.7: stage_contracts owns effective issue gates for resources/plan. Agent prose cannot create a design gate; actual questions, planner feedback, material hashes and approval validators remain authoritative. category/owningStage and requested blocked operations retain diagnosis history. Explicit compatible upgrade preserves approvals/IDs/history and corrects obsolete technical blocking disposition without marking an issue fixed.
APP-only prototype authorization clones a saved archive to artifacts/prototypes, binds stage/cycle/requirement-plan scope and expires after 30 minutes. Revocation affects future dispatch; no agent grant operation exists. Authoring requires approved requirements/plan or that grant. No policy record and pinned older policies retain compatibility. Fixture/candidate separation and honest implementation convergence remain unchanged.
UI hides programmer export when the filtered programmer issue set is empty. Prototype controls are optional and disclose saved-source copying, expiry and independence from approvals.

## Purpose

Manage persistent development work items and their lifecycle.

## Ownership

Owns task CRUD, task status transitions, task workspace allocation, and
task-to-instance history relationships.

## Public Contracts

Client retention (2026-10-16): pinned_player_paths reads current versionDetails.playerSource
from all persisted task production records, including completed/archived tasks. It performs
no workspace synchronization, exports, hash scans, pin rewrites or acceptance. Fail closed
on unreadable records. Current task pins protect older managed downloads until the task
captures another version or is deleted; prior revision history is not an active pin.
production_capture_version holds foundation EXECUTABLE_FILES shared through hashing and
pin persistence so exclusive package cleanup cannot race a newly captured pin. Verify
read-only behavior, archived pins and release after task deletion.

Production v1.2 adds playerMode (unspecified/single/multiplayer) and durable questionGroups.
Legacy records default to unspecified and no questions. Publishing questions is agent-accessible
in the current stage when questions are enabled, with a validated Desktop-owned conversation binding. User-only
production_answer saves drafts, submits, amends, cancels or changes player mode. Revisions protect
against stale writes and duplicate submission. Prior answers remain immutable; answers are not
document approvals. Changes invalidate downstream stages; existing version resubmission starts
the next cycle. CLI cannot override an already explicit player mode. Pending questions prevent
requirements submission/approval; ask mode also requires an explicit player mode.
APP drafts persist to SQLite plus immediate local browser recovery until successful submission.
TaskIntakeDialog polls the selected task even while its terminal tab is active and opens an APP
modal for a new pending group. Close/Escape/later retains drafts and never submits/cancels.
Dismissed groups have a resume button; production shows summary/reopen entries, not a second
answer editor. New groups and amendment versions can prompt again. Late responses are ignored.
Question groups carry native identity; continuation must target their originating conversation.
Submit and continue is one action. The parent dialog owns submission so a polling rerender or
editor close cannot lose an already accepted intent. Revision checks reject stale views.
Continuation feedback includes queued, paused, waiting gates and needsReview. If the backend
requires a terminal, TasksView connects the exact conversation then flushes the saved request ID;
it does not create a new intent. Document approval/rejection passes its saved production revision
through the same queue; decisions remain saved when connection fails. Required answers are checked before submission
and the first missing question is selected; only revision conflicts instruct the user to refresh.
Editing clears stale validation errors and allows autosave again.
Reports include question and answer history with timestamps. No-followup records explicit
assumptions in the requirement document and does not publish business questions.
Stage report buttons reveal the generated report in the OS file manager, selecting the exact
plan/review/closeout HTML inside the common reports directory. The service accepts only those
three phase IDs and reuses existing safe artifact validation/export. Evidence opening is unchanged.
Requirements, plan and delivery document cards also expose Show in File Explorer above the details.
The action selects the document's recorded path and remains available after approval or task closure.
It uses the existing artifact path validation; missing files show an error without rewriting documents
or changing approval state. It does not invoke a file editor or approve the displayed version.

`DevelopmentTask` includes a durable `workspacePath`. Create/update/archive/
delete commands and task list queries expose that path. New tasks create a
unique, title-derived directory below the configured foundation workspace root.
Legacy tasks with an empty path receive a workspace lazily on first read.

Every created or unpinned task receives provider-specific copies of the bundled
managed Skill at `.codex/skills/abya-game-development-task`. Synchronization is idempotent and
may replace only those exact managed Skill directories. User-owned files and
other Skills under the Codex root are never removed or overwritten. Each
bundled source is resolved from the matching project-root directory in the
development repository or beside the portable executable.

The additional managed template namespace is `abya-task-template-*`. Before a
task is pinned, discover sibling bundled directories with `SKILL.md` and a valid
`abya-task-template.json`: schemaVersion 1, kind `abya-task-template`, matching
id, nonempty displayName, description and sourceSkillName. Copy complete
resources to the matching provider root. Existing destinations are replaceable
only if they have that valid identity marker; an unmarked same-name user Skill
causes a validation error and is preserved. Ignore unmarked source Skills and
invalid template markers. Templates absent from the bundle are not deleted
from existing workspaces. The importer itself is not synced into tasks.

The task Skill reads marker summaries when selecting a relevant template and
includes a recommendation or none in the requirements document. An explicit
selection/decline is reused. Load only the chosen template; it supplements the
shared stages instead of starting another intake. Choices live in task-owned
production records, not new database fields. A scoped request never implicitly
starts the full production workflow or twelve rounds.

Art templates use the separate `abya-art-template-*` namespace and
`abya-art-template.json` marker, kind `abya-art-template`, with the same v1
identity fields, synchronization and collision protection as task templates.
Complete art resources (including images and nested directories) are deployed.
Task and art choices remain distinct. Existing art/resources are preferred;
new style suggestions are confirmed with resources in 2.1 (execution plan in older
pinned policies), without an automatic comic-arcade-ui default. A missing chosen template,
font, asset or tool is a feasibility gap. Art does not change gameplay or
override the multiplayer template's DingTalk font requirement.

The complete managed Skill directory is deployed, including its gameplay
architecture reference and manifest template. During an approved gameplay
implementation the provider writes the concrete manifest to the task-owned
`artifacts/gameplay-architecture.v1.json`; this artifact belongs to the task
workspace and is not persisted in the desktop database or the game archive.
Bot requests additionally load references/bot-development.md and bot-artifacts.md.
The Codex bundle ship versioned plan/report schemas, templates and four local
Node validator modules. Bot plans, definition snapshots and verification evidence
live under artifacts/bot-integration in the task workspace. They do not add task
database fields or automatically mark the task complete. New definitions use a
limited reversible staging step before full architecture validation; existing
definitions are preserved and writes use a single author plus immediate readback.
The local validator checks declared update scope, read-before-write content hashes,
saved archive/Player hashes and evidence-bound gameplay assertions. It does not
execute Lua, establish atomic write locking, authenticate evidence, or replace
runtime acceptance. The multiplayer template uses the same Bot contract.
The bundled Skill requires actual resource import/assignment/readback and
CustomUI contrast/layout acceptance, but no longer defaults to generating all
art. Missing assets are raised in the plan.

Production v1.1 uses SQLite task_production and append-only revision history as
the source of truth. Task-owned JSON/round files/three HTML reports are exports.
Only explicit enablement starts a full task; old artifact records are preserved
as legacy material without inferring acceptance. New-task UI offers full/scoped
mode and ask/no-followup. Scoped tasks keep the existing terminal workflow.
Revision-checked operations submit immutable document versions, issues, evidence,
milestones and rounds. Tauri user decisions bind the displayed document revision
and SHA; no agent/CLI approval operation exists. File changes invalidate approval.
Required issues, missing/stale evidence and incomplete 8+4 prevent stage closure.
Completed task status additionally requires accepted delivery and finished closeout.
Saved versions are hashed from a task-owned instance's known launch archive and
Player executable/Data/runtime files; callers cannot provide arbitrary source paths
or hashes. Required checks revalidate those sources. This is saved-file evidence,
not proof of the currently loaded runtime state; save/reload checks remain required.
Document changes retain history and invalidate affected downstream stages. Reports
escape all user text and percent-encode file links. Artifact paths reject traversal,
alternate streams and symlinks/reparse points outside task-owned files.
Production tasks pin installed Skills and their policy. Ordinary reads no longer
refresh them. Explicit upgrade requires stopped terminals, preserves a backup and
history, detects local managed changes and restarts appropriate confirmations.
The task UI provides stage status, documents/approval/feedback, issues, rounds,
evidence/report entry points and a searchable installed Skill library. It reads
only a selected Skill body and protects against stale async task responses.

The task workspace keeps persistent active, completed, and archived tasks.
Only active tasks may launch new managed instances, and deleting a task is
blocked while any owned process is launching, running, or stopping. External
game sources never belong to tasks. The task instance history table exposes
delete actions through the game-instances module and does not implement
instance deletion rules itself.

Task details expose `游戏实例` and `终端` tabs. Game instances remain the
default tab. A persistent workflow strip sits between the task header and tabs,
showing the selected task conversation's current-turn plan as horizontal nodes
and arrows in both tabs. Each task retains its selected Codex conversation.
Stale workflow reads/events must not replace that selection. Every task status may
read Codex conversations. Switching tabs, tasks or conversations preserves sessions.
Conversation metadata and terminal history remain usable after the desktop
tool restarts, so selecting an existing task conversation does not silently
replace it with a new conversation.
Task deletion unmounts that task's opened terminals first, then runs off the
UI thread. It validates existing game-instance protection, disconnects and
stops all task-owned Codex sessions without sending live terminal
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

Test required titles, state transitions, timestamps, Codex managed
template discovery, resource deployment and refresh, invalid marker filtering,
unmanaged destination collisions, generic fallback and template routing,
Skill deployment, architecture resources, and replacement boundaries, archive behavior, task
deletion protection while instances run, idle-task deletion that keeps the
workspace, frontend unmount of opened terminals before the delete command,
Codex terminal cleanup on successful task deletion without UI-channel
waits, tab and Codex conversation retention, workflow visibility across tabs,
provider-scoped conversation/workflow synchronization, and task-workspace
refresh after instance deletion.
Also compare every deployed Bot schema/reference/module with its bundled source
for new tasks and stale-task refresh, and execute the deployed Node entrypoint to
verify sibling imports. Preserve user-owned Skills and gameplay task artifacts.
The same create/refresh tests compare all production resources byte for
byte, preserving existing task records rather than reinitializing them.
Production tests cover stale revision/hash decisions, no CLI approval bypass,
source/evidence changes, invalid paths, complete twelve-round contracts, history,
escaped reports and pinned task reads. UI tests check explicit approval, stale
documents, task switching and selected-only Skill reads. Synthetic files test
consistency only; they are not real gameplay acceptance evidence.

## LLM Maintenance Rule

### Persistent action guidance (2026-10-12)
The approved split layout places TaskGuidance immediately after the task header,
outside all tab scroll areas. Status and instruction occupy the left column; the
destination and primary action occupy a separate tinted right column. Waiting,
stage action, native terminal interaction, failure and unknown have distinct text,
icons and blue/orange/purple/red/neutral surfaces. Waiting actions stay secondary.
Task-scoped pending counts drive page-tab badges; native terminal requests take
priority over questions and document review. More pending work remains discoverable.
Read-and-review sends the displayed document snapshot to ProductionPanel and opens
its review dialog once, including when mounting from another tab. It never approves.
Only meaningful transitions to human action get one short emphasis animation;
polling does not focus, flash or open a dialog. Reduced motion disables the animation.
Failed/stale reads clear pending actions instead of claiming no user action is needed.
Validate exact-version routing, badge reset, stale task isolation, state tones,
short/narrow layouts and continued terminal visibility with the real TasksView fixture.

When changing task fields, lifecycle rules, persistence, commands, or tests,
update this Skill in the same change.

Runtime image paths in completed CLI activity details are previewable in the existing workflow drawer. Image bytes are read only through a backend path check restricted to the selected task runtime artifact directory.


## On-demand history

Read the matching section in [historical contracts](../../docs/skill-history/20261026/development-tasks.md) only when changing that surface. New current-scope rules override historical versions.

Workflow continuity 2.6 (2026-10-26), Autonomous recovery 2.5 (2026-10-24), v5 workbench (2026-10-21), Approved art feedback recovery (2026-10-20), Game UI design and candidate references (2026-10-20), Reference conversion and optional collage specimen (2026-10-20), Concept-to-runtime calibration (2026-10-20), Codex-only deployment and historical records (2026-10-19), Contextual visual refinement (2026-10-17), Integrated Skill and art library (2026-10-17), Game art traceability and library (2026-10-16), Unified art collaboration (2026-10-16), Readable workspace presentation (2026-10-16), Task description disclosure (2026-10-16), Programmer Markdown export (2026-10-15), Planner art references (2026-10-15), Acceptance UI lifetime (2026-10-14), Programmer issue escalation (2026-10-13), Legacy default workspace repair, Stage workspace update, Workflow 2.0: visual proposals and human feedback, Interaction and art review (2026-10-12), Reconnected task guidance (2026-10-13), Background task notifications (2026-10-14), User-requested AI issue repair (2026-10-13), Atomic goal execution journal (2026-10-22), Resizable workbench panes (2026-10-22), Autonomous implementation 2.4 (2026-10-23).
