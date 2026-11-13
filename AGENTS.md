# LLM Maintenance Contract

每次交付新版桌面工具，按 skills/release/SKILL.md 的「项目接续与旧包清理」执行。验收覆盖更新前 App 内的全部项目（包括见缝插针）。以新正式 Publish 的实际回读结果作为清理旧版本的前提。

Branch and release policy (2026-11-13): develop on dev. Fetch origin/dev before editing and again before publishing. Integrate concurrent changes, validate the combined sources and the complete portable package, then push the source and versioned Publish together to dev. Verify the remote commit and update manifest. Keep dev checked out. Never force-push.

Before changing production code (load context according to the change):

Windows releases have one permanent installation directory, `Publish`. Build to temporary staging, verify the full inventory, close the old installation and replace its directory as a unit. After successful verification remove the superseded installation and staging copies. Keep databases, project workspaces, pinned Players and saves outside package cleanup. Point shortcuts, task CLI bindings and acceptance launchers at the formal package.

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

## UI 美术还原约束

维护受管美术流程时，保留「选中基准与唯一尺寸配置 → 资源分层合成核对 → 授权后引擎代表片段全屏/局部对照 → 修正关键偏差后扩展」链路。字体/按钮缩水、分区漂移与非等比变形不得以功能通过结项。具体尺度取当前任务基准与目标设备，不把单个案例坐标或统一容差固化到所有游戏；沿用现有审批及固定任务升级入口。
