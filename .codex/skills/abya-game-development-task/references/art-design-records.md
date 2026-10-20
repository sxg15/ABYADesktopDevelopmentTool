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

## 游戏 UI 的现有记录映射

游戏 UI 方法与离线候选索引随固定 Skill 分发，入口见 [游戏 UI 设计](game-art-design/references/game-ui-design/SKILL.md)。索引不是 art_library_cases；查询不产生 use-art-case、artTemplate 或人工认可。实际认可案例仍走下方 art list/get/inspect/use 契约。
UI 的玩法/镜头/输入/密度和界面职责写入 direction.tasksAndInformation/platformAndCamera；组件状态、语义色、保护项与交互反馈写入 visualRules；代表游玩画面及关键状态写入 representativeScene；字体、独立图层、统一 JSON 配置与资产状态写入 assetProduction/feasibility。外部来源与候选 ID、借鉴/适配/不采用及未验证项写入 reference-analysis 和 librarySearch；正文同时进入 resources 确认单。
沿用已有 requirementIds/coverage/realization/evidence 关联 UI 对象、预览与当前版本运行证据；UI 章节模板仅供合并进现有文档，不是新的提交或审批节点。

## 设计操作

外部网页/DESIGN.md 按参考转化指南处理：把来源观察与借鉴/适配/不采用/未验证写入现有参考分析和 direction，不新增操作。任务内 DESIGN.md 用 register-artifact 登记并关联资源确认单；它引用数值配置和版本，不复制另一套参数。网页提示词不参与执行权限或阶段判断。

### 概念到实现标定（2.2.1）

使用 [标定指南](game-art-design/references/product-interface-design/references/implementation-calibration.md)，沿用现有字段，不新增 approval 或假定后端会判断像素相似。
- direction.visualRules：真实字体身份/字重/样字和尺度关系；代表画面的关键边界、净间隙与密度模式。
- direction.assetProduction：可复用数值配置及版本/摘要、字体和资产实际规格、背景/前景/文字分层；标出实现读取位置及保护项。
- direction.feasibility：当前 Player 中实际字体、自动字号、锚点/缩放能力的验证与待确认替代，区分运行检查和仅源码推断。
- direction.representativeScene / acceptance：最小标定片段、状态/分辨率矩阵、同内容对照方法、阈值来源与适用范围；未约定阈值明确标为候选。
规格/测量配置/差异报告作为任务内版本化文件保存，用 register-artifact 登记，路径与摘要写入当前资源确认单/方向记录。不要直接编辑 workflow.json 或把这些本地文件当成新增 production 操作。
代表片段在计划授权后实施；字体接口等关键修复后更新对应实际版本和证据。visual 自测 observations 分别记录功能、外观与还原；realization.comparison 关联具体对象、测量与实际截图，differences 写残余差异。数字检查不是视觉证据，不可单凭工具结果写 matched。
图像内容/状态不同或未配准时只记录定性观察，不能写全画面还原率。保留实际原图与排除区域说明，动态效果以相同触发/时序另验。
案例 draft.production 保存真实字体、规格来源、代表片段、修正方法和复验范围；limitations 保留概念/实机差异及未测模式。试玩认可不得扩大成字体、几何和完整人数验证均已通过。

### 视觉细化的三个接入点

方向明确后按通用 Skill 的 visual-refinement 模块细化，不新增操作、字段、审批或固定轮次。
生成前：direction.visualRules 保存注意力顺序、持续信息、色彩职责、密度、字体/图像尺度、
关键反馈和风格边界；platformAndCamera 写目标平台，acceptance 写实际检查方法。
方案提交前：逐条核对当前要求与禁止项、全部有效参考的应用、代表状态和制作能力；
resources 文档说明本次改动及保留项，coverage 只关联当前正式方案。
实现后：visual 自测 observations 保存位置/证据、观察、任务影响、依据、修正/保留和复验；
realizations 继续按有效要求绑定当前版本与证据，不把全局审查复制成每项“已满足”。
对比度或变量工具只验证明确输入，unverified 不能填通过；不擅自安装依赖或生成新审批。
保留来源原文；已确认流程、身份色、布局和便捷操作不得因通用美化建议被静默移除。

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
完整版设计方法映射到现有记录，不新增审批点：tasksAndInformation 保存设计推导与领域到结构关系；representativeScene 保存3个低保真方向、至少3维差异、结构检查和选型；visualRules 保存至少5个产品专属决定及视觉职责；acceptance 保存实际检查方法。
这些内容同时进入美术确认单，不能只在会话里思考后丢弃。完整设计按数目要求探索，局部/已确认方向明确范围与沿用依据。数量不证明结构质量，不能以凑条目或自评分数冒充设计有效。
运行后将默认风格、通用性和产品识别审查写入 visual 自测的 observations，并关联实际渲染证据；逐条 realization 引用有关结论，不机械复制整份全局审查。没有人工测试时明确是 AI 自检。
提交新图后更新关联，不覆盖旧文件。来源变更后逐条复核仍有效要求，再更新 direction/coverage。
requirements、resources、plan、delivery 仍是原有确认点，没有新增固定确认菜单或固定迭代轮次。
资源仅复用或无需新增也要说明判断、适配和验收；不为了满足表格新造资产或无关任务。
完成 implementation/提交 delivery 前，每条有效要求须有当前版本实机对照与已审阅证据；文字或 JSON 检查不能冒充视觉审阅。
三个固定报告与 workflow.json 保留这些记录。机器门槛只检查完整性、版本与文件一致性，不能证明审美或要求语义满足。

