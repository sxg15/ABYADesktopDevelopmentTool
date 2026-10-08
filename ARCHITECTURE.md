# Architecture

The application uses capability modules with a one-way dependency flow:

`foundation -> application modules -> Tauri command adapters -> React features`

Tauri commands are transport adapters. They validate input, call a service, and
return a DTO. They must not contain SQL, process construction, MCP protocol
logic, or log parsing.

The registered modules are authoritative. Internal implementation types remain
private or `pub(crate)`. Cross-module behavior is expressed through small
service interfaces and serializable DTOs.

SQLite owns durable tasks, instance history, settings, log sessions, log
events, and archive transfer records. Live process handles, Windows Job
Objects, WebSocket sessions, CLI operation cancellation state, active transfer
cancellation channels, temporary ZIP packages, and open network connections
remain in memory or disposable application data.

Codex continuation intents are durable per conversation. One lifecycle lock serializes queue
dispatch, cancellation and native ownership checks; late completion cannot override a pause.
APP-only answers and document decisions remain in TaskService; CodexTerminalService orchestrates
their continuation without approving documents. TaskControlState projects native/connection/queue
and human-wait states for both UI surfaces. Application startup establishes normal Windows data
context and per-data-directory instance ownership before opening SQLite. Release identity is embedded
and verified against the installed manifest; validated packages are promoted to a stable Publish entry.

`development-terminal` owns the provider-neutral conversation workflow model,
sanitized timeline persistence, shared terminal UI, and explicit per-task
Codex/Grok selection. A task can keep independent provider conversation
directories containing metadata, output-only terminal replay, `workflow.json`,
and a bounded compact-revision `workflow-events.jsonl`. The UI never exposes arbitrary
commands, executable paths, working directories, or raw provider arguments.
`development-tasks` also deploys the provider-specific bundled
`abya-game-development-task` Skill into both `.codex/skills` and
`.grok/skills` whenever a task workspace is created or read. Only those exact
managed Skill directories may be replaced; user-owned Skills remain intact.
Tasks additionally discover bundled `abya-task-template-*` directories with a
valid `abya-task-template.json` marker and sync their complete resources to
the corresponding provider workspace. Matching unmarked user directories are
never overwritten. Template selection lives in the task production record for
full workflows or in scoped-task conversation context. The maintenance-only
`abya-import-task-template` Skill ships beside the executable for importing
templates into both provider roots and is not a gameplay template.
The same deployment boundary supports `abya-art-template-*` with a valid
`abya-art-template.json` marker. Task-template suggestions join requirements
confirmation; art-template suggestions join execution-plan confirmation.
Explicit choices are reused, no style is automatically selected, and existing
assets take priority. Templates add domain constraints to the shared stages.

The production workflow is owned by development-tasks. SQLite task_production and
task_production_history are authoritative; artifacts/game-development JSON/HTML
are exports, distinct from terminal workflow projections. Revision-checked mutations
manage documents, evidence, issues, versions and rounds. Only a Tauri user action
can decide the three document approvals; CLI cannot approve or fabricate acceptance.
Stage gates check current document hashes, actual saved archive/Player fingerprints,
evidence files, complete rounds/questions and issue closure. They validate declared
materials, not gameplay quality or user experience. Scoped tasks remain opt-in.
First-batch common workflow resources are maintained in the repository .codex
bundle and synchronized into .grok by npm run sync:workflow, preserving Grok
entry metadata. The check command rejects drift; remaining resources retain
their existing ownership. Once production is enabled, ordinary task reads stop
refreshing managed Skills. Explicit upgrades require stopped terminals, check for
local modifications, preserve backups/history and restart document confirmation.
The task UI separates production, installed Skill browsing and native conversation
activity. Approval continues an already-open task terminal without creating a chat.

game-instances owns the bounded FFmpeg window recorder and child lifecycle.
Capture targets a task-owned PID's HWND, temporarily shows the window without
activation, and restores visibility at completion. This is video-only capture;
black/occluded frames require review and cannot be treated as gameplay acceptance.
The pinned LGPL shared runtime, license materials and source manifest ship alongside
the application. It never captures the whole desktop or accepts arbitrary commands.

