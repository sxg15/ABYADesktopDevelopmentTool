# Game Connections Skill

## Purpose

Let ABYA game instances discover the desktop tool and initiate one shared LAN
WebSocket connection for structured logs and archive transfer.

## Ownership

Owns private IPv4 interface discovery, preferred-adapter endpoint selection,
UDP advertisements, the LAN WebSocket listener, protocol validation,
heartbeats, duplicate-session replacement, message limits, archive transfer
message/frame correlation, and typed connection events.

## Public Contracts

The gateway binds `0.0.0.0:47610` by default at `/game/v1/connect`. Discovery
broadcasts protocol version 1 on UDP port `47611` every two seconds from every
private IPv4 adapter. The UDP source address is authoritative; advertisements
contain no address, token, or secret.

Messages are `hello`, `welcome`, `heartbeat`, `log_batch`, `log_ack`,
`logs_control`, `error`, and `disconnect`.
Archive-capable sessions also use `archive_transfer_offer`,
`archive_transfer_decision`, `archive_transfer_ack`,
`archive_transfer_progress`, `archive_transfer_complete`,
`archive_transfer_result`, and `archive_transfer_cancel`, plus versioned
`ABAT` binary data frames.
Heartbeats are expected every five seconds and time out after fifteen seconds.
The initial hello must arrive within eight seconds. WebSocket messages are
limited to 512 KiB; log batches are limited to 100 events and 256 KiB.

Archive transfer v1 uses 256 KiB chunks and cumulative byte
acknowledgements. The binary header is `ABAT`, version byte, frame-type byte,
16 UUID bytes, little-endian `u64` offset, little-endian `u32` payload length,
then payload. A session must advertise `archive-transfer-v1`; managed and
external sessions may both use this capability.

The LAN gateway intentionally has no authentication, encryption, pairing,
token, or approval. Desktop MCP remains a separate authenticated loopback-only
server and must never be routed through this gateway.

## Dependencies

Depends only on foundation DTOs and settings. Instance and log services are
connected through `GameConnectionHandler`; this module must not write their
tables or own process lifecycle rules.

## Validation

Test private-adapter filtering and selection, discovery payloads, hello/version
validation, duplicate replacement, heartbeat timeout, reconnect cleanup,
log limits, and a fake WebSocket game client covering welcome and log
acknowledgement.

## LLM Maintenance Rule

When changing discovery, ports, protocol messages, limits, session behavior,
or security boundaries, update this Skill and
`docs/ABYA_GAME_RUNTIME_CONTRACT.md` in the same change.
