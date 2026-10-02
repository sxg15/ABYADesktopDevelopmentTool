# AI 开发流程第一步：Skill 与任务资料约定

日期：2026-10-02。状态：源码侧首批改造及静态/分发验证完成，尚未发布到在用便携包。
依据：[总体方案](D:/Unity项目/AbyaPB/Docs/Abya-AI开发工作流优化方案.md)、
[小桌坊参考](D:/Unity项目/AbyaPB/Docs/小桌坊工作流与Skill参考梳理.md)。

## 本次完成什么

本次先落地已授权计划的第一步，交付可被现有任务分发使用的 Skill 和资料模板。
没有提前改 APP 数据库、阶段界面或运行时 API；下阶段可以基于这些约定接入状态管理。

| 项目 | 本次结果 |
|---|---|
| 基线 | AbyaPB Plastic cs:11421；桌面工具 Git 8d47a5b，开始时桌面工作区干净 |
| 主入口 | 完整玩法、较大改动和指定制作阶段；局部修复/查日志/读报告不自动开完整流程 |
| 按需阅读 | 制作阶段、资料格式、运行实现、8＋4 分成四份参考；Bot/联机/美术条件加载 |
| 人工决定 | 需求、计划、交付三个版本确认点；已有选择与适用授权复用 |
| 模板与美术 | 模板建议随需求/计划确认，现有素材优先，去掉自动美术默认和单独强制菜单 |
| 完整迭代 | R0 后 R1—R12，后四轮全问答，固定九个维度和 16 题 |
| 任务资料 | 工作流、单轮、需求、计划、交付和收尾模板；初始均不带通过结论 |
| 原有保护 | 托管实例、Lua/架构、Prefab 版本、Bot 候选校验、未知结果读回、用户内容保护保留 |
| 维护来源 | 首批共同正文由 .codex 维护，受控同步到 .grok，保留 Grok 入口字段 |
| 分发验证 | 真实 TaskService 创建/刷新任务，核对完整资源且保留原第七轮记录与用户 Skill |

## 冲突处理

- 主 Skill 不再要求所有多操作请求在任何只读检查前都必须先建计划；多步骤仍报告有效进度。
- 模板不再作为第二套阶段和确认流程，与主入口共享记录和检查结果。
- 多人模板不自动选 comic-arcade-ui；原有 DingTalk 字体要求保留。
- 美术模板及导入器的新生成入口同步调整，避免以后导入又带回旧选择规则。
- Prefab 仍检查影响范围和运行端权限，但明确复用覆盖该范围的已有授权。
- Bot 预检复用同实例/版本的有效结果；原定义保护、候选暂存和真实验收规则保留。
- 同一次检查的结果可以引用，跨轮仍实际运行；不能用去重省掉已约定的十二轮。

官方写法依据已在总体方案第六节记录，本次采用 skill-creator 的范围、按需读取与验证要求。
此次没有修改全局 AGENTS.md；项目要求的格式、Clippy 和测试照常执行。

## 维护与使用位置

- [主 Skill](D:/Unity项目/ABYADesktopDevelopmentTool/.codex/skills/abya-game-development-task/SKILL.md)。
- [制作阶段](D:/Unity项目/ABYADesktopDevelopmentTool/.codex/skills/abya-game-development-task/references/production-stages.md)。
- [资料契约](D:/Unity项目/ABYADesktopDevelopmentTool/.codex/skills/abya-game-development-task/references/production-records.md)。
- [轮次与题库](D:/Unity项目/ABYADesktopDevelopmentTool/.codex/skills/abya-game-development-task/assets/production/workflow-policy.json)。
- [同步工具](D:/Unity项目/ABYADesktopDevelopmentTool/scripts/sync-game-workflow.mjs)：npm run sync:workflow。

任务产物保存到任务工作区 artifacts/game-development，与 APP 的对话 workflow.json 分开。
复制模板不得覆盖已有产物；旧任务映射实际资料、保留未知，不补造历史确认和轮次。
当前 APP 读取任务仍会刷新受管 Skill，因此资料固定 workflowVersion 并要求检测变化；
后端版本固定和升级控制尚未实现，不宣称正在执行的任务已获得硬性版本保护。

## 验证结果与边界

| 检查 | 结果 |
|---|---|
| 四个修改后的 Codex Skill 官方 quick_validate | 4 个通过；PyYAML 仅安装在临时校验目录 |
| npm run check | 通过：42 项 Skill/Bot/导入器检查、31 项前端测试、12 项 CLI 测试及构建 |
| cargo fmt --check | 通过 |
| cargo clippy --all-targets -- -D warnings | 通过 |
| cargo test | 83 项通过，3 项原有环境依赖测试忽略 |

3 项忽略项分别需要真实 Codex 会话生命周期、Windows 沙盒 CLI 和真实 Player 环境。
本次没有执行模型行为评测、完整玩法的 8＋4、用户试玩或便携包发布。检查通过只证明
资料结构、引用、两端一致性和实际分发行为，不证明模型每次都能正确选 Skill 或完成作品。
前端构建仍提示单个 chunk 超过 500 kB；本次未改界面代码，也未处理这个独立优化项。

## 下一步：接入可执行的任务状态

1. 在 APP/CLI 中实现任务级阶段记录、版本确认和问题/轮次读写，区别于现有对话时间线。
2. 校验缺资料、未确认、失效证据和未关闭问题，明确哪些只是提示、哪些阻止通过。
3. 固定任务采用的流程/Skill 版本，设计旧任务接续与显式升级，避免读任务时静默换规则。
4. 接入最小阶段、待确认和报告入口；提供基础 Skill 库查看/选择，复用本次配置与模板。
5. 核实实际 Player 的完整录像和审阅链路，再选首个完整玩法开展严格 8＋4 试点。

上述是后续实施范围，本次没有新增假定已存在的流程 CLI 命令或后台确认机制。
