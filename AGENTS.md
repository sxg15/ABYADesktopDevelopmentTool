# LLM Maintenance Contract

Task-specific branch exception (user request, 2026-10-19): remove Grok on the
requested 界面优化 branch based on synchronized CLI/dev. Validate and publish the
complete feature commit to CLI then dev without changing the occupied CLI worktree
or its uncommitted work. Leave 界面优化 checked out for follow-up UI work.

Task-specific branch exception (user request, 2026-10-16): game art workflow and
reference-library work develops on 美术优化, created from synchronized CLI/dev.
After verification, push this feature branch and integrate into CLI then dev under
the normal non-destructive release policy; leave 美术优化 checked out for follow-up.

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

Before changing production code:

1. Read `ARCHITECTURE.md` and `module-registry.json`.
2. Read every `skills/<module>/SKILL.md` affected by the change.
3. Keep business logic inside the owning module and depend only on its public
   interfaces.
4. Update the affected Skill in the same change whenever behavior, contracts,
   schemas, dependencies, tests, or workflows change.
5. Update `CHANGELOG.md` for every production-code change.
6. Run `npm run check`, Rust formatting, Clippy, and Rust tests.

Do not add raw command-line input to the UI. Do not introduce or persist game
connection secrets without an explicit security-design change. Do not add
desktop MCP tools without documenting their contracts in the `desktop-mcp`
Skill.
