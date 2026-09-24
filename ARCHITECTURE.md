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
never overwritten. Template selection lives in the task Skill conversation;
it adds no database field or UI command input. The maintenance-only
`abya-import-task-template` Skill ships beside the executable for importing
templates into both provider roots and is not a gameplay template.
The same deployment boundary supports `abya-art-template-*` with a valid
`abya-art-template.json` marker. Art styles are a separate conversation choice
after feasibility and before implementation, with comic-arcade-ui as the
default when the user elects to use a style without naming one.

`codex-terminal` discovers Codex, owns its PTYs and process trees, connects the
visible TUI to the authenticated loopback Codex app-server, and maps native
thread/plan/item notifications into the shared workflow model.
It also registers task-owned native Codex projects through the experimental
project API and synchronizes conversation naming/archive state by native ID.
Archive/restore and open/delete are serialized; archived conversations never
resume implicitly. Local deletion first archives native history. Native sync
failures retain the local conversation list and are displayed explicitly.

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
