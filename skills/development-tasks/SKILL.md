## 2026-11-12 Codex 桌面制作流程

桌面工作流按任务补装玩法文档、规则台账、蜡笔特效和桌面工作流 Skill。已有目录保留本地内容。revisit 保存历史并重新打开指定阶段及其后续阶段。

## 美术澄清与设计证据

资源问题可通过 clarifiesRequirementId 在发布时关联旧澄清要求。正式答案生成明确要求时校验 answerRevision、原句与来源哈希，保留旧 kind。旧未绑定题目可由策划在要求修订中关联。设计记录 blocker 使用同阶段、同周期、已审阅且文件哈希一致的 preflight 状态或文档证据。送审只结清同阶段同周期的无交付物队列项。

## 发布包启动迁移

迁移后导出报告时，阶段状态缺项显示「未记录」，保留旧数据的原始进度。

带编号的已验证发布包在启动后台任务前读取实际用户库。按项目及发布编号记录完成状态，归档和已完成项目可以适配并保留原状态。兼容策略与受管 Skill 差异使用备份、暂存校验和逐项目提交。恢复未完成的文件交换、补导出报告；阶段变更和本地编辑进入人工核对。未启用制作流的项目沿用原工作区逻辑。

## 2026-11-06 Workflow 2.12

冻结存档在 review 可用已确认并通过当前预检的 PB 独立试玩。先核对任务绑定，再分配副本。原候选的 PB 来源和验证证据保留。非冻结版本及实现检查点换包后须登记新版本。

当前子流程统一投影：先看当前阶段/周期的最近执行记录，再判断是否仍在执行或需要恢复；不得跳过已结束的新执行，复活更早中断的目标。新请求优先于已暂停/中断的旧请求；running/sending/queued/reconciling/needsReview 仍保留真实执行归属。读取旧任务时仅修正返回的 activeGoalId，不改持久化历史、请求完成状态或启动执行；下一次合法提交自然保存正确指向。前端提交新意见后清除旧目标选择，review 阶段保留原子流程继续入口，不因已有旧试玩版而隐藏。

Lua 实现和试玩修改的启动版本、人数、问题与反馈表单合并到同一“试玩与反馈”区域；VerificationPackage 的 embedded 模式去掉嵌套卡片边框，长问题保留数量并可展开。当前阶段连续滚动，历史仍只读；启动版本、修复、手动阶段交接、草稿/附件和提交继续沿用原接口及禁用条件。

试玩修改预检修复：begin_capability_preflight 支持活动任务的 plan/implementation/review；后两者必须已有有效需求和方案批准。review 默认 reuseExisting，返回 purpose=maintenance、实际 stage；验证通过为 existing-verified，仅要求 runtime-connect/save-archive/cold-load 及清理。预检不回退阶段或重开批准，原新建故障可保留为有证据的非阻断问题。缓存核对任务范围、包路径/指纹和路线，失败新建结果不锁住复用路线；运行包不匹配先报告确认需求。

Windows 长路径目录树改名返回访问/共享错误时，安装使用不存在的目标复制已完成暂存内容，最后写 SKILL.md 并逐文件校验。失败只清理本次创建的目标，既有目标不会被兜底复制覆盖；不改 ACL。发布前须用真实便携包验证此路径。

旧任务首次打开/恢复托管终端时，在无活动 CLI 会话且任务 active 的前提下，仅迁移 executionTimePolicy 并保存历史；不升级其他阶段门槛、不覆盖固定 Skill。运行中任务不被迁移打断。旧时间指令由托管 developer instructions 明确废止。全流程升级仍通过原 APP 升级入口同步 Skill 和新测试计划契约。

Workflow 2.12: executionTimePolicy=unlimited removes all development deadlines after upgrade, including preflight, turns, recovery and finalization. Timing remains telemetry. Migration reopens only budget-expired work, preserves approvals/results/unknown writes and stays paused. Plan submission requires a requirement-hash-bound artifacts/runtime-tests/plan.json without verdicts. Art cases may include abya.art-resources/v1 source/asset files with hashes and closed dependencies; collect/import/export/inspect preserve complete versioned packages. Six bundled interactive recipes are available without claiming planner approval or native verification.

## 2026-11-05 当前包与任务存档契约

task_archive_sources 只提供本任务归属的编辑根及冻结候选；无版本的普通任务也发现 owner 校验的 .abya-authoring。task_player_status/check_task_player 记录当前 APP 包，adopt_task_player 仅 APP 可调用，保留切换历史且不修改旧版本证据。task_export_source 只返回校验通过的指定当前冻结版本。

# Development Tasks Skill

## Current scope

多人人工试玩支持本次1至8个窗口（默认2，正式验收默认沿用已配置人数），每窗口对应真人席位。AcceptanceSpec 保留 multiplayer 标记，选择1窗口仍为联网房主；单人任务固定1。人数选择只影响本次启动，不改任务模式、已保存验收配置或通过记录。界面说明应遵循游戏容量，Bot在游戏内添加。

2026-11-05：review 阶段 preview_record 选择当前登记的冻结候选，校验真实存档/Player 后复制到独立试玩根。缺版本或校验失败时拒绝启动，不回退旧 verificationPackage。implementation 仍按原核心检查点规则预览；旧 review 记录自动适用当前候选选择。
review 请求标记 completed 前须校验当前冻结版本，并记录 playtestResult 的 version、archiveHash、playerBuildHash。此绑定是复验交接，不提升为核心自测或人工验收通过。复验卡片显示当前候选和待复验状态，旧 ZIP 只作历史说明。

