# 桌面 CLI 命令契约

桌面应用继续拥有所有业务状态。托管任务中的 abya-desktop 通过认证的本机命名管道调用应用服务，
由桌面使用现有 abya CLI 操作游戏。没有旧协议适配器或自动降级。

使用 ABYA_DESKTOP_CLI 环境变量指定的程序；先运行 doctor --json 和 capabilities --json。
命令组与 ID 映射见 .codex/skills/abya-game-development-task/references/cli-commands.md。
所有命令接受 --input-file 文件或 -，--json；可用 --request-id 指定 UUID 以关联取消操作。
任务/会话/provider 由环境变量传入并由服务验证，不作为认证凭据。
独立同用户 CLI 保留 DPAPI 认证的 /api/v1/command，可调用任务列表和创建；托管会话不能创建或列出全局任务。
托管终端额外注入 ABYA_DESKTOP_PIPE、ABYA_DESKTOP_PID 和 ABYA_DESKTOP_SESSION_TOKEN。
CLI 校验服务进程身份；服务验证客户端 SID 和凭据绑定的任务/会话/provider。不要打印或手动复制凭据。
会话凭据只在内存中签发，12 小时过期；停止终端或重启服务即撤销，重新打开终端取得新凭据。
管道出错不会降级到 DPAPI HTTP，也不会修改沙盒、信任或审批策略。
Desktop 长期令牌继续由原 Windows 用户的 DPAPI 保护，不交给沙盒解密。

默认工作区改为 USERPROFILE/ABYA Desktop Development ToolWorkspaces。打开旧默认目录中的任务终端时，
先复制迁移到新目录并更新任务记录，原目录保留为恢复副本；任务 ID、会话和历史不变。
自定义工作区不自动迁移；Codex 启动前的沙盒 doctor 自检会明确报告目录错误 267 或连接失败。

stdout 为单个 JSON 对象，schemaVersion=1，包含 requestId、success、data 和完整 content。
返回码：0 成功，2 参数/校验，3 未找到，4 认证，5 能力不可用，6 执行失败，7 结果不确定。
没有自动写入重试。Ctrl+C 尝试发送取消，退出码 7 不能证明操作已回滚。
截图保存在 artifacts/runtime/<instance>/<operation>；模型必须打开路径进行视觉验收。

runtime describe/search 分别包装 capability_describe/capability_search；参数为 instanceId 加 capabilityId/query。
runtime cancel 参数为 instanceId 与 operationId。取消仅限同一任务、会话及实例的活动操作。

## Desktop

### `desktop_get_capabilities`

Input: `{}`.

Returns registered tool names, security boundaries, query limits, and
non-secret LAN game-gateway state. Read-only.

## Development Tasks

### `development_task_list`

Input: optional `status` (`active`, `completed`, or `archived`).

Returns persistent tasks, optionally filtered by status. Read-only.

### `development_task_create`

Input: required `title`; optional `description`.

Creates an active persistent task. Reversible write.

### `development_task_update`

Input: required `taskId` and `title`; optional `description`.

Updates task text. Omitting `description` preserves the current description.
Reversible write.

### `development_task_set_status`

Input: required `taskId` and `status` (`active`, `completed`, or `archived`).

Changes task lifecycle status. Reversible write.

### `development_conversation_bind`

Input: required `taskId` and `conversationId`; optional `provider` (`codex` or
`grok`, default `codex`).

Requires an initialized CLI session and validates that the provider-owned
conversation belongs to the task. The binding is held only for that CLI
session. Subsequent Desktop CLI tool calls are automatically recorded in the
conversation workflow as started and completed/failed activities with
sanitized input arguments. Reversible in-memory orchestration write.

### `development_conversation_activity_report`

Input:

- Required `status`: `started`, `progress`, `completed`, or `failed`.
- Required `kind`: `analysis`, `command`, `fileChange`, `cli`,
  `gameInstance`, `test`, `web`, `agent`, or `other`.
- Required `summary`: 1 through 240 characters.
- Optional `detail`: at most 2000 characters.

