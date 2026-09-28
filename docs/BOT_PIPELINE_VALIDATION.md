# 人机开发流程接入：实施与验证

日期：2026-09-28。流程与本地检查器已实现；真实 Bot 玩法交付验收尚未完成。
源代码基线：桌面 238a687，游戏 changeset 11352；改动留在本地，未提交。

## 已落地

- Codex/Grok 公共任务 Skill 按需进入人机规划；多人模板引用同一套指南。
- 增加候选定义暂存例外：检查已有基础和 Lua 后，只暂存可撤销定义，再校验完整架构，最后保存/重载。
- 在任务侧保留 before/current/candidate/reloaded 定义；明确整体替换、稳定 ID 合并、读回及恢复规则。
- 本地 prewrite 检查误删/误改无关定义、写前内容变化、清空授权、四档参数、脚本来源和计划覆盖。
- 数字/名称难度枚举规范化，避免实际 get 返回数字而 set 使用名称时产生虚假内容差异。
- 增加版本化 plan/validation Schema、模板和四个独立 Node 模块；无需额外 npm 运行依赖。
- 验收检查关联存档 GUID、定义摘要、完整存档目录、Player exe/Data/运行库、provider、批次和实例。
- 从捕获文件执行 JSON Pointer 断言；动作接受必须有玩法状态变化；双端同步直接比较两端数据。
- 检查必测用例、独立客户端和四档覆盖；材料无效/运行未验证/已通过使用不同退出码。
- 部署测试覆盖新任务与旧任务刷新，逐文件比较两 provider 资源并执行已部署入口验证导入。
- 发布预检要求全部人机资源；-Staging 将包生成到固定 Publish-Staging-Bot，避免覆盖正在运行的桌面程序。

## 自动化验证

- Bot 工作流检查：34 项通过，包含正常材料与失败反例；使用合成材料，不是玩法验收。
- npm run check：模块规则、模板/工作流检查、前端与 CLI 测试、TypeScript 和 Vite 构建通过。
- Rust 常规测试：库 78 项、CLI 2 项通过；3 项实机/环境相关测试默认跳过。
- Rust fmt 与 Clippy（所有 targets，warnings 为错误）通过。
- skill-creator 的 quick_validate：Codex 公共 Skill、多人工具模板通过。
- Grok 保留自身 frontmatter 与原生计划规则；共用人机资源与 Codex 完全一致并有部署/执行测试。
- 便携发布构建通过：Publish-Staging-Bot 内程序/CLI 校验值与 manifest 一致，20 个人机新增配套文件与源码一致。
- 使用包内 Node 分别加载包内 Codex/Grok 检查器，2 项材料检查烟测通过。

## 实际托管调用链复测

执行已有 real_player_cli_pipeline_with_both_provider_contexts 测试；使用复制存档、隔离数据目录及托管进程。
本次补充保存能力目录，校验必要能力与旧日志服务排除项，不再把能力总数写死为 239。

- Player：D:/AbyaCliValidation/CLI-Player-20260924/ABYA_PB.exe，历史 CLI 开发验证构建。
- 存档：CLI-Smoke-Archive 内“合作模式-1”，只用作传输/加载复测，不是交接人机样例。
- 创建编辑实例并达到 CreatorEditor，读取说明/状态、执行纯 Lua 返回 42、截图通过。
- 两 provider 会话上下文均可调用；没有运行两个模型的自主推理任务。
- 独立 Host 与 ClientOnly 启动、联网、运行状态检查通过；测试结束自动停止托管 Player。
- 已打开截图确认是实际目标存档的编辑画面。
- 实测能力目录 239 项，定义 get/set、架构检查与存档保存存在；runtime_bot_diagnostics 不存在。

原始证据目录：D:/AbyaCliValidation/Bot-Pipeline-20260928。
基线摘要：[BOT_PIPELINE_BASELINE.json](BOT_PIPELINE_BASELINE.json)。
本机测试日志：logs/bot-pipeline，包含 publish.txt。

## 当前部署与恢复

- 完整新验证包：Publish-Staging-Bot/，build-manifest.json 时间为 2026-09-28T11:31:26Z。
- 已确认现有 Publish 的相关资源与修改前基线一致，随后备份并同步 30 个受管流程文件。
- 备份：Publish/.backups/bot-pipeline-20260928/；manifest.json 记录原有/新增路径及前后哈希。
- 更新记录：Publish/bot-workflow-update.json。现有任务下次读取时由原部署机制刷新两套 Skill。
- 没有强制重启活动会话；正在进行的会话需重新读取更新后的 Skill 才能采用新流程。
- 当前 Publish 可执行文件未覆盖，游戏 Player 也未替换。完整验证包可在合适时机切换使用。
- 回退时先核对当前文件仍等于本次部署哈希，再恢复备份原文件；新增项按 manifest 逐项处理。
  不覆盖部署后用户再次修改的文件，也不删除其他 Skill、任务产物或验收记录。

## 当前阻塞与下一步

1. 交接包未附快速口算/颜色记忆完整存档；默认存档目录及已知任务目录未找到，已询问用户路径。
2. 历史验证 Player 缺少新版诊断能力；需要包含已合入 Bot 改动的实际游戏构建，并逐项发现 Lua Bot 接口。
3. 上次 Unity 编译存在 iOS/Xcode 依赖及补丁版本匹配问题；本次未改游戏 C#，未重跑或绕过该编译问题。
4. 取得样例和新构建后，在任务副本执行定义增量写入、保存重载、四档策略和完整双端玩法用例。
5. 完成口算后用颜色记忆验证通用性；没有这些证据，P0/P5 及整体运行验收继续保持未完成。

## 实现边界

本地检查器不编译 Lua、不证明 TextResource 内容可运行、不提供原子版本锁、不验证材料真实性。
Lua/资源/架构需实际工具校验，玩法意义需审阅真实状态与运行记录；不能另造 JSON 冒充采集结果。
首版仍要求同一存档单一写入方；检查报告不能强制锁定桌面后端任务状态。
交付未包含 APA 权限扩展、新 MCP 通道或 UI/数据库字段；未声称四档全量实机矩阵、连玩或发布级 IL2CPP 通过。
人机完整玩法验收仍依赖新版 Player 与原始样例，流程资源部署不等于运行验收通过。
