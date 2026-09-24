# Codex 后台版本与任务环境修复

## 原因

- npm CLI 在 18:43 更新到 0.156.1，但桌面工具仍复用 18:30 启动的旧 app-server。
  新前台连接旧后台，实际请求仍由旧版本处理。
- ABYA 环境变量只注入了前台 TUI。远程模式的命令在 app-server 中执行，
  因而读不到 CLI 路径、任务 ID 和会话 ID。

## 修复

- 缓存后台时记录 Codex 路径、文件大小和修改时间，并在复用前检查子进程存活。
  CLI 更新或后台退出后，空闲后台会被重新创建；其他活动终端不被自动中断。
- thread/start 与 thread/resume 按会话注入五个公开 ABYA 环境变量。
  使用独立的 dotted shell_environment_policy.set 字段，不覆盖其他环境设置。
- 没有修改登录凭据、项目 trust、sandbox、approval 或模型配置。

## 验证

- Rust 77 项常规测试通过；真实 Player 测试按原有规则单独 opt-in。
- npm run check、cargo fmt、cargo clippy --all-targets -- -D warnings 通过。
- 当前 Codex 0.156.1 的真实 app-server 命令接口中，A/B 两个会话分别读到
  各自的任务和会话 ID；恢复会话仍保留这些变量。测试会话已归档。
- 独立 gpt-6-astra 请求返回 OK；无需切换模型。
- 已更新 Publish 下的桌面程序及 CLI、校验值，并正常关闭和重启原桌面程序。
- 重启后 doctor 返回 success=true、runtimeCliInstalled=true。

请重新进入原任务的终端、打开原对话并发送先前的环境诊断指令。
历史记录里的 MISSING/旧版本报错会保留，应以重启后的新一轮输出为准。
