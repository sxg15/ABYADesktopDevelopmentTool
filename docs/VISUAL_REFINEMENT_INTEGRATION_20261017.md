# 视觉细化整合与验证记录

发布标识：visual-refinement-integration-20261017；APP 0.4.0，制作流程仍为 2.2.0。
基线：47500d1。保留当前流程工作台、人工确认边界和系统代理修复。

## 已落地

- 两个设计 Skill 保持主入口，通用 Skill 内新增按问题读取的视觉细化模块。
- 生成前约束、提交前核对、实现后审查分别映射到现有 direction、resources、visual 自测及 realizations；没有新数据库字段或审批点。
- 层级、排版、间距、颜色、层次和反馈规则包含适用条件；中文、多面板密度、身份色与风格化表现保留合理例外。
- 案例现有决策/制作/适用字段补充问题、方法、例外原因和实机差异；满意收录仍只由用户执行。
- 方案旁展示记录的视觉约束和当前版本实机差异；案例详情先展示满意范围、适用条件和限制。
- 自行实现便携 Node 对比度/变量工具，修正上游的大字说明、字体引号和对象颜色导出问题；不增加 Python/Tailwind 依赖。
- 主源为仓库 .codex 下的完整游戏 Skill；sync:workflow 同步 .grok，sync-design-skills.mjs 显式备份并同步个人两个入口。
- 保留上游来源和 MIT 许可材料；不分发书中图片或正文。

## 自动与原生验证

| 检查 | 结果 |
|---|---|
| npm run check | 164 项前端测试、45 项 Skill/工作流检查、27 项 CLI/包检查通过；TypeScript 与 Vite 构建通过 |
| Rust | fmt、Clippy 通过；202 项库测试与 2 项 CLI 测试通过，原有 11 项 opt-in 测试仍忽略 |
| 新工具 | 18px 普通字不按大字放宽；粗体阈值、未知输入、字体引号、对象颜色、变量引用及循环检测通过 |
| 分发 | 两个执行端全部相对引用存在，资源同步一致；个人 Skill 同步一致并保留备份 |
| 原生候选包 | 创建隔离任务、部署两个 Skill、实际运行随包工具、要求遗漏/降级拒绝、案例请求与收录边界通过 |
| 迁移包 | 中文及空格路径下重复原生验证通过，439 个文件完整哈希验证通过 |

官方 quick_validate.py 因当前 Python 缺少 PyYAML 未能运行；对本次两个仅含简单标量
name/description 的入口完成命名、字段、长度、占位符检查，另有递归相对引用验证。
这不替代任意 YAML 结构的通用校验。

第一次原生测试在 Vite 监听仓库内测试目录时遇到 Windows Skill 目录重命名拒绝。
停止本次两个预览进程后同一原生检查通过；迁移到新目录也通过，未修改真实用户目录或安装行为。

## 受控画面对比

相同基线任务数据，窗口 1440×960、1100×760、960×720；流程页保留要求、引导与导航，
页面无横向溢出。实际查看截图确认新增上下文可读、旧控件仍可访问，案例认可范围没有变成任务验收。
这些是浏览器视口检查，不是 Windows 125%/150% 原生 DPI 全覆盖。

- [基线流程](test-evidence/visual-refinement-20261017/before-workflow-wide.png) / [当前流程](test-evidence/visual-refinement-20261017/after-workflow-wide.png)
- [基线案例库](test-evidence/visual-refinement-20261017/before-library-wide.png) / [当前案例库](test-evidence/visual-refinement-20261017/after-library-wide.png)
- [HUD 受控样本](test-evidence/visual-refinement-20261017/hud-controlled-comparison.png)：同背景与对象状态，保留四种身份色、五枚物品及两枚已消耗，用局部底板、层级和轮廓改善阅读。
- [浏览器检查记录](test-evidence/visual-refinement-20261017/report.json)

HUD 为程序绘制的规则示例，不是游戏实机、Astra 输出或策划认可。此次没有运行新游戏
生成任务，也没有同模型生成 A/B 或人工盲评；不能据此声称“生成质量已证明提升”或“完美还原”。
后续真实使用仍需按同需求/参考/模型配置记录结果，并由策划明确评价与选择收录。

## 使用与回退

新任务随包获得完整方法；已固定任务继续原版本，通过 APP 显式升级才采用新内容。
个人 Skill 更新前备份位于 .codex/skill-backups；正式包推广由现有脚本保留 ReleaseBackups。
版本回退不删除任务历史、证据或案例；已升级任务不能靠替换安装目录伪装降级。
本轮不启动正式 APP，不迁移或修改用户的实际任务。
