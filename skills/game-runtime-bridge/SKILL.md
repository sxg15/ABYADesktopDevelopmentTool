# Game Runtime Bridge Skill

## Purpose

Connect to each managed game instance's authenticated local Runtime MCP HTTP
endpoint.

## Ownership

Owns MCP initialize/session state, non-secret connection state, game tool
discovery, bounded long-running tool calls, and complete MCP content results.

## Public Contracts

Runtime connection state, wait-for-ready behavior, game tool discovery, and
typed MCP call results.

The bridge obtains the live endpoint and token from the game-instances module,
initializes Streamable HTTP with Bearer authentication, caches only the
non-secret MCP session ID/server information, and reinitializes once when a
session expires. Tool calls may run for up to ten minutes. Returned `content`
blocks remain intact, including native `text` and `image` blocks. External or
stopped instances are rejected.

Read `docs/ABYA_GAME_RUNTIME_CONTRACT.md` before changing game MCP methods,
headers, protocol negotiation, or log server discovery.

## Dependencies

Depends on foundation and game instance identity/ephemeral endpoint access. It
must not own UI, durable secrets, processes, or log storage.

## Validation

Use an authenticated fake HTTP Runtime MCP to test initialize, session headers,
`tools/list`, text/image result preservation, timeouts, session refresh, and
stopped/external-instance rejection.

## LLM Maintenance Rule

When changing game MCP protocol behavior, tool names, retries, or result
contracts, update this Skill in the same change.
