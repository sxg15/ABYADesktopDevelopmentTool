桌面任务连接文件只存 DPAPI 保护的位置引用，绑定 Windows 用户、任务 ID 与规范化工作区。访问指定 APP 的描述符时同时校验回环路由及凭据密文的完整性。旧环境继续使用原有默认连接发现。

桌面工作台通过 read_document_source 读取任务已登记 Markdown 与提交快照之间的差异。CLI 上下文文件只记录任务、会话身份及 CLI 路径，认证沿用 Windows 用户连接。

## 2026-11-06 Workflow 2.12

task_player_binding 单独读取任务和 App 的 PB 路径与指纹。试玩及预检卡片据此提供确认与重新验证入口。

CapabilityPreflight 增加可选 stage/purpose/reuseExisting，识别 existing-verified。实现和试玩修改工作台显示预检恢复入口；仅方案阶段自动启动，查看 review 不启动进程。review 按钮走已有存档副本，不要求重新提交未变化的方案。

ArtCase optionally includes presetId and resources (kind, license, compatibility, parameters, lifecycle, integration, verification, requiredCapabilities, tests and hashed files). Builtin recipes are available references with designApproved/runtimeVerified=false. Only trusted bundled preview source runs in an opaque-origin sandbox iframe; imported scripts are never executed by the UI.

## 2026-11-05 当前包与任务存档契约

APP 当前 gameExecutablePath 是唯一启动来源；selected_game_executable 只读设置并拒绝不同请求路径。PLAYER_SELECTION 将设置保存与实际进程启动互斥。

# Foundation Skill

Art requirements may expose clarificationAnswer and the server projects currently valid resolvedClarifications. Intake questions optionally carry clarifiesRequirementId.

Verified build info also returns the installed manifest release notes.

The shared projectMigrationReport API exposes read-only per-project results from native startup.

Verified build info exposes the portable release number. Pin the public dev manifest to its Git commit. In-place updates prefetch that exact commit and validate its complete portable package before normal shutdown. The detached updater rechecks the checkout, fast-forwards the existing branch, verifies all hashes, then restarts the same EXE and data context. Reject dirty, diverged or unsupported checkouts and active work.

2026-11-05：人工验收/预览的 Tauri 命令与共享 productionApi 增加可选 seats；旧调用省略时保留默认规则。AcceptanceSession 增加可选 seats 读回实际窗口数。具体人数限制及联网模式由任务/Runtime 模块校验。

2026-11-05：共享 productionApi.startPreview 和 Tauri start_task_preview 接收可选 expectedVersion，传递给 RuntimeBridge 拒绝过期界面选择。省略参数的既有调用继续按任务当前阶段选择预览版本；候选选择与验证由任务模块拥有。

2026-11-02：共享 ProductionIssue 增加可选 playerSummary，表达面向玩家的具体影响；原始 description、诊断和证据继续保留。

## Current scope

Workflow 2.9 enforces reviewHandoffVersion=1: unresolved ordinary issues, missing/failed checks and unfinished work stay in Lua with bounded automatic handoff-repair. Only confirmed abya-source diagnosis bound to current Player/evidence may cross; blocked checks link blockedByIssueIds. Core package does not waive this gate. Playtest shows cross-stage source-defect cards and selected diagnostic ZIP; upgrades preserve original IDs, approvals, budgets and packages and return premature legacy handoffs to Lua without starting execution.

Workflow 2.8 adds a scoped completion loop: current-goal dependency closure, evidence-based wake signals, audited work revision, automatic repair plans, per-output finalization and honest repairRequired/externalBlocked outcomes. No candidate completion without a hash-bound playable ZIP and current lua/input/core-loop/save-reload evidence. Fixture validation remains separate. Upgrade preserves budgets, IDs and approvals; explicit resume retains the original goal and cannot replay uncertain writes. Package compression runs outside the database mutex.

Shared production types add optional stageContractsVersion, issue classification/disposition and prototypeGrants. Legacy records deserialize with empty grants and no enforcement upgrade until explicitly requested. start_production_prototype is an APP-only orchestration command; approval and artifact contracts stay version-bound.

## Purpose

Provide shared errors, DTO conventions, SQLite access, paths, secret storage,
and command API helpers.

## Ownership

Owns durable storage initialization and shared contracts. It does not own task,
process, log, or MCP business workflows.

## Public Contracts

Workflow 2.11 shared production DTOs expose optional pinned-policy flags, preview provenance and technical-queue entries. Missing fields preserve old records. The preview API is distinct from formal acceptance; UI projections never manufacture a successful verification or human approval.

The standalone CLI HTTP transport explicitly disables proxies: its validated 127.0.0.1
endpoint and bearer token must stay on the local connection even when the application
enables reqwest system-proxy support for remote GitHub requests. Named pipes are unchanged.

Shared adapters expose typed intake groups/answer versions/player mode, APP-only answer actions,
native-bound Codex continuation, and scoped conversation recovery. They never transport session
credentials. Intake answers and approvals remain separate domain actions; business logic stays
inside task and provider services.
IntakeContinuation includes queued, paused, waitingForAnswers, waitingForApproval, needsReview,
needsConnection, started, resumed, running, completed and superseded, with request/native/turn IDs.
TaskControlState separates connection, native execution, queue, human gates and last event time.
BuildInfo validates the embedded release/version against the adjacent manifest and executable hash.
Before database initialization, normal launches from the OpenAI.Codex Windows package are brokered
through Explorer to avoid virtualized app data. ABYA_TEST_MODE explicitly permits isolated fixtures.
A per-user canonical-data-directory Global mutex prevents concurrent APP owners and focuses the
existing PID only after checking process creation time. Separate test directories remain independent.
productionApi.revealReport sends a task ID and a fixed report phase to the native adapter;
path validation and export remain in TaskService, and the adapter reveals the file in its folder.
productionApi.revealDocument calls open_production_artifact with optional reveal=true, selecting a
validated task artifact in the OS file manager. Omitted/false retains normal artifact opening.

