# LLM Maintenance Contract

Release branch policy (2026-10-14): all subsequent source and portable package
changes are committed/pushed to dev only. Do not update main or CLI without a new
explicit user instruction. Preserve existing dev features when integrating work.

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
