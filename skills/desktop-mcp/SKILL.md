# Desktop MCP

Owns the native loopback MCP gateway, task-bound credentials, protocol sessions
and tool dispatch. This replaces the ABYA business CLI; it must never spawn
that CLI or silently call its HTTP endpoint.

The current compatibility implementation negotiates MCP 2025-11-25 explicitly,
serves JSON responses on POST /mcp, and exposes only its supplied tool catalog.
The APP provisions credentials bound to a validated Codex task/conversation;
MCP callers cannot mint credentials or select their authorization context.
Initialization, session ownership, protocol headers and revocation are checked.
The business dispatcher must independently revalidate task/instance ownership.

Business operation IDs are separate from JSON-RPC IDs. The result carries
abya/operationId metadata. Unknown writes are reconciled through operation
receipts, never automatically replayed. Protocol disconnect or cancellation
does not by itself prove a business operation stopped.

Status: transport implementation in development. The gateway is not wired into
startup yet. CLI retirement, Codex registration, native PB MCP, progress/cancel
integration and full release qualification remain outstanding. Do not claim
that a passing protocol test proves a pure-MCP production workflow.
