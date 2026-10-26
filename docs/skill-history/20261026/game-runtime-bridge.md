# Game Runtime CLI Bridge

## Long response transport (2026-10-26)
Bundled Node CLI uses node:http for exact loopback POSTs, with caller AbortSignal deadlines,
bounded response bytes and no redirect/proxy/automatic replay. It avoids an independent
fetch/Undici header/body timeout overriding the 660s game / 670s Node / 690s bridge budgets.
Post-dispatch transport loss is outcome_unknown; retain the original receipt and reconcile.
Failure during discovery/status/input validation is explicitly not_executed; the bridge preserves
that state in its receipt. This does not classify returned game errors as rolled-back writes.
This does not make a failed game initializer ready. Diagnose transport and launch report separately.
Regress delayed headers/body, abort, dropped writes, redirects and a real >300s local response.

## 内存保护与操作追踪（2026-10-19）
同实例高开销 capability run 串行，全局八个 CLI 子进程上限保留；status、工具目录、
read_me_first、editor_get_state、capability_search/describe 仍可用于诊断。内存压力下拒绝
新高开销工具调用，返回 not_executed；执行中压力触发已有
取消通道，结果继续是 outcome_unknown，不能证明包体已停止或自动重试写操作。
发送前流式计算输入 SHA256 与大小，超过 16 MiB 拒绝；日志只保存摘要元数据、
dryRun/save/分页参数、操作数量与请求/会话身份，不复制完整参数、脚本或连接凭据。
started 在发起调用前持久化，返回/错误在结束时关联同一请求；真正工具失败与传输
返回需区分。取消和串行不能替代包体内部检查点或性能修复。
测试模式的 APP 数据目录独立，但 Unity 私有 CLI 注册目录由 Windows Known Folder
决定；仅在该模式下为运行时 Node 显式提供 OS 目录，仍校验 PID/启动身份，不复制或输出凭据。

Acceptance IPC adapters run startup/version hashing/readiness waits and status reads
through spawn_blocking, keeping the async runtime available. Status uses get_readonly
and avoids loading persisted production history when in-memory progress is present.
No status query creates a workspace, syncs Skills or starts Player processes.

## Purpose

Invoke the bundled Abya CLI for desktop-owned game instances.

## Ownership

Own fixed Node/script execution, PID mapping, stable conversation sessions, bounded IO, cancellation and screenshot artifacts.

## Public Contracts

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

## Task acceptance orchestration

start_acceptance resolves TaskService's fingerprinted version specification, launches a visible single-player or Host/Client group through InstanceService, and rechecks every seat after joining.
Check exact launch archive/level, CLI capabilities, initialized game context/network readiness, failed Lua traces and optional CustomUI roots. This is startup readiness, not human acceptance or a full gameplay test.
A task-scoped launch lease prevents concurrent duplicate groups. An intact same-version group is rechecked and shown; partial or old groups require closing first. APP restart checks persisted session IDs against live owned processes.
Progress distinguishes version checking, Host start, joining, gameplay checks, ready, failed and stopped. Failure records the reason and cleanup failures, stops only newly created instances, and retains task/user decisions. No inference calls or automatic gameplay input occur.
Required tools and seats are version-bound, with at most eight seats. Readiness operations are bounded read-only CLI calls; runtime secrets remain in memory. Unit tests reject connected-but-failed Lua and mismatched launch reports; real Player validation is separate.

## Durable runtime receipts and snapshot launch (2026-10-23)
Capability run persists an exclusive pending receipt before subprocess dispatch, hashing
instance/session/arguments/input. Same operation ID + same input returns the known completion
receipt (explicitly not the original payload); conflicting input is rejected, pending/unknown outcomes never redispatch. Receipt
metadata query excludes raw results. Result persistence failure remains outcome_unknown.
Receipts retain result hashes and success booleans only, never raw output/commands/credentials.
Repeated callers must inspect original evidence or read current state for payload details. Existing cancellation and memory guards apply.
For frozen versions, acceptance copies the validated snapshot to a fresh owned run directory;
never launch the immutable registered snapshot for editing or mutate the source archive.
