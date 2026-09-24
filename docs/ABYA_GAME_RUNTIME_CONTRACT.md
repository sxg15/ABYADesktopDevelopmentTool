# ABYA Game Runtime Contract

当前版本使用纯 CLI 开发入口。旧协议已退役；日志和存档传输继续使用原有 WebSocket。

## Launch Parser

Unity's existing parser accepts both forms:

- `--key=value`
- `--key value`

The desktop tool emits each argument separately and never exposes a raw
command-line field in the UI.

Existing parser source:
`Assets/Scripts/Abya/Runtime/Foundation/AbyaProcessLaunchOptions.cs`

## Managed Launch Arguments

Every desktop-managed instance receives:

```text
--abya-launch-mode=editor|offline|lan-host|lan-client
--abya-launch-archive-guid=<archive-guid>
--abya-launch-archive-path=<archive-directory>
--abya-launch-level-guid=<optional-level-guid>
--abya-launch-report=<per-instance-json-path>
--abya-launch-exit-on-failure=true|false
--abya-devtool-endpoint=ws://<selected-private-ip>:<port>/game/v1/connect
--abya-devtool-instance-id=<desktop-instance-id>
--abya-devtool-protocol=1
--abya-devtool-autoconnect=true
--abya-cli-autostart=true
```

`lan-host` receives a generated Host port. `lan-client` selects a running Host
from the same task and inherits its archive and Host port. Per-instance
identity overrides keep same-device participants distinct without combining
production launch with the legacy `--abya-test-role`.

桌面在托管游戏的进程环境中设置 ABYA_CLI_DEVELOPMENT=1。游戏确认桌面启动信号后，
仅向 ExternalCli 开放冻结的桌面开发能力清单并执行其写入授权策略。内置 APA 保持原准入。
游戏生成自己的回环端点及临时令牌，桌面不通过参数传递游戏令牌。

## Managed Window Lifecycle

New desktop-managed instances default to `visibilityMode=background`.
Background instances use ordinary windowed Unity rendering, while the desktop
tracks visible top-level windows owned by the managed PID, saves their bounds
and extended styles, removes them from normal task switching, and moves them
outside the display area without calling `SW_HIDE`. Avoiding `SW_HIDE` is
required because Unity D3D11 can stop presenting while a native Player window
is hidden. The controller restores the latest non-game foreground window if
Unity attempts to activate itself. This is not Unity `-batchmode -nographics`:
Runtime CLI screenshots, Runtime UI, Custom UI, animation, particles, and
visual validation remain available.

`visibilityMode=visible` preserves the normal visible Player. A running
windowed managed instance can switch between the two modes. Showing restores
the saved bounds/style with a non-activating Windows operation; HWND values
and saved native window state are never persisted. Borderless and fullscreen
launches cannot use background mode.

`igp-hosted` is represented by the launch model but ordinary desktop launch
rejects it because the desktop tool does not collect or persist IGP ticket,
bootstrap endpoint, secret, or IPC credentials.

## LAN Discovery

The desktop broadcasts one JSON datagram every two seconds on UDP port `47611`
from every private IPv4 adapter:

```json
{
  "magic": "ABYA_DEV_TOOL",
  "protocolVersion": 1,
  "toolId": "<persistent-uuid>",
  "displayName": "<computer/tool name>",
  "gatewayPort": 47610,
  "path": "/game/v1/connect",
  "transport": "ws",
  "capabilities": ["logs", "archive-transfer-v1"]
}
```

The game must use the UDP source IP as the desktop address. No IP address,
authentication token, or secret is embedded in the payload.

The game Settings > Development workflow should listen for these datagrams,
show discovered tools, and connect to the selected endpoint. A game launched by
the desktop connects directly from its launch arguments.

## WebSocket Gateway

Default listener:

```text
ws://0.0.0.0:47610/game/v1/connect
```

The desktop Settings view can change the port and select the preferred private
IPv4 adapter used in managed launch arguments. Discovery still broadcasts on
all private IPv4 adapters when enabled.

This LAN protocol intentionally has no authentication, encryption, pairing,
token, or approval. It must remain separate from the authenticated,
loopback-only desktop CLI endpoint.

## Connection Lifecycle

The first game message must be `hello`:

