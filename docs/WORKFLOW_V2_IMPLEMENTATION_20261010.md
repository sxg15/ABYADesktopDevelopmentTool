# Abya 验收与反馈流程 2.0 实施记录

日期：2026-10-10。目标版本：APP 0.4.0 / workflow 2.0.0。
依据：[修改方案](ABYA_WORKFLOW_IMPROVEMENT_PLAN_20261010.md)。

## 已实现

| 范围 | 结果 |
|---|---|
| 验收启动 | APP 与 CLI 共用任务验收编排；依据实际保存的存档和 Player 指纹启动，支持单人或 Host/Client 组 |
| 初始化验证 | 核对运行能力、存档/关卡、各端网络、Lua 失败记录和 UI 根节点，连接成功不自动算可试玩 |
| 重复与失败处理 | 同版本完整组复用；部分/旧组提示先关闭；失败只停止本次新建实例，记录清理失败 |
| 通用启动 | 优先读取任务运行包；读取完成前禁用启动；Client 继承 Host 的运行包、存档与端口 |
| Windows 路径 | 将指纹中的扩展路径格式转换为游戏启动可识别的普通路径，指纹仍验证真实文件 |
| 资源预览 | 版本化图片/视频、缩略图、双项对照、参考/示意/渲染/实拍标签、临时素材和来源 |
| 方案确认 | 执行计划绑定资源图路径及摘要；已登记图片不覆盖；文件变化阻止原版本确认 |
| 人工反馈 | 支持关联图片、截图/短视频附件、版本与实例关联、处理历史；用户关闭/重开 |
| 新流程 | 取消固定 12 轮和 R0/R8/R12 录像门槛，使用必要自测与按影响范围复验 |
| 证据复用 | 原证据须同周期、已审阅且文件未变；记录来源、未受影响范围和依据，不伪称新执行 |
| 旧任务 | 普通读取不升级；显式升级备份 Skill，保留有效需求/计划批准、周期、历史轮次和证据 |
| 报告与时间 | 报告新增视觉、自测与反馈；区分总历时、已记录推进、人工等待、阻塞和返工 |
| 两套 Skill | Codex/Grok 同步到 2.0 契约，保留旧任务固定策略的兼容说明 |

## 新任务怎样使用

1. 新建完整制作任务，按需求敲定与执行计划两个既有确认点推进。
2. 在资源节点查看素材板、布局与关键状态。设计示意、实际渲染和游戏实拍会分别标注。
3. Lua 实现与必要自测通过后，在制作流程或游戏实例页点击“一键开始验收”。
4. 试玩后直接提交问题，可关联视觉产物并上传截图或短视频。
5. AI 标记处理中/已修复待复验；策划复验后关闭反馈。
6. 满意后接受对应作品版本，再完成交付整理。

旧任务可直接使用新的验收启动入口，不必为了打开正确 Player 先升级制作策略。
如需切换人工反馈流程，在“制作流程 → 流程详情”中显式升级，先停止该任务的 AI 终端。
升级不会自动接受作品，也不会把历史 12 轮伪装成新流程必要自测。

## 自动验证

- npm run check：模块 Skill、双 provider 同步、工作流与模板检查、前端测试、CLI 测试及生产构建。
- cargo fmt / Clippy（-D warnings）/ Rust 全套测试。
- 新增覆盖：无需 12 轮交付但不能跳过必要自测、反馈关闭权限、失效候选、图片摘要绑定、
  旧任务升级保留批准与轮次、真实运行包来源、证据复用限制、Windows 启动路径。
- 前端新增覆盖：明确点击才启动、启动中禁止重复点击、切换任务丢弃旧响应、图片读取失败、
  反馈先保存再续接、旧候选不能直接关闭。

最终检查数量和发布结果见文末发布记录。

## 真实 APP 与 Player 验证

使用独立数据库、独立任务工作区及堵猫猫存档副本，不修改正式任务确认状态。

- 全局 Player 故意保留 Bot-Player-20260929。
- APP“一键开始验收”实际选用 CatTrapClockFix-20261009，启动 Host 与独立 Client。
- 两端 Gameplay/网络就绪、Lua 初始化、UI 根节点检查通过。
- 已查看真实游戏截图：同一房间 2/2、双方未准备，等待人工操作。
- 再次点击复用同一组实例，没有重复创建。
- 图片缩略图、类型/版本、临时素材标记、图片关联反馈及用户关闭反馈均通过真实界面验证。
- 上传反馈附件成功，反馈关联当前作品版本。
- 通过真实 APP 将隔离副本从 1.2.1 升级至 2.0.0：批准记录、周期和 12 轮历史保持一致，
  新 selfTests 为空，进入待验证状态，未补造通过。
- 验证完成后只停止本次测试实例。

首次隔离环境配置失败保留为诊断：游戏只识别其存档库中的副本；Mono 的用户目录与
隔离 APP 的 LOCALAPPDATA 布局不同，会导致 CLI 找不到实例。调整测试目录布局后通过，
没有为此放宽实例身份校验或复制游戏凭据。

## 证据

- [验收启动记录](test-evidence/workflow-v2/acceptance-launch.json)
- [重复启动复用记录](test-evidence/workflow-v2/acceptance-reuse.json)
- [实际双端运行状态](test-evidence/workflow-v2/real-player-readiness.json)
- [升级保留记录](test-evidence/workflow-v2/upgrade.json)
- [附件与反馈记录](test-evidence/workflow-v2/feedback-attachment.json)
- [资源预览界面](test-evidence/workflow-v2/visual-gallery.png)
- [反馈界面](test-evidence/workflow-v2/visual-feedback.png)
- [游戏 Host 画面](test-evidence/workflow-v2/player-host.png)
- [游戏 Client 画面](test-evidence/workflow-v2/player-client.png)

## 验证边界

本次证明 APP 功能、流程校验、升级兼容和真实双端启动可用。
不将这些检查表述为策划已经接受堵猫猫，不代替真实鼠标玩法体验或不同物理机器 LAN 测试。
3–4 小时是下次同规模新制作任务的目标，需要另一次完整制作计时验证。
时间统计基于已有事件；缺失/未完成执行区间仍显示未知，不声称获得纯模型工时。


## 最终检查与发布

- npm run check 通过：42 项 Skill/模板检查、58 项前端测试、12 项 Runtime CLI 测试，生产构建成功。
- Rust 格式与 Clippy 通过；127 项库测试、2 项桌面 CLI 单元测试通过，4 项独立环境测试仍按原设置忽略。
- 最终发布包再次完成真实双端就绪验证；测试修改存档副本后启动被指纹校验拒绝，没有回退旧包。
- 正式 Publish 更新为 APP 0.4.0 / workflow 2.0.0 / acceptance-feedback-20261010。
- 旧包保留于 `ReleaseBackups/Publish-20261010-171423`；正式数据库已在正常用户目录的 RecoveryBackups 备份。
- 测试实例已关闭，测试存档已移出正式游戏存档库，保留在隔离测试目录供追溯。
- 验证用浏览器缓存留在隔离目录，没有随正式包发布。

补充证据：[最终包双端启动](test-evidence/workflow-v2/final-package-acceptance.json)、
[失效存档拒绝](test-evidence/workflow-v2/stale-version-rejected.json)、
[新任务默认流程](test-evidence/workflow-v2/new-task-defaults.json)、
[正式包与原任务核对](test-evidence/workflow-v2/formal-package-verification.json)。
