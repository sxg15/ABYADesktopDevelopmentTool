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

APP source commit: `01ab11a` (pushed to origin/MCP). PB scoped source changeset:
`12028` on `/MCP-AI-App-20261031`. Unrelated PB import/build noise was excluded.
Final candidate executable SHA-256:
`ec0b016daaa2ec811bb9c1ccbfc3fb5ee7e7f55addc8a700d712a2bb1ae9ee70`.
Final source fingerprint:
`e7793fb5d1bd10da299b6cca2a4c721188f76f5ac452fd7f48f1acea1bec2d92`.
528 candidate runtime files verified. Final Player smoke passed again after CLI
server removal. Native client disconnect/timeout and candidate invalid-argument/stale
revision probes passed; these component probes are not gameplay completion.

Fresh native pilot root: `C:/ABYA-MCP-Pilot-20261102`.
Fish task: `5449bf95-63e5-4cbf-b43a-dec40ffdf0bd`.
Native conversation: `01a19b3d-45a4-79a2-b94d-6f9a0b189582`.
The pilot uses fixed original gameplay/art input and the independent MCP Player,
with all game writes delegated to the native APP workflow. It has not yet qualified
for release. Resume by reading `runs/fish-1/run.json` and the current APP state;
never recreate the task or overwrite approved artifacts to force a passing result.
