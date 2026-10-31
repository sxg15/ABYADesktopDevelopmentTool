# MCP edition implementation

User authorization: 2026-10-31. Keep Codex CLI/app-server; remove ABYA business
CLI tools and fallback paths. Work only on MCP in its independent checkout.

## Baseline and isolation

- Shared upstream base: `933f5cf` (CLI and origin/dev matched at creation).
- `6dd5294` snapshots 91 pending CLI files as a 2.11 candidate, not a qualified
  release. Original CLI files and index are untouched.
- Original checkout: `D:/Unity项目/ABYADesktopDevelopmentTool`.
- MCP checkout: `C:/Users/Indiegamespass/.codex/worktrees/abya-mcp/ABYADesktopDevelopmentTool`.
- No automatic CLI task/database import. Native conversation, task and archive
  migration must be explicit and preserve outstanding operation outcomes.
- Separate application/notification identities, data/workspace roots and ports.
- Inherited CLI Publish bytes were removed from this worktree. The MCP launcher
  additionally rejects any copied package without an explicit MCP edition marker.

## Implementation milestones

1. Independent branch and edition identity — implemented; storage isolation test passes.
2. Native APP MCP protocol/session core — implemented; two focused tests pass.
   Startup/business-dispatch wiring remains pending.
3. Codex task-scoped MCP registration; remove ABYA CLI injection — pending.
4. Native PB MCP client, ownership, receipts and long-operation recovery — pending.
5. PB native MCP transport and development allowlist in a separate PB workspace — pending.
6. Replace CLI-dependent task scripts, docs, runtime checks and packaging — pending.
7. Protocol/fault tests and real end-to-end game creation without ABYA CLI — pending.

## PB evidence and migration constraints

The inspected PB workspace is Plastic SCM, changeset 11806. A separate branch
`/MCP-AI-App-20261031` and partial workspace `D:/Unity项目/AbyaPB-MCP` were created
from that changeset. The original PB workspace was not switched. Its current
RuntimeCapabilityHost starts CliCapabilityHttpServer; Player access rejects
InternalMcp. Mcp-prefixed types alone do not establish a usable MCP endpoint.
Do not change or switch the active PB workspace. Prepare a separate workspace
and branch/profile before implementing its transport. Validate the actual
Player build; share the existing capability registry/dispatcher, not CLI HTTP.

Reuse workflow 2.11 domain services, approvals, same-unit recovery and verified
checkpoints. Preserve cancellation/unknown-write reconciliation. An MCP method
that starts abya-desktop.exe or tools/abya is not an acceptable implementation.
Protocol versions must be explicitly matched with the installed Codex client.

No MCP release is claimed at this milestone. Do not publish inherited binaries,
overwrite the CLI Publish directory, or merge this branch wholesale into dev.
