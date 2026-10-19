# Game Instances Skill

## 内存监测与异常证据（2026-10-19）
每个托管进程的监控线程每秒读取私有提交、工作集、系统可用物理内存与提交上限。
进程采样持有原进程句柄，不能按同名进程或复用 PID 停止其他程序。
MemoryIncidents 位于 Reports，保留独立事件目录、触发前采样/操作、后续有界采样、
Codex 元数据、退出及保护停止结果；每次压力恢复十个样本后再次超限创建新事件。
滚动数据有大小上限，触发前快照固定保留；事件不随任务删除而级联删除。
APP 重启将未正常结束的监测记为中断，不能直接认定 OOM。写入失败须出现在状态中。
默认仅记录并阻止新工具操作，自动停止必须通过 APP 明确对当前托管实例启用，
不持久化该危险开关；停止复用已有优雅退出/Job Object 流程并记录观察结果。
默认阈值为 4096 MiB 私有提交、768 MiB/分钟增长、8192 MiB 系统空闲下限、90% 提交量。
这些是可调整的起始保护值，不是硬内存配额或泄漏判定。外部实例不接受停止控制。
增长率在至少120个启动样本后才启用，避免把正常加载增长误判；绝对占用/系统余量
保护不等待预热。记录无法落盘时阻止新的高开销工具操作，诊断读取仍保留。
启动前串行检查可用内存及提交余量，为尚在加载的实例预留最多 3072 MiB（不超过
私有提交阈值），避免多个启动同时看到同一份余量。仅观察到真实退出才结束监测，
停止请求/失败不伪装成退出；用户正常停止不误报为 OOM。
InstanceRuntimeInfo.memory 是只读投影；策略写入和自动停止授权仅暴露给 Tauri 用户操作。
测试覆盖重复事件、恢复、记录失败、真实采样、外部实例拒绝与逐次明确授权。

## Purpose

Build visual launch profiles and own managed ABYA game process lifecycles.

## Ownership

Owns executable validation, argument generation, multiplayer port and identity
allocation, Windows Job Objects, metrics, and stop behavior. Archive parsing
and discovery belong to `game-archives`.

## Public Contracts

Client retention (2026-10-16): running_executable_paths projects managed launching,
running and stopping paths plus a read-only OS process executable snapshot, including
user-started games. No process is stopped. launch holds foundation EXECUTABLE_FILES
shared through validation/spawn/registration, preventing a cleanup snapshot from racing
a managed launch. The APP coordinator takes exclusive access before collecting paths
and deleting unselected managed packages. Existing LAN Host and task pin precedence remain.

`LaunchProfile`, `LaunchMode`, `ArchiveSelection`, `GameInstance`,
`InstanceOrigin`, `ProcessState`, `ConnectionState`, `InstanceStopResult`,
`LaunchReportResult`, `WindowVisibilityMode`, and instance lifecycle commands.

Launch profiles expose only typed choices. They generate user IDs, user names,
reports, bootstrap log paths, per-instance CLI launch identity, and the
selected LAN gateway endpoint. Canonical modes are `editor`, `offline`,
`lan-host`, `lan-client`, and `igp-hosted`; legacy `normal`, `host`, and
`clientOnly` values deserialize as compatibility aliases. Ordinary desktop
launch does not expose IGP secrets and rejects `igp-hosted` without an
independently secured in-memory IGP context.

Managed launches use production `--abya-launch-*` archive, level, report, and
LAN parameters. They pass the desktop WebSocket connection arguments and --abya-cli-autostart=true. The child receives ABYA_CLI_DEVELOPMENT=1 in its process environment. The game generates the runtime endpoint and token; the desktop retains no runtime connection secret. Runtime commands resolve the live managed PID and verify the desktop launch identity.

New launches default to `background`: the Unity Player remains windowed and
rendering, while a Windows PID-scoped controller preserves each visible
top-level window's bounds and extended style, removes it from normal task
switching, and moves it outside the display area without deactivating Unity's
D3D presentation path. It restores the latest non-game foreground window if
Unity attempts to take focus. `visible` restores the saved bounds/style and
uses `SW_SHOWNOACTIVATE`. Background mode is valid only with windowed
rendering. The desired visibility is persisted in the launch profile, while
HWND values, saved window state, and controller threads remain in memory and
stop with the process.

