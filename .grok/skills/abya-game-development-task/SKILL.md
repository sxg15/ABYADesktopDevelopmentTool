---
name: abya-game-development-task
description: Define, assess, implement, and validate an ABYA game-development task through desktop-managed game instances and the selected instance Runtime CLI. Use for ABYA game creation, editing, testing, or feasibility work inside a desktop development task.
when-to-use: Use when the user asks Grok to design, inspect, implement, fix, or validate an ABYA game through the ABYA Desktop Development Tool.
user-invocable: true
---

# ABYA Game Development Task

Use this Skill inside an existing desktop development task. Do not create a
second database task or change task status without the user's explicit
acceptance.

The desktop CLI is the control plane. Use it to discover archives, launch and
stop task-owned game instances, inspect launch reports and logs, and proxy the
selected instance's Runtime CLI. Never launch an unmanaged process when the
desktop CLI can perform the operation.

## Workflow Visibility

For every user request that needs more than one operation, publish a native
Grok execution plan before any command, file edit, web call, CLI command, or
game-instance action. Keep exactly one step in progress, update the plan as
work advances, and complete or fail every step before the final response. Do
not enter formal read-only plan mode unless ambiguity genuinely requires user
approval.

The executable path is injected as ABYA_DESKTOP_CLI. In PowerShell use
`& $env:ABYA_DESKTOP_CLI doctor --json` and then `capabilities --json`.
Every command automatically carries ABYA_DEVELOPMENT_TASK_ID,
ABYA_DEVELOPMENT_CONVERSATION_ID and ABYA_DEVELOPMENT_PROVIDER.
Use `conversation bind` once to verify context and `conversation report`
for semantic milestones. Pass a JSON object with `--input-file <file|->`.
Read [references/cli-commands.md](references/cli-commands.md) for the exact mapping.
Only use CLI commands for ABYA operations. There is no legacy transport fallback.
Read capability schemas before invoking game operations. Inspect all returned
content blocks and open every relevant image path for visual acceptance.
Exit code 7 or outcome_unknown requires reading actual game state before retrying;
never automatically replay a write. Reuse the same summary for milestone updates.
Do not include credentials, raw output, patches or secrets in activity details.

## 0. Choose A Task Template

Before game classification or instance assessment, inspect only the current
provider's sibling skills/abya-task-template-*/abya-task-template.json files.
A valid template has schemaVersion 1, kind "abya-task-template", an id matching
its directory name, displayName, description, sourceSkillName and SKILL.md.
Ignore ordinary Skills, the importer and incomplete/invalid markers. Do not
read all template bodies just to build the menu.

- If templates exist, show their displayName and description plus
  "不使用模板（通用开发流程）", and wait for the user's choice before starting
  game classification, launching instances or implementing gameplay.
- If the user already names a template or declines templates, honor that
  choice immediately. Never choose multiplayer automatically from keywords.
- If none exist, continue the general workflow below without an empty menu.
- Record the choice in the task specification/conversation; reuse it on
  follow-up turns. Do not ask again for routine edits or acceptance checks.
  Reopen selection only when the user requests a new choice. If a selected
  template disappears or cannot be read, explain and ask for another choice;
  do not silently substitute the general workflow.

For a selected template, read its SKILL.md and load references as needed.
Use its task-definition questions, stages, outputs and acceptance details as
the primary workflow. Reuse the shared rules below for provider binding,
managed instances, read_me_first, feasibility/authorization, architecture,
authoring and completion. Do not run two duplicate intake workflows or ask
again for facts and authorization already supplied. The user's explicit
requirements take priority; choosing a template alone does not authorize game
mutation. A direct template invocation counts as selection and must not loop
back through this menu. The generic path retains sections 1–3 below.

## 1. Classify The Game

Before proposing implementation, ask the user to choose both dimensions:

- Native system Player, or no system Player. The no-Player design uses
  FreeObjects, CustomUI, and explicit input like an ordinary custom 2D game.
- Single-player, or networked multiplayer.

Then collect the intended game content, core loop, victory condition, failure
and restart behavior, input, camera, UI, resources, target archive/level,
network authority model, and any reference videos or documents. Read accessible
references with the appropriate tool and identify inaccessible material as an
evidence gap.

