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

## 临时文件与磁盘空间

- 开发、构建、下载、解压、联测和故障排查产生的临时文件，由创建它们的代码、脚本或维护者负责清理。开发联测默认集中在仓库已忽略的任务临时目录，APP 运行时使用应用管理的临时目录。不要在 C 盘、D 盘根目录或用户桌面随手创建“ABYA验证”“ABYA维护”等长期堆放文件的目录。确需跨盘验证时，记录本轮创建的准确路径并负责回收。
- 下载、解压和构建完成后，及时删除已用完的中间包、分片和临时工具副本。任务结束时，关闭本轮测试进程，清理隔离测试工作区、测试数据库、旧候选包和重复副本。成功、失败、取消都要有清理路径；异常退出留下的文件，在下次相关操作时按归属记录补清。需要排查或回滚时，只留必要材料，记录位置和清理时机，问题关闭或回滚窗口结束后清除。
- 清理前核对实际绝对路径、所属任务和使用状态，递归操作须限定在已确认的目录内，不跟随链接越界。不能仅凭目录名、所在盘符或修改时间删除。即使位于 `logs` 或旧验证目录，也要保留正式库仍引用的工作区、固定 Player、存档、试玩包和当前依赖。用户项目文件按用户明确选择处理。
- 开发交付前核对清理结果。对占用较大的目录，记录清理前后占用及仍需保留的内容；移入回收站的大小不能计作已释放空间，不代用户批量清空回收站。

## UI 美术还原约束

维护受管美术流程时，保留「选中基准与唯一尺寸配置 → 资源分层合成核对 → 授权后引擎代表片段全屏/局部对照 → 修正关键偏差后扩展」链路。字体/按钮缩水、分区漂移与非等比变形不得以功能通过结项。具体尺度取当前任务基准与目标设备，不把单个案例坐标或统一容差固化到所有游戏；沿用现有审批及固定任务升级入口。
