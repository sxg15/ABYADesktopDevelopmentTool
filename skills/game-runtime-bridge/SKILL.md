## 2026-11-13 新版 PB 初始化等待

App 试玩和正式验收也在读取必需能力目录之前执行同一游戏根就绪等待；CLI 连通时的初始化目录不能用来判定缺少 runtime_get_game_state。就绪后仍验证必需能力、存档/关卡、网络、UI 和 Lua，失败只清理本次创建的窗口。维护验证必须覆盖 start_preview 的单窗口真实入口，独立 instance launch 成功不等于试玩入口成功。

基础预检的初次连接与冷加载均区分 CLI 连通和游戏根就绪。只读 runtime_get_game_state 返回明确的 runtime_not_ready / not_executed / initializing / ready=false 时，在原有根初始化限时内继续查询 CLI 状态及游戏根；这不代表预检通过。真实失败、未知结果、进程退出和通信错误仍进入失败及清理流程，创建/加载/保存写操作不自动重放。回归覆盖 starting_runtime、resource_label、场景切换以及未知结果拒绝。

## 2026-11-13 试玩入口就绪检查

验收/预览先等待 CLI 与游戏根就绪，再读取能力目录。目录请求失败与声明缺少工具分别报告。初始化过程中不提前检查 Lua 与 UI，进入目标关卡后才核对同一实例的脚本和 UI 根。

## 2026-11-13 主菜单原生空白建档

在已初始化的 MainMenu 中，若已知创建/加载接口受 CreatorEditor 场景约束，则使用受管启动的私有 Player：核对该进程在私有数据根留下的启动证据，在 MineMenu 与 CreatArchiveBox 中针对可见、唯一、可操作的控件选择创建、写入名称作者与说明、核对回读后只点击一次创建。只接受独立目录中的新 GUID 和实际关卡，用相同数据根重新启动编辑器，再进行保存与冷读验证。

## 2026-11-13 隔离 Player 就绪与策划确认

preflight_ready 把 runtime_get_game_state 报告 runtime_not_ready / not_executed / initializing / ready=false 视为尚未就绪，沿用原超时；其他失败直接返回。写入命令不因该判断自动重试。

## 2026-11-06 预检完成条件

启动试玩各窗口使用确认后的同一 App PB。试玩会话记录实际路径、实际指纹、候选原指纹和兼容试玩标记。预检回传原生设置错误码供 App 恢复。

维护阶段的预检使用 TaskService 回执中的有效 reuseExisting/purpose，不依赖调用者省略的参数。maintenance 的复用检查通过记 existing-verified，不添加虚构的新建故障；加载、保存前和冷加载后等待实际 isLoadingArchive/isLoadingLevel 结束。仍只操作本次 owner 验证的副本，不经过普通编辑入口造成递归预检。

基础能力预检取消180秒总期限；正常初始化和冷加载依据实际状态继续等待。单次通信故障检测仍独立报告，未知写入不重放。停止本次实例或实际连接故障可以结束等待并进入现有清理流程；未测或失败不能成为通过。此变更不新增 Abya 引擎 CLI 能力。

## 2026-11-05 当前包与任务存档契约

桌面 launch_instance 与 CLI 同走 launch_scoped，保留阶段校验与任务数据根。初始化及首次开发启动记录 APP 包路径/完整指纹；差异必须通过 APP 显式确认，不允许 agent 换包。

# Game Runtime CLI Bridge

2026-11-05：start_acceptance/start_preview 增加可选 seats，调用任务规格统一验证1至8及单人固定1。按任务模式决定 Offline/LanHost/LanClient，单窗口多人仍验证网络就绪。回执保存 seats 并显示窗口数；验收会话复用须版本及窗口数一致，旧窗口仍运行时明确要求关闭后更换人数。

2026-11-05：start_preview 接收可选 expected_version，界面选择过期时在创建实例前拒绝。review 使用当前候选独立副本，implementation 使用原检查点规则，回执显示实际版本。失败不回退历史包；并发版本变化由复制/记录阶段校验拒绝，清理本次实例。人工试玩实例继续禁止 AI 输入。

## 通用任务空白初始化（2026-11-02）
archive create 编排独立数据根的托管启动、当前 Player 原生 editor_create_archive / editor_load_archive 及真实读回。
不复制已有游戏、不生成猜测 PBArc、不改 Player；返回真实 instanceId、archiveGuid、levelGuid、archivePath。
普通任务可用；完整制作流程仅 implementation/review 且需求/计划已批准可用，实际写入前重验授权与运行包固定。
creationId 是任务内稳定 UUID，参数变化拒绝；重复请求只返回持久回执和实例是否存活，不再次启动或创建。
原子回执在 artifacts/archive-creation 下，逐步保留实例与 Runtime 操作 ID。创建派发后的错误保守记 outcome-unknown，保留实例和数据供对账。
创建派发前的错误只停止本次实例，其他实例不受影响。已停止的成功实例用返回 archive 字段通过普通 instance launch 重新打开。
普通任务及旧策略重开本工具创建的存档时，复用经过 owner 校验的同一私有数据根，不退回全局存档目录。
初始化后才给实例绑定读回的真实存档选择，普通版本登记可沿用该选择；启动时未伪造 Player 的存档就绪报告。
Player 须支持托管自动跳过启动页，以及主菜单的存档创建/加载能力。创建前实际 describe 两个入口的 available；旧包未支持时明确拒绝，不模拟按键或放宽普通编辑权限。
同时预检 editor_load_archive 的 openStartLevel 布尔参数，初始化明确传 true，直接加载原生开始关卡；没有该契约的 Player 在创建前拒绝。