`lan-client` must select a live `lan-host` in the same task and inherits its
archive, level, Host address, and Host port. Process handles are held in memory
and wrapped in a Windows Job Object. Stop first requests process-tree exit,
waits five seconds, force-terminates when necessary, waits another ten seconds,
and returns the observed result.

Managed stdout and stderr share the instance diagnostic log. The process
monitor checks it every 500 ms and truncates it with an explicit marker when it
exceeds 64 MiB, including while the child append handles remain open, so noisy
Unity output cannot exhaust the system drive.

External instances are created when a user-started game connects without a
desktop instance ID. They have no task, process, or CLI control, but may
provide logs and capability-gated archive transfer. Finished managed instances
and disconnected external history may be deleted; active processes and
connected sources are protected. SQLite cascades associated structured log
sessions, events, and archive transfer history.

Read `docs/ABYA_GAME_RUNTIME_CONTRACT.md` before changing launch arguments,
archive parsing, identity allocation, or local multiplayer behavior.

## Dependencies

Depends on foundation, task contracts, and the game-connections endpoint.
Runtime CLI and log protocols remain outside this module.

## Validation

The window recorder uses the bundled pinned FFmpeg LGPL shared build. Start selects
the largest non-minimized window owned by the managed Player PID and captures that
HWND through FFmpeg gfxcapture / Windows.Graphics.Capture at 15 fps to H.264 MP4, without audio. GDI is not a fallback: it may capture occluding windows over GPU-rendered content. A zero HWND is rejected; no monitor, title or executable matching is exposed. It temporarily restores and raises the
game window without activation, then restores the prior visibility on stop, early
failure or maximum duration. Occlusion must not replace the target content; minimization and resize can invalidate visual
evidence. Metadata records task, instance, version, frames, times and final state.
A Windows Job Object owns the encoder; process/APP stop finalizes or marks interruption.
The native smoke test uses an owned brief non-activating fixture, checks actual
encoding/dimensions and is run explicitly. Visual QA must inspect decoded frames;
an earlier off-screen probe generated black frames and is not a passing capture.

Test exact production launch and desktop-connection arguments, CLI autostart and absence of legacy launch arguments,
Host port allocation, archive parsing, LAN client Host selection, launch
report bounds, wait timeouts, background launch defaults, legacy visible
profile compatibility, PID-scoped off-screen/restore behavior, foreground
preservation, hidden task-switching state, continued background rendering,
process-tree termination, bounded instance stdout/stderr logs while append
handles remain open,
managed/external registration, external process-control rejection,
active-instance delete protection, and log cascade deletion.

## LLM Maintenance Rule

The instance service exposes storage repair for truncating oversized files
under the authoritative InstanceLogs directory.

When changing launch options, instance data, process behavior, or validation,
update this Skill in the same change.

Rust test builds append an isolated --abya-data-root for real Player validation. Production builds do not change the game data root.

A wall-clock watchdog bounds a stalled capture to maxSeconds plus five seconds of startup allowance, marks it failed and restores prior visibility. The occlusion regression decodes two video frames and checks target pixels under a separate topmost cover; static fixtures must animate to provide compositor updates.

## Stage workspace update

The stage workspace release retains the existing Windows Graphics Capture recorder fix. Verification must keep file completion separate from frame review and reuse the owned-window occlusion regression.

## Acceptance launch consistency

LAN clients inherit the selected live same-task Host's executable as well as archive/level/port. Validate the inherited executable before spawning so generic UI and CLI cannot mix Player builds within one pair.
RuntimeBridge owns acceptance orchestration and calls this module's existing lifecycle APIs. Failed acceptance launches stop only instances created by that attempt; unrelated and reused instances are preserved.
The generic launch dialog prefers the selected task's captured Player over global settings. The dedicated acceptance button additionally validates fingerprints and readiness. Test inherited executable/archive and task ownership.
