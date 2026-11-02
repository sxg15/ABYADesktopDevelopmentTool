# MCP edition local credentials

ABYA business RPC uses native MCP over authenticated loopback HTTP. This edition
retains Codex CLI/app-server as the model provider, not as an ABYA business client.

APP tokens are random, in-memory leases bound to provider/task/conversation. They
are supplied only to the owned native app-server thread's MCP configuration. The
model shell receives task identifiers, never the bearer credential. Pause/stop and
lease revocation retain existing ownership fences. Requests revalidate context at
dispatch; client arguments cannot choose a different authorization context.

PB registration reuses the existing current-user private descriptor/Windows ACL
design under a separate `AbyaPB/Mcp/instances` namespace. The APP native client
reads it in process and validates loopback endpoint, PID and desktop launch ID.
Tokens are cached in memory, not copied into settings, game archives, task artifacts,
workflow events or diagnostic bundles. Descriptor changes invalidate cached sessions.
Public runtime DTOs omit tokens. No new internet-facing server is introduced.

This is a same-Windows-user trust boundary; another process already running as that
user can read private registrations. MCP session IDs alone do not authorize a request.
Origin-bearing requests are refused, proxy and redirects are disabled. Cancellation
is scoped to the original owner and request and does not promise rollback. Unknown
write results require reconciliation before another write; they are never silently
retried through CLI or another transport.

APP identity, data/workspace roots, ports, notification identity and release artifacts
are separate from the CLI edition. Existing CLI tasks are not imported automatically.