Unless the user supplies assets or explicitly requires existing resources,
plan for the agent to create every visual, audio, and UI asset required by the
accepted gameplay design, import it into the editor, and assign it to the
intended scene objects and CustomUI nodes. Treat unavailable generation or
import capability as an explicit feasibility gap; do not silently substitute
unfinished placeholders.

Read [references/game-launch-parameters.md](references/game-launch-parameters.md)
before selecting a launch profile. Prefer structured launch parameters that
open the target archive/level or join the target Host directly.

## 2. Assess With A Managed Instance

Feasibility analysis may create and destroy temporary task-owned instances, but
must remain read-only with respect to game/archive content until the user
confirms implementation.

1. Inspect desktop capabilities, archives, existing task instances, and the
   configured executable.
2. Launch `editor` for authoring inspection or `offline` for a single-player
   runtime probe. For multiplayer, launch `lan-host` and an independent
   `lan-client`; do not treat CreatorTest/Local as multiplayer evidence.
   Managed instances launch in `background` mode by default. Use Runtime CLI
   screenshots and UI state while the render-preserving Player window remains
   off-screen; show the native window without activation only when direct
   human inspection is required, then return it to background mode.
3. Wait for the process and Runtime CLI with the desktop wait tools. Read the
   launch report when startup is slow or fails.
4. Call the target Runtime CLI `read_me_first` before every new instance
   workflow. Dynamically list tools and Lua APIs; do not guess names, schemas,
   event IDs, or capabilities.
5. Inspect archive/level readiness, visible UI, screenshots, logs, and runtime
   state relevant to the proposal. Do not save, bind, assign, or mutate.

Report one feasibility grade:

- `可直接实现`: supported by current authoring/runtime tools with routine work.
- `可实现但复杂`: supported, but needs substantial gameplay, authority, UI, or
  validation work.
- `需要补充工具`: implementation is plausible but a concrete CLI/Lua/tool
  capability is missing.
- `当前阻塞`: required source material, runtime state, platform, or capability
  is unavailable.

Include evidence, major implementation stages, missing capabilities, risk,
expected validation topology, and what cannot yet be proven. Then complete
the art-style choice below and ask whether to execute if implementation is
not already authorized.

## 2A. Choose An Art Style After Feasibility

After presenting the feasibility result and before implementation or asset
creation, offer a separate art-style choice. Task templates and art templates
are independent: declining a task template does not decline an art template.
Discover the current provider's sibling abya-art-template-* folders using
abya-art-template.json (schemaVersion 1, kind "abya-art-template", matching id,
nonempty displayName, description and sourceSkillName, plus SKILL.md). Read
only marker summaries until the user selects a style.

- Present "使用美术模板（默认 comic-arcade-ui）" and "不使用美术模板";
  list other available styles by name and summary when present.
- A yes/use response without a named style selects
  abya-art-template-comic-arcade-ui. Preselect/recommend it, but wait for the
  user's choice; a default is not an answer. An explicit prior art selection
  or decline already answers this question and must not be requested again.
- The multiplayer task template defaults to this same comic-arcade-ui style.
  The user can decline or choose another style. Selecting multiplayer alone
  does not remove the post-feasibility art choice.
- No available art templates: explain briefly and continue without a template.
  If the requested/default comic style is missing while others exist, offer
  those styles or no template; never silently substitute a different style.
- Record the art template id or explicit none in the task specification and
  conversation, and reuse it on follow-ups. Only reopen selection on request.
  Do not apply a newly added default retroactively to an existing game.

After selection, read only the chosen art Skill and required references.
Inspect its actual visual references before visual design. Reuse suitable
bundled art assets; generate missing assets as needed. Validate import support,
font availability and device fit, and update feasibility if the style adds a
capability gap. Report any blocker before implementation. Keep gameplay,
network authority, input and acceptance contracts from the task workflow.
An art template's font suggestions never override a task's required DingTalk
font. Choosing no art template leaves explicit user art requirements and the
existing general asset/CustomUI workflow in effect.

Art selection can be asked together with the implementation confirmation;
keep the two decisions explicit. Selecting a style alone is not authorization
to mutate gameplay. Existing implementation authorization remains valid.

