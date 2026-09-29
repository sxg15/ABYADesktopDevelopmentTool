# 新人机测试 Player 就绪验证（2026-09-29）

结论：旧游戏包导致的 receive/诊断接口缺失已解决，可以在桌面工具中继续创建新的口算 Demo。
这是构建与接口就绪验证，不表示尚未创建的口算 Demo 已通过玩法验收。

## 使用

在桌面工具“设置 → 目标游戏 EXE”选择：

`D:/AbyaCliValidation/Bot-Player-20260929/ABYA_PB.exe`

回到原“人机口算 Demo”任务，让 Agent 使用此路径重新启动托管实例、重新预检，再继续原需求。
不要仅凭界面版本号判断新旧；这是独立的新构建目录，原 CLI-Player-20260924 保留用于恢复。
本次没有更改桌面设置或已有存档，也没有创建用户的口算玩法。

## 构建来源与环境

- 游戏源码：changeset 11355，复制时主工作区干净；本次没有修改游戏业务 C#。
- 独立工程：D:/AbyaCliValidation/Bot-Player-Project-20260929。
- 使用 Assets、Packages、ProjectSettings、Tools、BuildConfigs 和完整 Wwise 工程的独立副本。
- Unity：D:/Unity Editors/Unity 2022.3.62f2c1/Editor/Unity.exe，精确匹配工程版本并已安装 iOSSupport。
- 之前误用了 D:/Unity Editors/Editor/Unity.exe（f3c1、缺 iOSSupport）；无需为此修改构建源码。
- 配置：Windows x64、UGC、Mono Diagnostic/Development，7 个场景，独立输出目录。
- 预检 runId：be7fde7d-4cc3-4f55-94b2-552828267a32，success=true、issues=[]。
- 构建 runId：2702c517-13c1-478c-a9e0-3f703ff3a667，success=true、exitCode=0、issues=[]。
- 完整 Player 内容摘要：b44efe8abf9147c3793830d308c940238cddda6942990862e3ad7ab856a168f7。

## 验证结果

- 正确 Unity 下 EditMode 30/30：BotMessageMailbox 4、BotObservationContract 5、CliCapabilityHttpServer 1、RuntimeCapabilityRegistry 20。
- 实际桌面托管 Player 目录 240 项；定义 get/set、Bot 诊断可发现。
- Codex/Grok 两种真实会话上下文均成功读取定义、调用诊断并描述 receive/submit_action 的精确 Lua API。
- 诊断返回 bots 数组、acceptanceVerified=false，保持与玩法验收的边界。
- 纯 Lua 返回 42、目标编辑器画面截图、独立 LAN Host/ClientOnly 启动与网络就绪通过。
- 新增 ABYA_CLI_SMOKE_BOTS=1 开关，使既有实机测试可重复执行该检查；没有发起模型推理任务。
- npm run check、Rust fmt、Clippy、常规 Rust 80 项测试通过；3 项可选环境测试默认跳过，真实 Player 用例另行通过。

证据：D:/AbyaCliValidation/Bot-Player-Readiness-20260929/。
Unity XML：独立工程 Logs/bot-editmode.xml；构建结果：Logs/bot-build-result.json。
新版定义写入的 schema 已发现，但本次只读，没有覆盖测试存档的人机定义。

## 继续验收

新建 Demo 无需旧的口算/颜色记忆存档。Agent 仍应完成实际报名、动作接受、计分、结算、重开和退出清理。
原交接的四玩法回归仍需对应样例；四档全量联机、跨档连玩与正式 IL2CPP 发布不在本次结论中。
