# 美术设计记录与案例库（流程 2.2）

仅 policy.artDesignVersion=1 时使用。旧任务继续原策略；升级必须从 APP 显式执行。当前 task、questionGroups、artifacts、feedback、artDesign 与 artDesignStatus 来自 production get，不从旧摘要或导出 JSON 推断。
先读 [游戏美术设计](game-art-design/SKILL.md)。所有写入沿用 production update 的 expectedRevision/data；人工认可、撤回要求与案例收录只由 APP 完成。

## 开始、续接与变更

1. 读取 artDesign.sources 中的原始任务、已提交答案、有效参考备注、人工反馈、退回意见和补充要求；这些来源由 APP 汇集。图片来源同时有 asset.id/path/sha256。
   终端对话中新提出而尚未入记录的美术要求，先用 record-art-input 追加完整原句和可定位的会话/消息来源。它标为“AI 摘录，待策划核对”，不能伪称 APP 已认证原话；对话未出现的建议不能登记成用户指令。
2. 将每个具体要求写为 save-art-requirement；图片没有原话时 quote 留空，解读必须标注推断。AI 建议不要伪装成策划来源。
3. 用 review-art-source 核对每个来源是否已完整拆解，列 requirementIds 与 reason；不涉及美术时可以空数组但必须说明，不把“已查看”当作无要求。
4. 查看并分析每张有效策划参考图；检索并查看适用案例。先阅读后判断，不用文件名代替看图。
5. 更新设计依据、代表场景、资产规格和能力预检。登记当前方案图后，逐条关联落实位置。
6. 提交本版 resources 文档。来源、要求、参考、借鉴或设计记录变化会使原版方案失效；不重复确认无关阶段。

quote/sourceId/kind 一经登记，AI 不可静默变更；补充解读可以更新。原话仍在新来源中时可刷新 sourceHash；来源已改变含义时新增要求并请策划明确修订/撤回旧项。
反馈关闭不会抹掉其要求。退出后续接仍以当前记录为准；文件导出是快照，不直接改 JSON 推进状态。

## 设计操作

| operation | data |
|---|---|
| record-art-input | id、text（完整原话）、locator（会话/消息位置）；只追加，不覆盖，来源标明 AI 摘录 |
| save-art-requirement | id（稳定编号）、sourceId/sourceHash（来自 sources）、quote（来源原句）、kind:must/preference/prohibited/clarify、interpretation、scope、acceptance |
| review-art-source | sourceId/sourceHash、requirementIds数组、reason（完整拆解或不涉及美术的理由） |
| analyze-art-reference | artifactId、role:overall/asset/layout/mood/negative、observations、borrow、avoid、application、conflicts；无冲突也说明 |
| save-art-direction | playerExperience、tasksAndInformation、platformAndCamera、visualRules、representativeScene、assetProduction、feasibility、acceptance、librarySearch；均为非空文字，最后一项记录真实检索结果与取舍 |
| save-art-coverage | requirementId、artifactIds数组、designLocation、status:addressed/conflict/not-designed；new-art 必须关联当前正式图，不能关联参考或临时图 |
| save-art-realization | requirementId、target、evidenceIds数组、comparison、differences、status:matched/mismatch/blocked；服务绑定当前实际版本与设计摘要 |

来源、要求与周期构成 inputHash；设计记录构成 designHash。摘要由 APP 计算，AI 不提供或伪造它们。
提交新图后更新关联，不覆盖旧文件。来源变更后逐条复核仍有效要求，再更新 direction/coverage。
requirements、resources、plan、delivery 仍是原有确认点，没有新增固定确认菜单或固定迭代轮次。
资源仅复用或无需新增也要说明判断、适配和验收；不为了满足表格新造资产或无关任务。
完成 implementation/提交 delivery 前，每条有效要求须有当前版本实机对照与已审阅证据；文字或 JSON 检查不能冒充视觉审阅。
三个固定报告与 workflow.json 保留这些记录。机器门槛只检查完整性、版本与文件一致性，不能证明审美或要求语义满足。

## 案例检索与借鉴

使用 `art list --input-file <json> --json`，参数 query（最多200字）；按玩法、输入、镜头、设备等分开检索，再看风格。无匹配时可以空 query 浏览最近案例，不强行套用。
用 `art get --input-file <json> --json` 阅读 caseId/version 的固定案例。不要把导入案例的历史认可当作当前策划认可。
先用 production update/inspect-art-case（caseId、version）将候选媒体复制到本任务；从返回 artifacts 中查看 libraryReference=true、source 为该案例的路径并实际看图。此操作不登记采纳、不改变设计摘要；不适用就不采用，不能在看图之前假写借鉴结论。
选用时 production update/use-art-case：caseId、version、borrow、adapt、avoid。服务将经过校验的图像/视频复制到本任务 artifacts/art-library 并返回 caseUses.paths。
必须实际查看这些任务内媒体；若初读发现不适用，不继续套用。caseUses 的借鉴说明可用同一案例版本更新，不覆盖其冻结快照。
案例图始终是 reference，不满足本任务设计预览或验收。库撤回后禁止新引用，已有任务保留原快照与版本。

## 案例草稿与人工收录

save-art-case-draft：id、title、artifactIds（1–16个本轮正式成果）、tags数组、context、plannerRequirements、visualRules、decisions、production、applicability、limitations、sources。
可选 referenceIds 保存与本案例有关且允许保留的原始参考；参考与成果合计最多16个，并分别标注。sources 写明其来源/使用范围；参考不能代替正式成果或实机证据。
可选 parentCaseId/parentVersion 创建已有案例的修订草稿；必须基于该案例最新版本。
草稿描述满意成果的具体范围，不包含无关聊天、账号或机器路径。来源与许可范围写清楚，不能推定可跨项目复制原参考素材。
APP 让策划查看草稿及图片，填写满意范围后收录；AI 无收录、撤回案例、导入或审批 CLI。
概念设计可收录为设计参考；runtimeVerified 只由服务根据所选实拍、对应版本证据和视觉自测计算，不接受 AI 自填。
案例素材独立保存在库中，不依赖任务目录。可从 APP 导出/导入完整 ZIP 包；导入保留来源声明但不伪造本机认可和验证。
只把真正复用后的经验作为新的证据；不能以库条目增加代替作品质量改善。