Bot development is a conditional part of the managed task Skill, shared by the
multiplayer template and both providers. Task-owned artifacts/bot-integration
contains the plan, before/current/candidate/reloaded definition snapshots and
version-bound verification materials. Standalone bundled Node modules validate
update scope, artifacts and explicit evidence assertions without game writes.
Game operations still use desktop-cli. Candidate staging is reversible and
followed by full architecture validation before bindings/save. Workflow turn
completion is separate from Bot runtime acceptance and user task acceptance.
The checker is not a backend-enforced completion gate or an atomic version lock.

`codex-terminal` discovers Codex, owns its PTYs and process trees, connects the
visible TUI to the authenticated loopback Codex app-server, and maps native
thread/plan/item notifications into the shared workflow model.
It also registers task-owned native Codex projects through the experimental
project API and synchronizes conversation naming/archive state by native ID.
Archive/restore and open/delete are serialized; archived conversations never
resume implicitly. Local deletion first archives native history. Native sync
failures retain the local conversation list and are displayed explicitly.

Workflow v1.2 persists intake groups, answer revisions and explicit player mode in the
production record. Only APP actions submit human answers; CLI publishes questions with
an authenticated conversation binding. Drafts survive restart, and answers do not approve
documents. Native-bound continuation retains a delivery receipt to prevent blind retries.
Delivery receipts are reconciled against native turn state. Interrupted/failed latest attempts
can resume with a new message ID and preserved attempt history; running, completed or superseded
attempts do not replay. Unknown outcomes fail closed. Intake can request attachment to its exact
original terminal and retry once after the UI acknowledges connection, without creating a chat.

Codex execution uses one runtime app-server per conversation and a separate metadata
backend. Pause interrupts the turn before closing its executor and revoking access;
TUI detachment alone preserves backend access. Reopen loads the original ID with new
scoped credentials. Native preparation fails closed instead of starting a replacement
conversation. Recovery backs up metadata and only rebinds a stopped conversation to a
verified same-workspace native history; neither native history is rewritten.

The terminal UI owns paste gestures; foundation reads the Windows clipboard
only on demand. Plain text uses bracketed paste and serialized bounded IPC
chunks. Only actual images trigger the provider's native image-paste command.
`grok-terminal` independently discovers Grok, owns its PTYs and process trees,
creates or resumes UUID sessions, and incrementally maps the native ACP
`updates.jsonl` plan/tool/turn stream into the same workflow model. Codex uses
`<workspace>/conversations`; Grok uses
`<workspace>/grok-conversations`. Grok discovers its task workflow from
`<workspace>/.grok/skills/abya-game-development-task`. Native authenticated
provider state remains in the user's normal `.codex` or `.grok` directory and
is never copied into a task workspace.

`desktop-cli` exposes authenticated non-MCP command dispatch. Managed terminal CLIs use a local-only Windows named pipe, restricted client SIDs, and revocable in-memory session capabilities bound to provider/task/conversation. Foundation owns transport and session-capability primitives; provider services issue and revoke capabilities without depending on desktop-cli. Desktop retains the long-term credential; sandbox CLIs never decrypt it. Normal same-user standalone CLI access retains the DPAPI-backed HTTP route; managed pipe failures never fall back to it. Each command validates ownership and records sanitized activities. Runtime operations start bundled Abya CLI with managed PID and launch identity. No MCP adapter exists.

New default workspaces live under USERPROFILE/ABYA Desktop Development ToolWorkspaces. On terminal open, tasks in the former LOCALAPPDATA default root are copied to that compatible root before updating their database path. Original files remain as a recovery copy. Only inactive task terminals may migrate; custom roots and reparse points are not migrated automatically.

`game-connections` is the only module that binds the LAN gateway, broadcasts
discovery datagrams, owns WebSocket sessions, and correlates archive transfer
messages and frames. It emits typed connection and log events through an
application coordinator. It does not persist instances or logs and never
exposes the loopback-only desktop or runtime CLI endpoints.

`game-archives` is the sole owner of `Main.PBArc` discovery and complete-folder
validation. `archive-transfer` consumes immutable archive snapshots and the
public game-connection transport; it owns packaging, persistence, limits, and
workflow state without writing connection or instance tables.

Every top-level module has a Skill under `skills/`. A module change without a
matching Skill update is incomplete.
