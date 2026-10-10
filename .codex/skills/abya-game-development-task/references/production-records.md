# 任务资料与制作 CLI（v2.0）

仅完整制作、较大改动和指定阶段加载。状态以桌面数据库为准；workflow.json、
rounds/ 和 reports/ 是导出，不能手改来批准文档、关闭反馈或完成任务。

## 当前状态与变更

使用环境注入的 ABYA_DESKTOP_CLI；task/conversation/provider 上下文由桌面绑定。
首次 doctor、capabilities、conversation bind，再 production get --json。
读取 record.revision、固定 policy、阶段、问题、反馈和确认版本；不要按随包最新版本猜旧任务规则。
没有 record 时才 initialize，expectedRevision=0，并沿用用户已选的 questionMode。
所有 agent 变更：production update --input-file 文件或- --json，例如：
~~~json
{"expectedRevision":7,"operation":"submit-document","data":{"kind":"requirements","path":"artifacts/game-development/requirements.md"}}
~~~
冲突时读回合并，不盲目重放。outcome_unknown 可能已经保存，先读实际状态。

## 三处人工决定与问答

requirements/plan/delivery 的确认只有 APP 用户入口，没有 approve CLI。
文档由 AI 写任务内 .md，通过 submit-document 提交，后端记录摘要、修订和实际作品版本。
plan 自动绑定资源节点当前视觉产物的路径与摘要；提交后不得覆盖图片，修订用新文件。
用户确认后正文变化须重新提交；范围改变修订相关需求/计划，缺陷修正不重审无关决定。
问答 publish-questions 使用 id/title/questions/stage，当前阶段每组1–12题：
questions 项为 id/text/options/optional。绑定由桌面注入，不填写原生会话身份。
只使用 answers 最新提交版，draft 不是答案；问答提交不是文档批准。
发布后结束本轮等待，不轮询、不要求暂停或重复输入“继续”。APP负责原会话排队续接。

## Agent 操作表

| operation | data 与条件 |
|---|---|
| initialize | questionMode: ask/no-followup，可含 playerMode；仅尚未启用时 |
| configure | questionMode/playerMode/taskTemplate/artTemplate；模板为真实ID或none，已明确人数仅用户修改 |
| publish-questions | id/title/questions，可含stage；绑定由桌面注入 |
| register-artifact | stage/path/title/summary；普通草稿先保存再登记 |
| register-visual | stage/path/title/sourceType/roles/visualVersion/groupId，可含summary/source/temporary |
| update-stage | stage/summary/nextAction，可选status: in-progress/blocked/stopped；当前阶段 |
| submit-document | kind/path；需求可含taskTemplate，计划可含artTemplate，正文明确ID |
| register-approved-template | key/templateId/documentHash；仅补登记批准文档明确的同一选择 |
| register-evidence | id/path/kind/captureType/reviewed/description，可含instanceId |
| reuse-evidence | id/sourceId/reason/unaffectedScope；只复用同周期、已审阅、文件未变的证据 |
| configure-acceptance | seats:1–8，requiredTools:能力ID数组，requireUi:默认true；绑定当前登记版本 |
| save-self-test | id/status/observations/evidenceIds；绑定当前版本与周期，保留历史 |
| update-feedback | id/status/fix；status仅in-progress或awaiting-recheck；后者另需recheck/evidenceIds |
| save-issue | id/kind/status/stage/description；解决需fix/recheck/evidenceIds；blocker另需resumeWhen |
| complete-stage | resources:stage/summary；implementation:stage（验证必要自测）；closeout:stage |
| save-knowledge | entries，每项summary/status，可补采用位置与证据；无复用亦说明原因 |

问题 kind 为 defect/blocker/checkpoint/suggestion；status 为 open/in-progress/awaiting-recheck/resolved。
可指定 affectedStages 与 blockedOperations（submit-document/complete-stage），调整范围要 scopeChangeReason。
必要问题不能改成 suggestion 绕过检查；无关建议不阻塞交付。
后端验证声明与材料的一致性，不能证明游戏质量，也不能代替真实输入与视觉审阅。

## 视觉产物

sourceType 为 reference/mockup/render/gameplay/feedback；roles 为 asset-board/layout/states/effect/feedback 数组。
资源阶段必须有素材板、整体布局和关键状态，一张组合图可以有多个roles。
groupId 表示同一设计项，visualVersion 表示其方案版本；新版本使用新文件，旧路径不能覆盖。
PNG/JPEG/WebP/GIF图片上限16 MiB；MP4/WebM视频上限64 MiB。文件保存在本任务artifacts内。
设计阶段标mockup，真实游戏截图标gameplay，独立引擎渲染标render，临时资源temporary=true。
source记录来源/许可说明。游戏实拍可回填资源节点，但不会替换已批准设计图。
策划可针对artifactId反馈并上传图片/视频；AI应实际查看关联内容后处理。

## 实际版本与必要自测

保存并冷加载正确存档/关卡后，production version 使用：
~~~json
{"expectedRevision":12,"versionId":"game-v1","instanceId":"实际托管实例ID"}
~~~
来源只能是该实例的启动存档与Player，后端计算指纹，不接受编造hash或任意源路径。
更改保存内容或运行包须登记新ID，当前版本自测与交付重新核对。
selfTestChecks 来自固定policy，当前为 architecture/lua/core-loop/input/visual/lifecycle/save-reload/authority。
每项实际运行/审阅后保存具体观察与证据；单人authority可写not-applicable，其余未测不得写通过。
证据kind=image/video/json/text；captureType=full-cycle/inspection/action/frames/state/log。
同一ID不可覆盖；reviewed仅在真正审阅后设true；说明自然操作、语义事件、夹具及查看范围。
登记版本前的证据是preflight，不能冒充当前玩法通过；旧周期不能冒充当前需求周期。
复用要具体说明未受影响范围和原因；新证据记录明确来源与“不是本版本重新执行”。

## 验收启动、反馈与交付

production start-acceptance --json 使用任务的保存版本启动单人或同机多人验收。
production acceptance-status --json 返回版本、启动进度、instanceIds、失败原因。
入口核对文件指纹、能力、正确存档/关卡、各端连接、Lua失败记录和UI入口；不能代替人工试玩。
默认单人1席、多人2席；需其他人数用configure-acceptance。新版本重新配置。
给策划操作后停止自动输入；不要绕回全局旧Player或自行启动非托管进程。

record.feedback 保存用户问题、版本、实例、artifactId/attachmentIds与处理历史。
Agent只用update-feedback标处理中或修复后待复验；用户在APP关闭/重开，没有CLI代替入口。
必要自测通过后完成implementation，立即试玩，取消固定轮次和R0/R8/R12强制录像。
全部必要问题及策划反馈处理后提交delivery，用户接受当前版本后才能完成closeout。
production report生成三个固定HTML及JSON；报告保留自测、反馈、图片和历史轮次。

## 旧流程、升级和进度

固定旧policy的任务继续旧规则：save-round/set-milestone只为兼容历史1.x任务保留。
APP显式升级需停止任务终端，保留Skill备份、批准文档、轮次和证据；不补造2.0自测。
升级后按当前版本登记必要自测，无需重新确认仍有效的需求/计划；普通读取不升级。

顶部对话步骤用conversation report的plan数组，每项step/status（pending/inProgress/completed/failed）。
阶段进展用update-stage。计划和报告面向策划用自然中文，命令、技术编号和证据放详细记录。
技能只按当前操作读取；skill list看摘要，skill read仅读选中项，避免重复全库阅读。
