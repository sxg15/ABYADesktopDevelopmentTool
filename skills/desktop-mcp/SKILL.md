# Desktop MCP

## Purpose
Expose ABYA workflows through native local MCP while retaining Codex as the AI host.

## Ownership
Own loopback MCP serving, task-scoped authentication, protocol sessions, tool schemas
and dispatch into existing domain services. No ABYA CLI subprocess or fallback route.

## Public Contracts
POST /mcp negotiates 2025-11-25, requires a task/conversation credential issued by
foundation, and checks initialized session ownership and protocol headers. Credentials
are revoked with the native executor; callers cannot select authorization context.
The service starts on the MCP edition's separate port. The former CLI HTTP route and
named-pipe server are removed. The catalog is in registry.rs; tools.rs revalidates
ownership, stage contracts, operation receipts and APP-only decision boundaries.

Business IDs are distinct from JSON-RPC IDs. abya/operationId metadata identifies the
business operation. Runtime writes use durable receipts and never replay unknown
outcomes. Cancellation is a request, not evidence that side effects were rolled back.
Codex receives per-thread MCP configuration, not a shell executable or CLI credential.
New connections must not invalidate an already initialized session for the same task.

## Dependencies
foundation, development-tasks, codex-terminal, game-instances, game-runtime-bridge,
game-connections, game-archives, archive-transfer and logs.

## Validation
Protocol/session rejection tests, business ownership tests, native Codex MCP discovery,
real PB MCP calls and end-to-end workflow validation. Mock transport tests do not
establish gameplay completion. Full native qualification is pending until recorded.

## LLM Maintenance Rule
Update this Skill and the catalog/documentation whenever tools, authorization,
protocol support or workflow mapping change. Never reintroduce a CLI fallback.

Runtime calls accept a stable requestId UUID; results expose abyaOperationId in text and metadata. MCP cancellation is scoped by protocol session/request and reaches only that owned runtime operation. Conversation binding is automatic and is not an advertised tool.

Explicit connection reset stops serving and revokes current leases before restart; ordinary restart preserves same-endpoint leases. Reset requires reopening owned conversations for fresh credentials. No global reusable bearer token is stored.

The existing development_production_update data supports clarifiesRequirementId on a published resource question and answerRevision on the adopted art requirement. Planner-only answers, legacy linking and approvals remain APP actions. No shell command or new MCP tool is needed.

For an active resources/plan task with stage contracts, development_production_current returns designRuntimeScope with playerRequiredForSubmission=false and the currently effective prototype instance IDs and expirations. An empty grant list allows continued document work. A grant alone never authorizes an arbitrary PB tool, task or archive.