Requires `development_conversation_bind` in the same CLI session. Records a
sanitized semantic milestone in the active conversation turn. When no native
turn exists, reporting creates a local reported turn. Reusing the same summary
updates the matching active reported milestone instead of appending a new
lifecycle row. Do not send credentials, tokens, raw command output, patch
bodies, or downstream CLI results. Reversible workflow-metadata write.

## Game Instances

### `game_instance_list`

Input: optional `taskId`.

Returns managed and external persistent instance history. Providing `taskId`
limits results to managed instances owned by that task. Read-only.

### `game_instance_get`

Input: required `instanceId`.

Returns persisted details plus process state, connection state, current
process-alive state, report path, and non-secret gateway metadata. Read-only.

### `game_archive_list`

Input: optional `root`.

Discovers ABYA archives and levels. The application's default archive root is
used when `root` is omitted. Read-only.

## Archive Transfer

### `game_archive_transfer_target_list`

Input: `{}`.

Lists connected managed and external instances that advertise
`archive-transfer-v1`. Read-only network observation.

### `game_archive_transfer_source_list`

Input: `{}`.

Lists valid archive folders directly below the configured fixed local save
directory. Each result includes the exact `Main.PBArc` path, GUID, name,
author, file count, byte size, and last modification time. Read-only.

### `game_archive_transfer_start`

Input: required `instanceId` and `mainArchivePath`.

Validates the complete source folder, creates a ZIP package and SHA-256, asks
the selected game user for approval, and transfers the package in bounded
chunks. Returns immediately with the durable transfer record. An accepted
request replaces the target archive with the same GUID, so this is an
open-world destructive operation.

At most two transfers may be active globally and one per target. Interrupted
v1 transfers do not resume.

### `game_archive_transfer_get`

Input: required `transferId`.

Returns durable status, phase, acknowledged/package bytes, percentage, hash,
installed path, and error details. Read-only.

### `game_archive_transfer_cancel`

Input: required `transferId`.

Cancels an active transfer and requests target staging cleanup. Open-world
destructive operation.

### `game_instance_launch`

Input:

- Required: `taskId`, `name`.
- Optional: `executablePath`, `mode`, `windowMode`, `visibilityMode`, `width`,
  `height`, `archive`, `hostInstanceId`, `exitOnFailure`, and `language`.
- `mode`: `editor`, `offline` (default), `lan-host`, `lan-client`, or
  `igp-hosted`. Deprecated `normal`, `host`, and `clientOnly` aliases remain
  accepted for compatibility.
- `windowMode`: `windowed` (default), `borderless`, or `fullscreen`.
- `visibilityMode`: `background` (default) or `visible`. Background instances
  remain windowed and rendered, but their windows are removed from normal task
  switching, kept outside the display area, and do not retain foreground
  focus.
- Default resolution: `1280x720`.
- `archive`: `archivePath`, `archiveGuid`, `archiveName`, `levelGuid`, and
  `levelName`.

Starts a managed process with production `--abya-launch-*` parameters, a
selected LAN desktop-connection endpoint, an independent authenticated
Runtime CLI endpoint, and `--abya-cli-auto-approve=true` so HighImpact game
tools do not wait for the in-game confirmation dialog. Omitting `executablePath` uses desktop settings. Editor,
Offline, and LAN Host require an archive. LAN Client requires a live LAN Host
from the same task and inherits its archive and Host port. `igp-hosted` is
rejected without a separate secure IGP context. Raw command-line arguments are
never accepted. Process-creation side effect.

### `game_instance_set_window_visibility`

Input: required `instanceId` and `visibilityMode` (`background` or `visible`).

Changes a running managed Player between its render-preserving off-screen
background state and a visible state. Showing restores the saved bounds and
window style with a non-activating Windows operation. Background mode is
available only for windowed instances. External and stopped instances are
rejected. Reversible local process-window operation.

### `game_instance_stop`

Input: required `instanceId`.

Requests log pause when connected, requests process-tree exit, waits five
seconds, force-terminates when needed, waits another ten seconds, and returns
the final instance plus graceful/forced/timeout observations. External sources
are rejected. Destructive because unsaved runtime state is lost.

### `game_instance_wait_for_state`

Input: required `instanceId` and `targetState`; optional `timeoutMs` from 100
through 600000, default 30000.

Waits for the persisted process state without client polling. Read-only.

