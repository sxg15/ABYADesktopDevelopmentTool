# LLM Maintenance Contract

Release branch policy (2026-10-15): work completed on CLI must automatically be
integrated into dev, including source, module Skills, project constraints and the
verified complete portable package. This supersedes the 2026-10-14 dev-only rule.
After required checks and package validation, commit CLI, fetch origin, integrate
any newer remote CLI/dev commits without losing either branch's features, and
update dev with the validated CLI changes. Prefer fast-forward; if branches have
diverged, merge and rerun affected checks before publishing. Push CLI and dev
(atomically when supported), then verify origin/dev contains the completed CLI
commit. Do not report completion while synchronization is outstanding; report
any actual conflict or remote failure. Never force-push, reset unrelated work or
update main. No additional confirmation is needed for this routine sync.

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