2026-11-04：同周期、同需求范围和同目标的当前收尾项可核对其自身新登记的版本后收束；所有正式验收门槛照常验证。收尾超额或中断后，调度器能以当前签名记录缺口，避免反复派发已超额的最终核对。
APP 明确恢复意图可给已耗尽的原目标追加一轮有上限的额度，记录 intentId、旧累计耗时、额度起点及恢复历史。同一意图幂等；自动检查点和重连不追加额度。恢复清除旧轮次起点，离线等待不入账；重新排入原未完成项，已完成项与未知写入/真实外部条件保持原状态。收尾额度单独使用 budgetEpochMs，累计 elapsedMs 始终保留。

通用 archive create 为当前任务分配 .abya-authoring/<taskId> 独立数据根和 UUID 回执，不绑定具体玩法或测试项目。
完整流程须已批准需求/计划且处于 implementation/review；未启用制作流程的普通任务沿用任务级授权。
旧策略不因初始化而升级。目录 owner.json 及路径边界必须通过；不同任务即使用相同 creationId 也不共享回执。
production_owned_archive_root 为位于该任务私有 Archives 内的已有存档提供同一数据根，允许普通任务安全冷加载本工具的成果；不创建或改变其他存档。

2026-11-02：Lua 阶段与顶部指引共享试玩包判断，核心已验证版本显示“可试玩”，独立列明待确认/已确认问题。
试玩包、独立启动与诊断修复入口集中；旧流程也可预览已校验包，不强制升级。
APP-only workbench `enter-playtest` 按 revision/current version 校验包与批准，执行停止后允许用户带未完成项进入 review。
implementation 记为 playable，保留自测、问题、工作项、预算与批准；最终交付门槛不变，CLI 不提供此人工决定。
问题可提供 playerSummary（面向玩家的影响说明）；没有可信摘要时保留“待确认”及原始技术记录，不推断已确诊。

Workflow 2.11 uses one work unit for bounded recovery, preserves original IDs and cumulative time, separates observation from diagnosis, and shares a 180-minute Lua-goal allocation plus a 15-minute finalization window. Explicit migration records a new allocation epoch without erasing history; ordinary retries do not reset it. Additional checks bind approved text or real integrity defects; optional failures do not expand gates. Scoped evidence requires unchanged build/dependencies and rejects unknown impact. Playable previews use verified historical snapshots in isolated copies, never acceptance. See [implementation and release criteria](../../docs/EXECUTION_REFACTOR_20261030.md). Pinned earlier policies retain their behavior.

## Purpose

Manage persistent development work items and their lifecycle.

## Ownership

Owns task CRUD, task status transitions, task workspace allocation, and
task-to-instance history relationships.

## Public Contracts

UI 美术交接沿用现有 resources/direction/visual/realizations：资源批量制作前锁定选中布局基准、唯一配置、分区/留白、字面层级和控件可见尺寸及目标显示下限。制作前分层合成稿、计划批准后的引擎代表片段分别保留全屏与局部对照；缩水、漂移或非等比变形违反有效要求/下限时先修正再扩展，功能通过不能替代视觉通过。
具体规则由受管游戏 UI Skill 和原确认单模板承载；宿主完整部署，不新增 schema、审批点或自动像素判定。现有固定任务仍走 APP 的 Skill 升级入口，保留批准、历史、用户修改和备份，不直接覆盖其工作区。

Model-reported permission causes do not themselves stop 2.11 recovery; native approval and authorization controls remain authoritative. A dependency deferral waits only for an actual unfinished dependency, never for a label on a ready work item. Unknown write outcomes remain unreplayed and explicitly reconciled.

The 2.11 impact mapper preserves full file fingerprints and only recognizes PBUI v1 Layout-only changes and parsed archive LastEditTime changes. Binding/code/unknown-schema changes are never ignored. Layout changes require fresh input/visual/save-reload checks. Explicit dependency projections may support logic evidence reuse, but cannot cross build/scope changes, required retest categories or check identities.

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
## 2026-11-05 实施准入预检与自动清理

打开方案、提交与批准方案都不启动 Player。capabilityPreflightVersion=1 的实施/试玩任务在首次需要 Player 的操作前核对完整预检和当前包。首次测试由策划在 APP 明确启动，CLI 仅可复用已有回执或在相同授权范围重试。
独立 .abya-preflight/<runId>（兼容清理旧 taskId/runId 目录） 具有 owner 标记；不放宽普通 authoring 权限。首次失败立即登记 preflight:basic；超时/上下文不可用为普通待定位问题，真实能力目录缺项/必需契约缺失才标 PB 包兼容问题。此问题只能由实际预检回执更新。
reuseExisting 仅复制本任务唯一编辑存档，实测保存冷加载后记为已验证替代，保留原问题。方案可先记录已知风险；运行前须验证包、范围、报告及清理结果。旧任务未显式启动或升级时不追溯撤销既有批准。
回执保存在 artifacts/capability-preflight，临时数据自动清理：停止本次实例、核对 OS PID 已退出、验证 UUID/路径边界/owner/无链接后删除。失败/未知也先停止并核对；重启及每 30 秒继续清理 pending，不重放创建、不停止外来 PID、不删正式存档。只保留诊断回执和日志。无 owner 的空目录可回收，非空且归属不明则保留并报告。

试玩摘要直接展示 playerSummary，缺失或空白时使用原问题 description/复现记录，另列已记录影响；无标题时从具体描述提取首行。失败检查展示 observations，不按 Esc 等关键词编造表现；确认状态单独标示。真正缺失描述时明确指出记录缺项与问题 ID。