### `game_instance_wait_for_cli`

Input: required `instanceId`; optional `timeoutMs` from 100 through 600000,
default 30000.

Waits until the managed instance accepts authenticated Runtime CLI initialize
and returns non-secret endpoint and server information. Read-only network
operation.

### `game_instance_get_launch_report`

Input: required `instanceId`.

Reads the instance's production startup-report JSON. Missing reports are
returned as `exists=false`; files larger than 1 MiB are rejected. Read-only.

## Selected Game CLI

### `game_runtime_get_state`

Input: required `instanceId`.

Checks the selected local HTTP Runtime CLI and returns non-secret endpoint,
game, platform, capability, server, and availability metadata. Read-only.

### `game_runtime_list_tools`

Input: required `instanceId`.

Lists the selected managed instance capability definitions through the bundled Abya CLI. Read-only operation.

### `game_runtime_call_tool`

Input: required `instanceId` and `toolName`; optional object `arguments`.

Calls the selected Runtime CLI tool. The target tool may mutate or destroy
game/editor state. Call `game_runtime_list_tools` first and inspect its schema
and annotations. All text is preserved; images are saved to task artifact files and returned as absolute paths.

## Game Logs

### `game_log_collection_start`

Input: required `instanceId`.

Sends `logs_control` to resume a connected source. Log sessions are created
automatically when games connect. Reversible network operation.

### `game_log_collection_stop`

Input: required `instanceId`.

Sends `logs_control` to pause a connected source. Persisted events remain
available. Reversible network operation.

### `game_log_source_list`

Input: `{}`.

Lists managed and external game sources with current connection state, remote
metadata, and latest log-session state. Read-only.

### `game_log_session_list`

Input: optional `instanceId`.

Lists persisted log sessions, optionally for one game instance. Read-only.

### `game_log_query`

Input: required `sessionId`; optional `severity`, `provider`, `eventName`,
`contains`, `beforeSequence`, and `limit`.

Returns newest persisted WebSocket log events first. `limit` defaults to 200
and must be from 1 through 1000. Read-only.

## Production workflow (1.1)

The authenticated task context owns these commands. `taskId` may be omitted in a managed session.
`production get` returns record, pinned policy, warnings and report locations; no workflow is created by reading.
`production update` accepts `expectedRevision`, `operation`, and object `data`. Supported operations:
initialize, configure, submit-document, register-evidence, set-milestone, save-issue, save-round,
complete-stage and save-knowledge. Their fields are documented in the distributed production-records.md.

`production version` requires instanceId, expectedRevision and versionId. It resolves the task-owned
instance's saved launch archive and Player files, computes their hashes, and binds the record to them.
It accepts neither arbitrary filesystem sources nor agent-supplied hashes. Save and reload the matching
archive/level before invoking it. Actual saved-file changes invalidate later round/delivery checks.
`production report` regenerates three HTML reports and JSON exports from the database.
There is no production approve command. Only a user decision in the APP can approve the current
requirements, plan or delivery document; its displayed revision and hash are checked again.

Writes reject stale revisions. Source JSON exports are not an import/approval channel. A write may
persist in SQLite before report export fails: after any write error, read current state before retrying.
Full tasks cannot be marked completed until current delivery acceptance and closeout have passed.

`skill list` returns installed task Skills and fingerprints. `skill read` requires provider and skillId,
and reads that entry only. Enabled production tasks retain their installed resources and policy until
an explicit APP upgrade; upgrade requires stopped task terminals and preserves history and backups.

## Managed-window recording

`recording tools` reports the installed recorder. `recording start` requires instanceId and optionally
maxSeconds (integer 5–7200, default 3600); get/stop require instanceId. Only owned running Player
windows are permitted. The recorder targets that PID's HWND, shows it without activation for capture,
and restores prior visibility on completion. Keep the window unobstructed and do not minimize/resize it.
Output is 15 fps H.264 MP4 with no audio, plus task/instance/version/timing metadata and a frame count.
A file or positive frame count is not visual acceptance. Inspect actual frames and the relevant process.
Completed native recordings may be registered as production evidence only for the same task/version;
interrupted or zero-frame captures are rejected. The original video and metadata remain task artifacts.
