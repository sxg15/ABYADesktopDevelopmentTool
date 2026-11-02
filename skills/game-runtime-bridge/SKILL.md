# Game Runtime MCP Bridge

## Purpose
Invoke native PB MCP capabilities for desktop-owned game instances.

## Ownership
Own native HTTP sessions, descriptor discovery, exact PID/launch identity, bounded IO,
cancellation, durable operation receipts and task-owned artifacts. No subprocess,
ABYA business CLI, shell wrapper or transport fallback is permitted.

## Public Contracts
RuntimeBridgeService obtains live managed PID from InstanceService and workspace from
TaskService. Discover private AbyaPB/Mcp/instances JSON records; require loopback /mcp,
initialize protocol 2025-11-25, acknowledge initialization, and verify abya_host_identity.
Cache by PID, desktop instance and conversation; revalidate registration generation before
reuse. Reject ambiguous, stale, foreign and stopped instances. Never log registration tokens.

New-policy launches use task-owned authoring data roots and captured Player pins. Agent
writes require the current approved stage and canonical live archive ownership. Design
stages permit catalog/cleanup only; APP-controlled prototype grants are separate. Do not
allow archive switching to escape a grant or Player replacement through agent arguments.

Keep the eight-operation concurrency limit. A timeout covers discovery and dispatch.
Cancellation addresses only the original protocol request and owner. Unknown write outcomes
are retained and reconciled, never replayed automatically. Use stable operation UUIDs;
transport success is distinct from game readiness and verification success.

Responses are bounded to 32 MiB; decoded PNG/JPEG to 16 MiB. Images are saved under
artifacts/runtime/<instance>/<operation>, preserving MCP image blocks and returning
_meta["abya/artifactPath"]. read_artifact canonicalizes paths under the selected task's
runtime root and bounds image reads. No credentials enter artifacts or workflow journals.

Frozen acceptance/preview candidates use validated .abya-play-runs/<UUID>/Archives copies
and task owner markers. Launch failure stops only newly created instances. Retained playable
checkpoints are previewable without declaring formal acceptance. Player qualification requires
actual reports and full build fingerprints; a capability catalog never proves game behavior.

## Dependencies
Foundation, game-instances and development-tasks public interfaces.

## Validation
Protocol ownership/cancellation tests and real native-client disconnect/incomplete-response
timeout fault tests (unknown outcomes, no CLI peer); actual Player launch, identity, Lua execution and
image roundtrip; native Codex workflow qualification. Synthetic tests and externally repaired
games do not prove autonomous completion.

## LLM Maintenance Rule
Update this Skill, schemas and focused tests whenever ownership, transport, artifacts or
workflow contracts change. Historical CLI transport rules are retired in this edition.