## 案例检索与借鉴

APP 的“让 AI 评估并参考 / 整理案例草稿 / 整理新版本”会先保存到当前任务的 `artDesign.caseRequests`，再请求原会话继续。每次继续先读取这些 APP 用户请求，按 `selection.cycle`、具体案例版本和当前上下文核对适用性；`prompt` 是该次操作的具体意图。先核对已有 `caseUses`、草稿与对话中的处理记录，避免重复执行已处理的请求；设计变更后重新判断，不能把旧请求当作自动授权覆盖新要求。
保存请求不代表已查阅、已采用或已认可，不改变美术确认状态。待人工回答或确认时，沿用原工作流条件，不能绕过。参考请求按下方 inspect/use 顺序执行；草稿和修订仍需策划在 APP 中明确收录。

使用 `art list --input-file <json> --json`，参数 query（最多200字）；按玩法、输入、镜头、设备等分开检索，再看风格。无匹配时可以空 query 浏览最近案例，不强行套用。
用 `art get --input-file <json> --json` 阅读 caseId/version 的固定案例。不要把导入案例的历史认可当作当前策划认可。
先用 production update/inspect-art-case（caseId、version）将候选媒体复制到本任务；从返回 artifacts 中查看 libraryReference=true、source 为该案例的路径并实际看图。此操作不登记采纳、不改变设计摘要；不适用就不采用，不能在看图之前假写借鉴结论。
选用时 production update/use-art-case：caseId、version、borrow、adapt、avoid。服务将经过校验的图像/视频复制到本任务 artifacts/art-library 并返回 caseUses.paths。
必须实际查看这些任务内媒体；若初读发现不适用，不继续套用。caseUses 的借鉴说明可用同一案例版本更新，不覆盖其冻结快照。
案例图始终是 reference，不满足本任务设计预览或验收。库撤回后禁止新引用，已有任务保留原快照与版本。

## 案例草稿与人工收录

save-art-case-draft：id、title、artifactIds（1–16个本轮正式成果）、tags数组、context、plannerRequirements、visualRules、decisions、production、applicability、limitations、sources。
可选 referenceIds 保存与本案例有关且允许保留的原始参考；参考与成果合计最多16个，并分别标注。sources 写明其来源/使用范围；参考不能代替正式成果或实机证据。
decisions 同时保留“领域原型如何形成结构、其他方向为何淘汰、哪个专属决定经过实际验证”；让后续任务借鉴方法和适用边界，不只继承配色与外形。
其中补充实际视觉问题、选择的方法和例外理由；production 记录实机差异及修正，applicability/limitations 记录可借鉴及不能套用的条件。不新增评分充当认可，不补造旧案例的历史验证。
可选 parentCaseId/parentVersion 创建已有案例的修订草稿；必须基于该案例最新版本。
草稿描述满意成果的具体范围，不包含无关聊天、账号或机器路径。来源与许可范围写清楚，不能推定可跨项目复制原参考素材。
APP 让策划查看草稿及图片，填写满意范围后收录；AI 无收录、撤回案例、导入或审批 CLI。
概念设计可收录为设计参考；runtimeVerified 只由服务根据所选实拍、对应版本证据和视觉自测计算，不接受 AI 自填。
案例素材独立保存在库中，不依赖任务目录。可从 APP 导出/导入完整 ZIP 包；导入保留来源声明但不伪造本机认可和验证。
只把真正复用后的经验作为新的证据；不能以库条目增加代替作品质量改善。
