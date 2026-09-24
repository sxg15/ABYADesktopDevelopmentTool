# Desktop CLI

## Purpose

Expose the existing desktop application through a pure CLI workflow.

## Ownership

Own authenticated loopback command dispatch, schemas, task/conversation validation and semantic activity correlation.

## Public Contracts

POST /api/v1/command accepts version=1, UUID requestId, command, object arguments and optional provider/taskId/conversationId context. No MCP routes, sessions or fallback exist. Authentication uses a DPAPI-protected descriptor in current-user application data. Origin-bearing requests are rejected. Requests are limited to 4 MiB; duplicate IDs are rejected. Command contexts are validated through the owning provider service on every call. Instance operations require matching task ownership. Bootstrap task list/create, doctor and capabilities may omit conversation context. Commands retain existing domain IDs except deleted protocol aliases. See docs/DESKTOP_CLI_COMMANDS.md.

The abya-desktop binary reads JSON from stdin or a file and emits one JSON result. The terminal supplies its absolute path as ABYA_DESKTOP_CLI. Runtime commands use the existing Abya CLI through runtime-bridge. Content retains all text and image file paths. Credentials and raw outputs never enter semantic workflow events. Settings shows connection state and reset/restart actions; no client configuration templates or tokens are exposed.

## Dependencies

Foundation plus task, provider terminal, instances, runtime bridge, archives, transfers, connections and logs public services.

## Validation

Authenticated command integration, Origin rejection, old endpoint absence, schema uniqueness, task ownership, both providers, activity updates, runtime failures and command parsing.

## LLM Maintenance Rule

Update this Skill, command documentation and tests whenever these contracts change.

Managed log-session and transfer IDs are resolved to their instance before ownership checks. External sources retain logs/archive-transfer access only. The real Player opt-in test verifies CreatorEditor readiness, HighImpact pure Lua, screenshots, both provider contexts and independent LAN Host/Client readiness without launching model inference.
