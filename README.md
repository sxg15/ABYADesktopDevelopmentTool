# ABYA Desktop Development Tool

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
