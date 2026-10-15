# Codex 长会话读取修复（2026-10-15）

截图报告原会话响应约 18.3 MB，超过 WebSocket 默认单帧 16 MiB 限制。
仓库的 Codex app-server 桥接使用默认连接配置，读取错误被统一转换为连接中断。

## 修复

- 元数据和运行时桥接共用显式配置：单帧及完整消息均限制在 128 MiB。
- 容量超限返回 `codex_response_too_large`，显示固定提示，不暴露原始响应。
- 保持原会话关联，不截断历史，不创建替代会话，不自动重发任务。
- AGENTS.md 与 release Skill 改为 CLI 完成后自动同步源码、文档与验证后的完整包到 dev，并验证远端提交包含关系。
- 发布标识：`codex-large-history-fix-20261015`。

## 验证

- 新增回环 WebSocket 测试在旧代码上复现 `codex_transport_failed`，修复后通过。
- 11 轮、约 19 MiB 单帧历史完整读取，原会话 ID、每轮 ID 和正文长度保留，后续请求成功。
- 65 MiB 分片响应成功读取；超过 128 MiB 的帧仅凭头部即被拒绝并返回专门诊断。
- `npm run check`、Rust 格式检查、Clippy、Rust 测试和发布构建通过。
- 前端 104 项测试通过；Rust 153 项库测试及 2 项 CLI 测试通过，7 项环境专用测试按原配置忽略。
- Validation 包使用独立 LOCALAPPDATA、WebView 数据目录和空数据库启动；CLI doctor 与空任务列表读取通过。
- 正式 Publish 已更新，396 个发布文件完整哈希及 Git 跟踪检查通过；原包保留在 ReleaseBackups。

本轮大响应验证使用合成原生协议消息，没有读取或恢复截图对应的用户真实会话。
这验证了已定位的传输故障，不代表对特定任务玩法或 AI 输出的验收。
