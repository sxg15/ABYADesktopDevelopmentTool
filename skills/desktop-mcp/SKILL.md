# Desktop MCP Skill

## Purpose

Expose desktop application workflows to LLM clients over local MCP.

## Ownership

Owns loopback binding, Bearer authentication, MCP sessions, protocol responses,
server status, tool schemas, risk annotations, argument validation, and
application-service dispatch.

## Public Contracts

`DesktopMcpState`, initialize, ping, `tools/list`, and `tools/call`.

The server binds only `127.0.0.1`, exposes `POST /mcp`, requires the DPAPI-backed
Bearer token, issues an `Mcp-Session-Id` during initialize, and runs blocking
application operations outside the async HTTP worker. Token regeneration
restarts the listener and briefly retries loopback binding while the previous
listener completes shutdown.

The registered tools are:

- `desktop_get_capabilities`
- `development_task_list`
- `development_task_create`
- `development_task_update`
- `development_task_set_status`
- `development_conversation_bind`
- `development_conversation_activity_report`
- `game_instance_list`
- `game_instance_get`
- `game_archive_list`
- `game_archive_transfer_target_list`
- `game_archive_transfer_source_list`
- `game_archive_transfer_start`
- `game_archive_transfer_get`
- `game_archive_transfer_cancel`
- `game_instance_launch`
- `game_instance_set_window_visibility`
- `game_instance_stop`
- `game_instance_wait_for_state`
- `game_instance_wait_for_mcp`
- `game_instance_get_launch_report`
- `game_instance_mcp_get_state`
- `game_runtime_list_tools`
- `game_runtime_call_tool`
- `game_instance_mcp_tools_list`
- `game_instance_mcp_call`
- `game_log_collection_start`
- `game_log_collection_stop`
- `game_log_source_list`
- `game_log_session_list`
- `game_log_query`

Read `docs/DESKTOP_MCP_TOOLS.md` for each input contract and side effect. Most
tool results use JSON text content and set `isError=true` for validation,
application, or unknown-tool failures. `game_runtime_call_tool` and its legacy
alias preserve the target Runtime MCP's complete content array, including
native image blocks, and propagate its `isError` value.

`development_conversation_bind` requires an initialized MCP session and binds
that session in memory to one existing task-owned Codex or Grok conversation.
Its optional `provider` is `codex` or `grok` and defaults to `codex` for
backward compatibility.
`development_conversation_activity_report` records a sanitized semantic
milestone against the bound conversation. Reusing the same summary updates the
current matching reported milestone from started/progress to completed/failed.
After binding, every other Desktop MCP tool call is automatically recorded as
a start plus completion/failure activity with sanitized input arguments. MCP
session bindings are disposable and are not authentication or conversation
credentials.

`desktop_get_capabilities` includes non-secret game gateway state.
`game_instance_launch` accepts only typed production launch choices and falls
back to the configured game executable path. New launches default to an
off-screen, non-activating background window that remains rendered, and they
pass `--abya-mcp-auto-approve=true` so Runtime MCP HighImpact tools skip the
in-game confirmation dialog.
`game_instance_set_window_visibility` reversibly restores a running managed
window without activation or returns it to the render-preserving background
state.
`game_runtime_call_tool` is a
deliberately powerful pass-through for running managed instances; external
instances are logs/archive-transfer only. Callers must inspect
`game_runtime_list_tools` first. The older `game_instance_mcp_tools_list` and
`game_instance_mcp_call` names remain compatibility aliases.

Archive transfer discovery is read-only. Starting a transfer is an open-world
destructive operation because an accepted game-side request replaces the
archive with the same GUID. Cancellation is also open-world and destructive
to the target's staging operation. Transfer results never expose package
contents.

Desktop and generated game MCP tokens must never appear in tool definitions,
results, logs, or persisted launch data.

The Settings view owns the task workspace root selector and copy-ready client
configuration templates for Codex, Grok, Claude Code, Visual Studio Code,
Cursor, Windsurf, Gemini CLI, and generic MCP JSON. Templates use the current
loopback endpoint and desktop Bearer token, remain read-only in the UI, and
must be updated when a supported client's configuration contract changes.
Codex TOML uses `http_headers`; Grok TOML uses `headers`.

## Dependencies

Depends on foundation, development tasks, shared development-terminal
contracts, Codex terminal, Grok terminal, game instances, the game runtime
bridge, game logs, game archives, and archive transfer. The dispatcher composes
those public services; transport code must not duplicate their business logic
or write provider workflow files directly.

## Validation

Test loopback-only binding, authentication, initialize negotiation, ping,
unique tool names, object schemas, argument rejection, application-service
delegation, provider-aware conversation binding, reported milestone lifecycle, automatic
bound-tool start/completion/failure recording, Runtime MCP image preservation,
wait/report tools, downstream errors, token regeneration, template syntax,
endpoint injection, token injection, and provider-specific Codex/Grok TOML
authentication fields.

## LLM Maintenance Rule

Settings exposes the Tauri `repair_storage` command as a one-click local
storage recovery action; it runs off the UI thread and is not an MCP tool.

When changing MCP transport, authentication, advertised capabilities, or adding
any tool, update this Skill in the same change.
