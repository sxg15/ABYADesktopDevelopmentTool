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
## 2026-11-05 方案基础能力预检与自动清理

capabilityPreflightVersion=1 的任务在方案阶段运行固定创建/加载/保存/冷加载检查，仅使用 APP 当前包。production preflight / development_production_preflight（reuseExisting、retry 默认 false）从认证上下文取得 taskId，无任意 Player 或测试目录参数。APP 进入方案自动执行，CLI 提交方案遗漏预检时先执行并要求刷新记录。
独立 .abya-preflight/<taskId>/<runId> 具有 owner 标记；不放宽普通 authoring 权限。首次失败立即登记 preflight:basic；超时/上下文不可用为普通待定位问题，真实能力目录缺项/必需契约缺失才标 PB 包兼容问题。此问题只能由实际预检回执更新。
reuseExisting 仅复制本任务唯一编辑存档，实测保存冷加载后记为已验证替代，保留原问题。方案引用当前报告并说明替代路径及限制；未测、失败、证据变化、包变化或清理未完均不能进入实现。旧任务未显式启动或升级时不追溯撤销既有批准。
回执保存在 artifacts/capability-preflight，临时数据自动清理：停止本次实例、核对 OS PID 已退出、验证 UUID/路径边界/owner/无链接后删除。失败/未知也先停止并核对；重启及每 30 秒继续清理 pending，不重放创建、不停止外来 PID、不删正式存档。只保留诊断回执和日志。无 owner 的空目录可回收，非空且归属不明则保留并报告。
