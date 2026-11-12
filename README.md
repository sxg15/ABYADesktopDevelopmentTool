# ABYA Desktop Development Tool MCP

MCP 版在独立的 Windows 桌面应用中管理开发任务、游戏实例和运行日志。Codex 会话通过原生桌面 MCP 工具执行 ABYA 业务操作，运行时调用连接到支持原生 MCP 的 PB Player。

## 在 Windows x64 上使用

1. 将 MCP 便携包完整解压到独立目录；通过仓库取得正式包时，使用 **MCP** 分支的独立克隆。
2. 保留根目录的「ABYA 开发工具.cmd」、`scripts/launch-mcp.ps1` 和完整的 `Publish-MCP/`。双击启动器，它会校验清单和文件哈希，然后启动程序。
3. 在设置中选择支持原生 MCP 的 Player。AI 会话使用本机 Codex 安装和登录；桌面界面依赖 Windows WebView2 Runtime。

可用「ABYA 开发工具.cmd --check」单独校验文件。更新 MCP 包前先关闭旧实例，保留独立的 MCP 用户数据和工作区。账户、凭据和项目存档保存在本机。

## 发布与维护

MCP 工作区只向远端 MCP 分支发布。候选包位于 `Publish-Staging-MCP/`，正式包位于 `Publish-MCP/`，回滚副本位于 `ReleaseBackups-MCP/`。便携包包含 MCP 桌面程序、受管 Codex Skill 和 FFmpeg。候选验证需绑定源码指纹、可执行文件和资源清单，正式晋级条件见[发布规则](skills/release/SKILL.md)。

开发前阅读 [AGENTS.md](AGENTS.md)、[架构](ARCHITECTURE.md)及相应模块 Skill。原生任务会话与传输边界见[桌面 MCP 契约](skills/desktop-mcp/SKILL.md)。游戏美术需求、实机追踪和案例复用见[美术设计流程](docs/ART_DESIGN_WORKFLOW.md)。