```json
{
  "type": "hello",
  "protocolVersion": 1,
  "runtimeInstanceId": "<game-persistent-runtime-id>",
  "instanceId": "<optional-desktop-managed-id>",
  "displayName": "Game instance",
  "gameVersion": "1.0.0",
  "platform": "Windows",
  "capabilities": ["logs", "archive-transfer-v1"],
  "logSessionId": "<runtime-log-session-id>"
}
```

Supplying `instanceId` associates the connection with an existing managed
instance. Omitting it creates or resumes an external source outside development
tasks. External sources cannot receive process or Runtime CLI control, but may
provide logs and capability-gated archive transfer.

The desktop replies with `welcome`, including its assigned connection and
instance IDs, heartbeat and timeout values, the desktop log-session ID, whether
logs are enabled, and the latest persisted sequence to resume after.

Protocol timing and limits:

- Heartbeat interval: 5 seconds.
- Disconnect timeout: 15 seconds.
- Reconnect grace contract: 30 seconds.
- Initial hello timeout: 8 seconds.
- Maximum WebSocket message: 512 KiB.
- Maximum log batch: 100 events and 256 KiB.

Messages are:

- `hello`, `welcome`
- `heartbeat`
- `log_batch`, `log_ack`
- `logs_control`
- `archive_transfer_offer`, `archive_transfer_decision`
- `archive_transfer_ack`, `archive_transfer_progress`
- `archive_transfer_complete`, `archive_transfer_result`
- `archive_transfer_cancel`
- `error`, `disconnect`

## Structured Logs

The game sends:

```json
{
  "type": "log_batch",
  "batchId": "<unique-batch-id>",
  "sessionId": "<runtime-log-session-id>",
  "events": [{ "sequence": 1 }],
  "droppedEvents": 0
}
```

The desktop inserts events with `(desktopLogSessionId, sequence)`
deduplication, updates dropped-event counts, and only then sends:

```json
{
  "type": "log_ack",
  "batchId": "<unique-batch-id>",
  "latestSequence": 1,
  "acceptedEvents": 1
}
```

The game must retain unacknowledged events for replay. On reconnect it resumes
after the `welcome.resumeAfterSequence` value. `logs_control` with
`enabled=false` pauses sending while preserving desktop history.

External connections automatically create or resume log collection and remain
queryable after disconnect.

## Managed Runtime CLI

运行时桥接通过固定的 Node 和随包 abya CLI 调用游戏。按托管 PID 发现实例，
并核对 status.desktopInstanceId 与桌面 launch ID。命令共享会话身份，每次操作使用不同 UUID。
输入经 stdin 传递，图片落盘到任务目录，结果保留全部内容。超时或取消返回结果不确定，
不自动重试写入。目录可见性仍受调用方、Player 清单与场景上下文约束。

## Archive Transfer

The game must advertise `archive-transfer-v1` in `hello.capabilities`. Both
managed and external instances may advertise it. The desktop first sends:

```json
{
  "type": "archive_transfer_offer",
  "transferId": "<uuid>",
  "sourceDisplayName": "<desktop computer>",
  "archiveGuid": "<archive-guid>",
  "archiveName": "<archive-name>",
  "author": "<author>",
  "fileCount": 42,
  "uncompressedBytes": 123456,
  "packageBytes": 100000,
  "sha256": "<zip-sha256>",
  "format": "zip",
  "conflictPolicy": "replace",
  "chunkSize": 262144
}
```

The game must prompt its local user and respond with
`archive_transfer_decision`, including `accepted` and an optional `reason`.
The desktop waits up to ten minutes for this decision.

Accepted package data uses binary WebSocket frames:

```text
magic "ABAT" (4 bytes)
version 1 (1 byte)
frame type 1/data (1 byte)
transfer UUID (16 bytes)
offset (u64 little endian)
payload length (u32 little endian)
payload
```

Chunks are at most 256 KiB. The desktop keeps at most four chunks
unacknowledged and the game returns cumulative `receivedBytes` through
`archive_transfer_ack`. Data acknowledgements time out after 30 seconds.

After all bytes are acknowledged, the desktop sends
`archive_transfer_complete`. The game verifies SHA-256, safely extracts and
validates the archive, atomically installs it, refreshes archive metadata, and
returns progress plus a final `archive_transfer_result`. Finalization may take
up to ten minutes.

`archive_transfer_cancel` requires the game to stop receiving and remove
staging data. v1 does not resume after disconnect or application restart.
Desktop limits are two active transfers globally and one per target.
