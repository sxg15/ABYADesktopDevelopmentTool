---
name: archive-transfer
description: Package and send complete ABYA archives from the desktop tool to connected game instances.
---

# Archive Transfer Skill

## Purpose

Coordinate user-approved transfer of a complete local ABYA archive folder to a
connected game instance.

## Ownership

Owns transfer persistence, ZIP package creation, SHA-256 calculation, active
transfer limits, cancellation, byte-window flow control, status transitions,
temporary package cleanup, and the transfer feature view.

## Public Contracts

`ArchiveTransferService`, `ArchiveTransferTarget`,
`StartArchiveTransferInput`, `ArchiveTransferRecord`, and the Tauri archive
transfer commands.

Targets include managed and external connected instances only when their
`hello.capabilities` contains `archive-transfer-v1`. A transfer sends the full
folder containing `Main.PBArc` as Deflate ZIP, then waits for explicit game
acceptance before sending 256 KiB binary chunks. At most four chunks may be
unacknowledged. Globally at most two transfers may run, and a target may own
only one active transfer.

Statuses are `preparing`, `waitingAcceptance`, `transferring`, `finalizing`,
`completed`, `rejected`, `cancelled`, `failed`, and `interrupted`. Records are
durable; package files are temporary. v1 has no interrupted-transfer resume.

The UI enforces target-first selection, supports fixed-directory discovery and
manual `Main.PBArc` selection, polls progress, and exposes cancellation for
active transfers.

## Dependencies

Depends on foundation persistence and paths, game-archives validation,
game-connections transport, and game-instances metadata. It must not parse raw
WebSocket frames or duplicate archive validation.

## Validation

Test package contents and hash, state round trips, source/target validation,
global and per-target limits, cancellation, rejection, disconnect, invalid
acknowledgements, timeouts, completion persistence, cleanup, command DTOs, MCP
dispatch, and frontend target-first behavior.

## LLM Maintenance Rule

When changing transfer state, packaging, limits, protocol interaction, UI flow,
or persistence, update this Skill, `skills/game-connections/SKILL.md`,
`docs/ABYA_GAME_RUNTIME_CONTRACT.md`, and desktop MCP documentation when
applicable.
