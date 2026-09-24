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

## Windows sandbox sessions
Managed terminals use a random local-only named pipe with explicit owner and available CodexSandboxOffline/Online SID ACLs. After reading each bounded request, inspect peer identity without retaining impersonation. Validate the in-memory capability against the entire provider/task/conversation context before dispatch. A managed session cannot create/list global tasks. Requests are capped at 4 MiB, responses at 32 MiB, concurrent connections at 32, initial reads at 5 seconds and operations at 720 seconds. Duplicate IDs share the HTTP registry. Service stop revokes all capabilities and closes pipes. Native provider session configuration receives ephemeral credentials; ABYA never writes them to its logs or project files. The standalone DPAPI descriptor is not read by managed CLIs. Run sandbox_pipe_authentication_smoke alone with --ignored; ABYA_TEST_MODEL=1 also runs the model-backed test script.


Pipe clients request explicit data rights without FILE_CREATE_PIPE_INSTANCE and verify ABYA_DESKTOP_PID before transmitting their session credential. Sandbox SIDs cannot create additional server instances. Invalid sessions and server identity mismatches use CLI exit code 4.
