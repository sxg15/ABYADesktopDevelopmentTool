# ABYA Desktop Development Tool

## 策划直接使用（Windows x64）

1. 切换并拉取仓库的 **dev** 分支。后续源码与完整运行包只同步 dev，不再同步 main 或 CLI。
2. 双击仓库根目录的 **ABYA 开发工具.cmd**。
3. 首次使用在 APP 设置中选择自己电脑上的 ABYA Player 路径；AI 会话使用自己电脑的 Codex/Grok 安装和登录。

`Publish/` 是完整、已编译的运行包，包含桌面程序、Desktop CLI、Node、Abya CLI、FFmpeg 和两套工作流资源。
不需要安装 Rust、Node、npm 或自行编译，也不需要 Git LFS。保持整个仓库目录结构，不要单独复制 EXE。
启动入口按自身位置寻找程序，支持任意盘符、中文及空格路径；不依赖开发者电脑的绝对路径快捷方式。
Windows 需要可用的 WebView2 Runtime；Windows 11 通常已提供，缺少时按系统提示安装微软运行时。

拉取失败或启动提示文件不完整时，先完成 Git 更新，再运行 `ABYA 开发工具.cmd --check` 校验全部运行文件。
任务、账户、登录令牌和本机路径设置属于各自 Windows 用户，不随仓库上传，不应互相复制。
更新前退出旧 APP，拉取后仍使用同一启动入口；已有任务数据不会因拉取代码被替换。

## 发布与维护

- 正式 `Publish/` 和相对路径启动入口纳入 Git；测试包、旧发布备份、日志、缓存和数据库仍忽略。
- `npm run build:portable` 构建正式包并生成 `Publish/package-files.json`，记录每个运行文件的大小和 SHA-256。
- 更新发布包后将其完整暂存，再运行 `npm run check:portable`，确认文件完整且没有被忽略规则漏掉。
- `Publish/**` 禁止 Git 自动转换换行，保证拉取后的字节与发布清单一致。
- 不将开发者本机的 `.lnk` 作为团队入口；个人桌面快捷方式可以指向自己的仓库启动器。

Portable Windows tooling for ABYA game development tasks, managed game
instances, structured runtime logs, and pure CLI AI development workflows.

## Commands

- `npm run tauri dev`: run the desktop application.
- `npm run check`: validate module Skills, changelog policy, tests, and frontend build.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust tests.
- `npm run build:portable`: build the release executable into `Publish`.

Read `AGENTS.md`, `ARCHITECTURE.md`, and the relevant module Skill before
changing production code.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## CLI 开发入口

保留现有桌面软件。启动桌面后，任务终端通过 ABYA_DESKTOP_CLI 获得 abya-desktop 的绝对路径。
执行 doctor --json 与 capabilities --json，再按 JSON schema 调用命令。
无需配置 MCP；旧服务与客户端模板已移除。完整契约见 docs/DESKTOP_CLI_COMMANDS.md。
开发时先 npm run build:cli；便携包包含 Node 20+ 与固定版本的 Abya CLI。
