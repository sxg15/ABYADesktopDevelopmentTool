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

1. Independent edition, paths, ports and package identity — implemented.
2. Native APP MCP serving and domain dispatch — implemented; scoped authentication,
   cancellation, stable operation IDs and protocol tests pass.
3. Codex task-scoped registration — implemented. Installed Codex 0.156.1 reports
   `abya` connected with the actual APP tool catalog. Actual native inference completed a read-only
   development_task_list call and returned the isolated task title.
4. Native Rust PB MCP client — implemented, with exact process identity, private
   discovery, image artifacts, bounded transport and unknown-write receipts.
5. Native PB MCP host — implemented in the independent Plastic workspace.
6. ABYA business CLI binaries/wrappers removed; Skills and packaging migrated.
7. Real Player transport smoke — passed: 241 capabilities, Lua return value and
   screenshot roundtrip. This reused a fixture and is not fresh-game completion.
8. Complete native autonomous game pilots and release promotion — pending.

The first real Player smoke evidence is stored at
`C:/ABYA-MCP-Qualification-20261031/transport-smoke`. Component validation on this
revision: npm checks passed; Rust 347 passed, 11 explicitly ignored integration tests.
Final candidate source/package fingerprints must be rechecked after remaining edits.

## PB evidence and migration constraints

The inspected PB workspace is Plastic SCM, changeset 11806. A separate branch
`/MCP-AI-App-20261031` and workspace `D:/Unity项目/AbyaPB-MCP` were created
from that changeset. The original PB workspace was not switched. The baseline
RuntimeCapabilityHost started CliCapabilityHttpServer and rejected InternalMcp in Player.
The MCP workspace now starts the native MCP server and rejects ExternalCli; actual
Player smoke establishes that this is a serving transport, not just renamed types.
Do not change or switch the active PB workspace. Prepare a separate workspace
and branch/profile before implementing its transport. Validate the actual
Player build; share the existing capability registry/dispatcher, not CLI HTTP.

Reuse workflow 2.11 domain services, approvals, same-unit recovery and verified
checkpoints. Preserve cancellation/unknown-write reconciliation. An MCP method
that starts abya-desktop.exe or tools/abya is not an acceptable implementation.
Protocol versions must be explicitly matched with the installed Codex client.

No MCP release is claimed at this milestone. Do not publish inherited binaries,
overwrite the CLI Publish directory, or merge this branch wholesale into dev.

## 2026-11-02 resume

The interrupted PB build completed; focused Unity validation passed 56 tests, including
MCP transport, capability access and focus parity. Lua parity/AOT checks passed with
31 unrelated historical planned mappings. Windows launcher verification passed nine
package tests. Final MCP gameplay pilots remain pending; no release promotion occurred.