`AppError`, database access, application paths, settings values, and shared
serialization helpers. Settings include the game gateway port, LAN broadcast
switch, preferred adapter ID, and persistent discovery tool ID.

Shared TypeScript command adapters carry `TerminalProvider` (`codex` only) plus typed provider availability, state, event, input, resize,
conversation, and workspace contracts. They expose provider-specific commands
through one typed terminal API while preserving the existing Codex wire
contracts. Workflow turns, plan steps, activities, observability, rename, and
workflow reads are shared. Settings persist a user-selected task workspace
root and expose the default root through `AppPaths`. Foundation does not
discover a provider, create PTYs, map native events, or own terminal/workflow
lifecycle.

Shared instance contracts include the `background` and `visible` window
visibility modes used by Tauri commands and React features. Business rules and
Windows HWND control remain owned by `game-instances`.

Durable data lives below
`%LOCALAPPDATA%\ABYA Desktop Development Tool\Data`. Desktop CLI secrets are encrypted for the current Windows user with DPAPI. Legacy settings are read without reusing the old token, then saved in the new schema. Public settings never serialize the CLI token. cli_environment publishes an encrypted connection descriptor and resolves the bundled CLI/Node paths. The versioned runtime-table
migration preserves existing task, instance, log-session, and event IDs while
generalizing instances into managed and external origins with separate process
and connection states. `AppPaths.archive_transfers_dir` stores temporary
desktop ZIP packages; durable `archive_transfers` rows store status and
progress. Active transfer rows are marked interrupted on restart.

SQLite uses a 1,000-page WAL auto-checkpoint, a 64 MiB retained-journal limit,
startup checkpoint truncation, and incremental auto-vacuum for new databases.
Small legacy databases and legacy databases with at least 256 MiB and 25% free
pages are migrated through a one-time vacuum; later startup maintenance
reclaims incremental-vacuum free pages without changing durable row IDs.

## Dependencies

May depend on platform and persistence libraries. Product modules may depend on
foundation; foundation must not depend on product modules.

## Validation

Additive task_production and task_production_history tables retain authoritative
versioned production JSON and prior revisions, with task-ID foreign keys. Existing
task IDs and archives are not migrated or rewritten. Domain transitions stay in
TaskService; shared TypeScript production adapters expose typed read/mutate/user
decision, Skill browsing and artifact-opening commands. JSON exports do not import
themselves back into the database.

Run Rust unit tests, migration and storage-maintenance tests, TypeScript contract checks, and
`npm run check:skills`, including workflow DTO serialization compatibility.

## LLM Maintenance Rule

The public `repair_storage` operation performs an explicit checkpoint, VACUUM,
and `quick_check`, returning before/after byte and WAL metrics for the UI.

When changing foundation behavior, schemas, contracts, paths, or dependencies,
update this Skill in the same change.


## On-demand history

Read the matching section in [historical contracts](../../docs/skill-history/20261026/foundation.md) only when changing that surface. New current-scope rules override historical versions.

Owned CLI lease heartbeat (2026-10-26), Workflow 2.5 shared diagnosis (2026-10-24), Terminal freeze recovery (2026-10-22), System memory warning (2026-10-22), Workbench adapters (2026-10-21), Art approval and recheck projections (2026-10-20), 内存保护适配接口（2026-10-19）, Codex-only provider contract (2026-10-19), Workspace library contracts (2026-10-17), Art design contracts (2026-10-16), Game client selection (2026-10-14), Managed sandbox transport, User-initiated clipboard access, Stage workspace update, Workflow 2.0 shared contracts, Review availability contracts (2026-10-12), Background task notifications (2026-10-14), Execution ownership contracts (2026-10-22), Autonomous implementation presentation (2026-10-23).
## 2026-11-05 实施准入预检与自动清理

打开方案、提交与批准方案都不启动 Player。capabilityPreflightVersion=1 的实施/试玩任务在首次需要 Player 的操作前核对完整预检和当前包。首次测试由策划在 APP 明确启动，CLI 仅可复用已有回执或在相同授权范围重试。
独立 .abya-preflight/<runId>（兼容清理旧 taskId/runId 目录） 具有 owner 标记；不放宽普通 authoring 权限。首次失败立即登记 preflight:basic；超时/上下文不可用为普通待定位问题，真实能力目录缺项/必需契约缺失才标 PB 包兼容问题。此问题只能由实际预检回执更新。
reuseExisting 仅复制本任务唯一编辑存档，实测保存冷加载后记为已验证替代，保留原问题。方案可先记录已知风险；运行前须验证包、范围、报告及清理结果。旧任务未显式启动或升级时不追溯撤销既有批准。
回执保存在 artifacts/capability-preflight，临时数据自动清理：停止本次实例、核对 OS PID 已退出、验证 UUID/路径边界/owner/无链接后删除。失败/未知也先停止并核对；重启及每 30 秒继续清理 pending，不重放创建、不停止外来 PID、不删正式存档。只保留诊断回执和日志。无 owner 的空目录可回收，非空且归属不明则保留并报告。

开发任务菜单通过 open_task_folder 按任务 ID 读取登记的绝对工作区路径，验证目录存在后在系统文件管理器打开。

play_version_dates 按任务 ID 返回已登记版本的首次制作修订时间，供试玩卡片显示。

delete_task 接受可选 cleanWorkspace，缺省仅删除任务记录。清理范围由任务服务根据任务 ID 及登记的目录确定。
