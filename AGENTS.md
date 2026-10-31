# LLM Maintenance Contract

## MCP branch exception (user instruction, 2026-10-31)

Develop the MCP edition only on branch MCP in this independent worktree.
Do not switch, stage, commit, reset, clean, build into, or publish over the
original CLI checkout. Do not merge MCP-specific changes into CLI/dev/main.
Shared fixes may be transferred as reviewed, scoped commits. Preserve the
snapshot provenance recorded in docs/MCP_IMPLEMENTATION.md. The CLI-only
branch/release instructions below apply to the CLI edition, not MCP.
Keep Codex CLI/app-server; ABYA business operations must use native MCP,
never a hidden CLI wrapper or fallback. Use distinct application identity,
data/workspace roots, ports, notification identity and release artifacts.
An inherited CLI Publish directory is not an MCP release and must not launch
from this branch. Native end-to-end qualification is required before release.

Release branch policy (user clarification, 2026-10-15): all development happens
on CLI. Before editing, inspect the working tree, fetch origin/CLI and origin/dev,
switch to CLI, and integrate both latest remote branches into CLI. Preserve local
changes and all features; do not develop directly on dev. Prefer fast-forward;
when histories diverge, merge and resolve conflicts without dropping either side.
After implementation and required checks/package validation, commit and push CLI,
then integrate that completed CLI commit into dev and push dev. Include source,
module Skills, project constraints and the verified complete portable package.
Fetch again before publishing to account for concurrent changes; merge any new
changes into CLI and rerun affected checks/rebuild if the combined code changes.
Verify origin/dev contains the completed CLI commit and return the working branch
to CLI. Do not report completion while synchronization is outstanding; report any
actual conflict or remote failure. Never force-push, reset unrelated work or update
main. This supersedes historical dev-only rules. No additional confirmation is
needed for this routine sync.

Before changing production code (load context according to the change):

1. Locate ownership with `module-registry.json`; read relevant `ARCHITECTURE.md` sections when crossing boundaries.
2. Read affected `skills/<module>/SKILL.md` current contracts. Load linked history only for the affected surface; do not read all modules or dated release history every turn.
3. Keep business logic inside the owning module and depend only on its public
   interfaces.
4. Update the affected Skill in the same change whenever behavior, contracts,
   schemas, dependencies, tests, or workflows change.
5. Update `CHANGELOG.md` for every production-code change.
6. During iteration run focused checks for the affected contract. Before release run `npm run check`, Rust formatting, Clippy and Rust tests; reuse successful results unless relevant inputs change. Package validation is still required.

Do not add raw command-line input to the UI. Do not introduce or persist game
connection secrets without an explicit security-design change. Do not add
desktop MCP tools without documenting their contracts in the `desktop-mcp`
Skill.