## 3. Execute Only After Confirmation

After confirmation, state a decision-complete task specification,
implementation plan, test flow, and observable acceptance criteria before
mutating content.

Read [references/gameplay-architecture.md](references/gameplay-architecture.md),
call `editor_gameplay_architecture_snapshot`, and create
`artifacts/gameplay-architecture.v1.json` from
[assets/gameplay-architecture.v1.template.json](assets/gameplay-architecture.v1.template.json).
Bind the artifact to the returned archive, level, and `snapshotRevision`, then
call `editor_gameplay_architecture_validate` with the complete manifest. Do not
mutate gameplay while any deterministic `error` remains. Resolve heuristic
warnings or record an exact warning exception with evidence and a concrete
reason; an exception never suppresses an error. Begin mutation only when the
validation response reports `ready=true`.

- New or rewritten gameplay defaults to a complete self-contained ABYA-LUA
  String. Read the Runtime Lua authoring guides, scaffold against the exact
  event target, search and describe public APIs, validate, then bind.
- Maintain existing APC only when the request targets it. Use APC for new work
  only when the user requests visual programming or a required public Lua API
  is confirmed missing; report the missing API and minimum APC scope.
- A no-system-Player game must use `generatePlayerMode=Manual`. In multiplayer,
  represent each participant with a FreeObject owned by that user and verify
  local owner, server authority, and remote proxy behavior separately.
- Unless the user directs otherwise, generate the accepted design's required
  assets, import them through the validated resource workflow, assign them to
  every intended scene object and CustomUI node, and re-read the assignments.
  A generated file that is not imported and applied is not implemented.
- For CustomUI, give every text element clear contrast against the background
  directly behind it, including relevant gameplay and interaction states.
  Build parent/child sizing, anchors, offsets, wrapping, and spacing so child
  content stays within its parent and the visible screen without unintended
  overlap or clipping.
- Preserve dirty/unsaved content. Never discard, save, revert, or replace it
  merely to run a test.
- Put movement, weapons, projectiles, collisions, damage, health, death,
  respawn, and presentation on the responsible Prefab or exact object event.
  Global and Level logic may coordinate phases, timers, spawn schedules,
  victory, loss, and reset; they must not update every entity each frame.
- For shared FreeObject behavior, scaffold and validate against a `prefab`
  target, then use `editor_prefab_logic_binding_*`. Prefab writes are
  high-impact because they synchronize every linked instance. Prefer native
  projectile and collision/trigger events over manual position or distance
  polling.

After each meaningful milestone, use the active game instance to collect
screenshots, logs, runtime state, Lua/APC traces, and explicit assertions.
Visual inspection alone is not sufficient when a state assertion is available.
After implementation, call `editor_gameplay_architecture_lint` with the saved
manifest. Fix every error. Resolve each warning or keep its evidence-backed
exception visible in the final report. A gameplay result is not structurally
accepted when entity behavior exists only inside a global per-frame loop.

## Instance Recovery

Manage only instances whose `taskId` matches `ABYA_DEVELOPMENT_TASK_ID`.

If an instance stops responding, first make a best-effort five-second capture
of its launch report, logs, Runtime CLI state, and screenshot. Then call the
desktop stop tool, which verifies process-tree termination. Restart at most
twice in one implementation stage. After that, report the repeated blocker and
preserve the collected evidence.

## Completion

For single-player acceptance, prove launch, target archive/level readiness,
core loop, victory, failure, restart, and cleanup. For multiplayer acceptance,
use a Host plus standalone ClientOnly with distinct identities and prove join,
ownership, synchronization, disconnect, and replay/reset on both peers.

When CustomUI is present, inspect every relevant UI state with the authoring
tree, resolved layout rectangles, and runtime screenshots at the target
resolutions or aspect ratios. Do not accept it until text/background contrast
is clearly readable and no child unintentionally exceeds its parent, overlaps
other content, is clipped, or leaves the visible screen. Verify that every
required generated asset is imported and visibly applied rather than merely
present as a resource file.

Separate source/static checks, validation calls, runtime assertions, and real
Host plus ClientOnly evidence in the final report. Do not mark the development
task complete until the user accepts the result.
