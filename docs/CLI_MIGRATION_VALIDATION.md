# 纯 CLI 开发管线：实施与验收

日期：2026-09-24。核心迁移已实现；原 ABYA Desktop Development Tool 界面与业务模块保留。
本报告区分协议/实机联通验收与模型自主完成整款玩法的内容验收。

## 交付

- Publish/ABYA Desktop Development Tool.exe：原桌面软件的新构建。
- Publish/abya-desktop.exe：新增命令入口，连接同一桌面进程。
- Publish/tools/abya：固定的 Abya CLI 0.2.0 源码快照。
- Publish/runtime/node.exe：Node v24.11.1；运行时最低要求 Node 20。
- Publish/.codex 与 Publish/.grok：完整工作流 Skill、引用文档与模板。
- Publish/build-manifest.json：桌面程序、CLI 校验值及协议/Node 版本。

调用链：AI → abya-desktop → 认证的桌面本机接口 → 现有应用服务 → abya CLI → 游戏能力宿主。
无旧桌面协议适配器、无 /mcp 路由、无协议回退。Unity 的旧 HTTP server/协议分发类已移除。
内部复用的历史命名 DTO 不提供协议监听或连接功能。

## 主要修改

1. 新增 desktop_cli 模块与 Rust CLI 二进制；保留原任务、实例、日志及存档传输服务。
2. 每条命令验证任务/会话归属；间接日志及传输引用也检查对应实例。
3. 桌面连接凭证只以 DPAPI 密文登记，不出现在 CLI 参数、公开设置或进度事件。
4. 运行时通过固定 Node/脚本调用，按真实 PID 与 desktopInstanceId 验证目标。
5. 稳定会话身份、UUID 操作关联、重复请求拒绝、至多八个并发 CLI 操作。
6. 输入通过 stdin；保留全部结果和图片路径。工作流不记录原始代码/参数正文。
7. 取消先通知游戏；结果不确定时不自动重放写入。关闭终端会请求取消对应会话的活动操作。
8. 图片保存在任务 artifacts/runtime 下，原进度抽屉可预览；后端拒绝越界和非图片路径。
9. 游戏主线程队列已抽到 RuntimeCapabilityDispatcher，独立于协议宿主。
10. 桌面显式启动的开发 Player 向 ExternalCli 开放固定 239 项能力；普通 Player 与内置 APA
    保持原来的五项只读准入。该策略依赖当前 Windows 用户拥有的开发进程，不是同用户进程隔离。
11. 修复 Unity/Mono 未实现 WindowsIdentity.User 导致实例登记失败的问题，改用原生 Windows
    令牌/SID 与 ACL API；目录及凭证文件只授予当前用户、SYSTEM、Administrators 权限。
12. 双 provider Skill 改用 CLI；删除桌面配置复制模板，保留旧工作流记录的显示兼容。

## 自动化验证

- npm run check：模块 Skill、变更记录、模板导入、19 项前端测试、12 项随包运行时契约测试及前端构建。
- AbyaPB/Tools/AbyaCli npm test：26 项通过，覆盖运行时与原构建 CLI 回归。
- Rust 常规测试：74 项通过；实机测试单独按 ignore 开关运行。
- cargo fmt、cargo clippy --all-targets -- -D warnings 通过。
- 独立 Unity 副本：RuntimeCapabilityRegistryTests、CliCapabilityHttpServerTests、
  ResourceCoordinationTests 共 29 项通过。
- 真实 Windows x64 Mono Diagnostic UGC Player 构建成功；未将其等同于正式 IL2CPP 发布验收。

## 真实 Player 验证

测试名称：real_player_cli_pipeline_with_both_provider_contexts。
使用复制的存档、独立游戏数据目录及桌面托管进程；原存档和原 Unity 场景不写入。

- 启动报告 success=true、phase=ready；目标进入 CreatorEditor，而不只检查 CLI 可连接。
- 当前注册表 244 项；桌面 Player 目录 239 项，排除五项旧日志服务器控制能力。
- read_me_first、runtime_get_game_state 调用成功。
- HighImpact 的 lua_execute 完成纯 Lua 测试，实际返回 42，无后台确认框阻塞。
- ui_capture_screenshot 返回可打开的 PNG，已人工检查确为目标存档编辑画面。
- Codex 与 Grok 两种会话上下文调用均成功；这里未触发两个模型的实际推理会话。
- 另起独立 LAN Host 与 Client：两边启动报告 ready，networkGameplayReady=true，
  Client 已连接，Host serverActive=true，PID 不同，两边均建立了桌面日志会话。
- 所有测试 Player 已停止。

本地证据：D:/AbyaCliValidation/CLI-Smoke-Evidence/。
Unity XML：D:/AbyaCliValidation/CLI-Migration-20260924-results-4.xml。
Player 构建结果：D:/AbyaCliValidation/CLI-Migration-20260924-build-result-3.json。
实机测试日志：D:/AbyaCliValidation/CLI-Smoke-Desktop-Test.log。

复测前设置 ABYA_CLI_SMOKE_PLAYER、ABYA_CLI_SMOKE_ARCHIVE、ABYA_CLI_SMOKE_OUTPUT，
多人验证额外设置 ABYA_CLI_SMOKE_MULTIPLAYER=1。然后运行：

~~~text
cargo test --manifest-path src-tauri/Cargo.toml real_player_cli_pipeline_with_both_provider_contexts -- --ignored --nocapture
~~~

## 使用与剩余验收边界

- 继续打开现有桌面软件，在原任务/终端界面使用；无需改用另一个桌面应用。
- 游戏可执行文件必须是包含本次 CLI 宿主、实例身份和开发准入策略的新构建。
  本地验证包为 D:/AbyaCliValidation/CLI-Player-20260924/ABYA_PB.exe；属于开发验证版本。
- 任务终端自动得到 ABYA_DESKTOP_CLI。先运行 doctor --json、capabilities --json。
- 如原客户端还配置了旧 Abya /mcp 服务，清理该 Abya 条目；本次没有读写用户的原生登录配置。
- 独立验证副本位于 D:/AbyaCliValidation/CLI-Migration-20260924。构建准备中只在该副本内
  保存自动预检修改，并复制了 Wwise 构建依赖；原工程的未保存场景未保存、未重置。
- 尚未宣称通过：两个模型自主制作完整关卡/小游戏的内容验收、所有 239 项能力逐项实机写入、
  全部胜负/重开/断线场景、正式 IL2CPP 发布以及干净机器安装验收。
- 发布版 CLI 与随包 Node 的版本命令已运行。最后一次“使用独立配置目录启动发布版桌面软件，
  再执行 doctor/capabilities”的烟测命令被自动审批审查拒绝，返回原因仅为 blocked by policy，
  没有更具体说明；该步骤未执行，也没有更换入口绕过。因此发布版界面的首次启动仍待人工验收。
