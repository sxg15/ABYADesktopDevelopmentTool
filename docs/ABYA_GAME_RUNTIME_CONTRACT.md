# ABYA Game Runtime Contract

This document records the desktop-side protocol implemented through September 1,
2026. The Unity project at `F:\Unity\Projects\AbyaPB` was inspected to verify
the production launch parser, Runtime MCP authentication/session behavior,
startup reports, and desktop WebSocket contract.

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
--abya-mcp-port=<per-instance-random-port>
--abya-mcp-token=<per-instance-random-url-safe-token>
--abya-mcp-autostart=true
--abya-mcp-auto-approve=true
```

`lan-host` receives a generated Host port. `lan-client` selects a running Host
from the same task and inherits its archive and Host port. Per-instance
identity overrides keep same-device participants distinct without combining
production launch with the legacy `--abya-test-role`.

`--abya-mcp-auto-approve=true` overrides the game Settings toggle for Internal
MCP HighImpact confirmation and is not written into `Settings.PBConf`. Ordinary
desktop launch always emits `true` so managed instances do not block on the
in-game approval dialog. The game still accepts `false` as a process-local
override; the desktop tool does not emit that value.

The Runtime MCP token is passed only to the child process and retained in the
desktop process's live instance registry. SQLite, launch reports, logs, tool
definitions, instance details, and MCP results never contain it. Persisted
arguments replace the value with `[REDACTED]`.

## Managed Window Lifecycle

New desktop-managed instances default to `visibilityMode=background`.
Background instances use ordinary windowed Unity rendering, while the desktop
tracks visible top-level windows owned by the managed PID, saves their bounds
and extended styles, removes them from normal task switching, and moves them
outside the display area without calling `SW_HIDE`. Avoiding `SW_HIDE` is
required because Unity D3D11 can stop presenting while a native Player window
is hidden. The controller restores the latest non-game foreground window if
Unity attempts to activate itself. This is not Unity `-batchmode -nographics`:
Runtime MCP screenshots, Runtime UI, Custom UI, animation, particles, and
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
loopback-only desktop MCP endpoint.

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
tasks. External sources cannot receive process or Runtime MCP control, but may
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

## Managed Runtime MCP

Every live managed process has an independent Runtime MCP endpoint:

```text
http://127.0.0.1:<generated-port>/mcp
Authorization: Bearer <generated-token>
```

The desktop initializes MCP protocol `2025-11-25`, caches the non-secret
`Mcp-Session-Id` and server information, and sends that session header on
`tools/list` and `tools/call`. A 404 session response causes one reinitialize
and retry. Readiness probes use bounded two-second connection attempts;
ordinary tool listing is bounded to 30 seconds and long tool calls to ten
minutes.

Runtime MCP results preserve the complete MCP `content` array. In particular,
`ui_capture_screenshot` metadata text and PNG image blocks are returned through
desktop MCP without JSON-text conversion. Stopped and external instances have
no desktop-managed Runtime MCP endpoint.

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