2026-11-02：旧流程的已验证包也使用独立 preview 启动，不再混用正式验收入口。
当前包无 sourceRevision 时必须完整核验当前版本；历史包仍须校验来源、范围、指纹与证据。

## Owned acceptance data roots (2.8)
Frozen candidates run from TaskService-validated workspace-root/.abya-play-runs/<UUID>/Archives copies. An owner marker binds every run to its task; short paths avoid legacy Player MAX_PATH failures. The acceptance orchestrator alone passes an internal data-root override; ordinary CLI/UI profiles cannot select arbitrary roots. This makes older Players discover the exact run copy without changing the candidate, global archive collection, or Player fingerprint.

## Current scope

Workflow 2.7: authoring.rs guards agent launch and scoped capability dispatch. Approved requirements/plan authorize implementation/review; design permits catalog reads and cleanup, not architecture/Lua checks. APP-only start_prototype verifies task ownership, copies saved state, launches a dedicated editor and commits a short-lived archive grant. Before prototype writes, read the live archive path and compare its canonical path; archive-switch commands are refused. Grant failure stops only the new instance. No claim of a general filesystem sandbox.
Transport success is distinct from data.valid/ready and runtime acceptance. Preserve delayed-response budgets, exact ownership, operation receipts and unknown-write reconciliation; never auto-replay writes.

## Purpose

Invoke the bundled Abya CLI for desktop-owned game instances.

## Ownership

Own fixed Node/script execution, PID mapping, stable conversation sessions, bounded IO, cancellation and screenshot artifacts.

## Public Contracts

Workflow 2.11 checkpoint previews launch isolated verified snapshots, including a retained older candidate, without mutating formal acceptance or the current candidate. Launch failure cleans up only instances created by that request. Player qualification reads reports directly from the owned instance, rejects incomplete/empty/failed reports and validates full build fingerprints; discovered tools are separate from verified checks. Baselines never exempt current-game acceptance.

New-policy agent launches use a task-owned authoring data root and enforce the task's captured Player pin. Existing policies keep their original launch behavior. The bridge does not let a game agent replace its engine simply by supplying another executable path; explicit APP-controlled platform maintenance is separate.

RuntimeBridgeService reads live managed PID from InstanceService and the task workspace from TaskService. Each CLI invocation passes --instance, --json and an operation-owned output directory. ABYA_CLI_DESKTOP_INSTANCE_ID must match the host status. No endpoints or tokens are cached by this module. JSON arguments travel through stdin without shell interpretation. Images are written under artifacts/runtime/<instance>/<operation>. Output is capped at 16 MiB, stderr is drained with a 64 KiB cap. Writes are never automatically retried. Cancellation sends the game cancel request before terminating a lingering child and returns outcome_unknown. External and stopped instances are rejected. Readiness reports CLI connectivity; the task Skill must additionally verify archive/level and required capability availability.

## Dependencies

Foundation, game-instances and development-tasks.

## Validation

CLI process lifecycle, fake CLI host, output/identity rejection, multiline input, Unicode paths, screenshots, session isolation, cancellation and stopped-instance rejection.

## LLM Maintenance Rule

Update this Skill, command documentation and tests whenever these contracts change.

read_artifact canonicalizes both the task runtime artifact root and requested file, rejects path escape and non-PNG/JPEG files, and bounds images to 16 MiB before returning a data URL.

The shared process budget allows eight concurrent runtime CLI calls. Stopping a terminal requests cancellation for its active session. Release builds require bundled CLI and Node files and never fall back to the developer source tree.

Bundled CLI discovery deduplicates identical registration records. Multiple remaining
candidates are checked using authenticated status reads, four at a time with a
1.5-second per-host deadline. PID, project and ABYA_CLI_DESKTOP_INSTANCE_ID must
match. Stale or foreign endpoints are excluded; multiple verified endpoints still
fail with ambiguous_instance. Discovery neither deletes registrations nor invokes
gameplay capabilities. Runtime ID is not the CLI selector. Four independent process
fixtures cover PID routing even when legacy Runtime IDs are identical.


## On-demand history

Read the matching section in [historical contracts](../../docs/skill-history/20261026/game-runtime-bridge.md) only when changing that surface. New current-scope rules override historical versions.

Long response transport (2026-10-26), 内存保护与操作追踪（2026-10-19）, Task acceptance orchestration, Durable runtime receipts and snapshot launch (2026-10-23).
## 2026-11-05 实施准入预检与自动清理

打开方案、提交与批准方案都不启动 Player。capabilityPreflightVersion=1 的实施/试玩任务在首次需要 Player 的操作前核对完整预检和当前包。首次测试由策划在 APP 明确启动，CLI 仅可复用已有回执或在相同授权范围重试。
独立 .abya-preflight/<runId>（兼容清理旧 taskId/runId 目录） 具有 owner 标记；不放宽普通 authoring 权限。首次失败立即登记 preflight:basic；超时/上下文不可用为普通待定位问题，真实能力目录缺项/必需契约缺失才标 PB 包兼容问题。此问题只能由实际预检回执更新。
reuseExisting 仅复制本任务唯一编辑存档，实测保存冷加载后记为已验证替代，保留原问题。方案可先记录已知风险；运行前须验证包、范围、报告及清理结果。旧任务未显式启动或升级时不追溯撤销既有批准。
回执保存在 artifacts/capability-preflight，临时数据自动清理：停止本次实例、核对 OS PID 已退出、验证 UUID/路径边界/owner/无链接后删除。失败/未知也先停止并核对；重启及每 30 秒继续清理 pending，不重放创建、不停止外来 PID、不删正式存档。只保留诊断回执和日志。无 owner 的空目录可回收，非空且归属不明则保留并报告。
