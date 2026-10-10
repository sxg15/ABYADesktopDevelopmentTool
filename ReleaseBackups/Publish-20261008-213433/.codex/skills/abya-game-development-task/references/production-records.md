# 任务资料与制作 CLI（v1.2.1）

仅完整制作、较大改动和指定阶段加载。普通任务不自动启用完整流程。
状态以桌面数据库为准；artifacts/game-development/workflow.json、rounds/ 和 reports/
是可恢复导出，不能通过手改这些文件批准文档、关闭轮次或标记任务完成。

## 先读当前状态

使用环境注入的 ABYA_DESKTOP_CLI；已建立的 task/conversation/provider 上下文直接复用。
首次运行 doctor、capabilities、conversation bind，再用 production get --json。
从返回 record 读取 revision、questionMode、固定 policy、当前阶段、问题和已确认版本。
没有 record 时才用 initialize，expectedRevision=0，并沿用用户已选的 ask/no-followup。
APP 创建完整任务已初始化时不要重复提问或初始化。旧文件会保留为 legacy 资料，不补造通过。

所有变更通过 production update --input-file <文件或-> --json，输入对象为：
~~~json
{"expectedRevision":7,"operation":"submit-document","data":{"kind":"requirements","path":"artifacts/game-development/requirements.md"}}
~~~
任务 ID 由受管上下文绑定。每次使用最新 revision；冲突时读回、合并本次变化，不盲目重放。
导出失败可能发生在数据库已经保存之后；遇到 outcome_unknown 先读 production get。

## 三处人工决定

需求、计划、交付正文由 Agent 在任务工作区编写；提交后后端计算文件哈希并保留版本。
使用 submit-document，kind 为 requirements/plan/delivery/closeout，path 为任务内相对 .md。
提交后引导用户在 APP“制作流程”中阅读、确认或退回；不要再要求对话重复确认。
没有 approve CLI，不能将模型填写的 accepted 当作用户授权。
用户“确认并继续”会保存决定并向已打开的任务终端发送继续提示；终端未打开时手工继续。
恢复时读取真实记录，不能仅凭一条“已确认”文本跳过后端状态。
确认后正文变化须重新提交；需求或计划变更保留历史，开启新的检查周期并使后续阶段失效。
反馈是新范围则修订相关文档，是当前缺陷则修复复验，不重审无关决定。

## 可执行的操作

| operation | data 与条件 |
|---|---|
| initialize | questionMode: ask/no-followup；可含 playerMode，未明确时 unspecified；仅尚未启用时 |
| configure | questionMode、playerMode、taskTemplate、artTemplate；playerMode 为 unspecified/single/multiplayer；已明确的人数由策划在 APP 修改；模板使用真实 ID 或 none |
| publish-questions | id/title/questions；仅需求阶段且允许提问，每次一组，发布后结束当前轮等待策划 |
| submit-document | kind、path；需求确认后完成 resources，才提交计划 |
| complete-stage | resources: stage/summary；implementation: stage/checks；closeout: stage |
| register-evidence | id/path/kind/captureType/reviewed/description，可含 instanceId；必须是实际存在的任务内文件 |
| set-milestone | name: R0/R8/R12，evidenceIds |
| save-issue | id/kind/status/stage/description；解决时 fix/recheck/evidenceIds；blocker 还需 resumeWhen |
| save-round | number/close/checks/questions/evidenceIds；先 close=false 开始本轮，取证后再关闭 |
| save-knowledge | entries 数组，每项 summary/status，可附采用、候选、用途和证据；无复用也说明原因 |

初版 checks 必须包含 architecture、lua、save-reload、runtime，各自 status=passed、
evidenceIds；它们是实际校验的记录，不是让模型生成一个成功结果。
issues.kind: defect/blocker/checkpoint/suggestion；status: open/resolved。
必要缺陷不能改名为 suggestion 绕过检查；无关建议不阻塞交付。
当前后台只检查材料与已声明结果，不能代替实际操作、规则判断和视觉审阅。

## APP 需求问答

~~~json
{"expectedRevision":1,"operation":"publish-questions","data":{"id":"player-mode","title":"先敲定玩法人数","questions":[{"id":"mode","text":"这次制作单人还是多人玩法？","options":["单人","多人"],"optional":false}]}}
~~~
只问用户尚未明确且影响方案的问题，每组 1–12 题；没有默认提交选项，自由答案同样有效。
题目 ID 组内唯一。会话关联由桌面注入，不填写或猜测 provider/conversationId/nativeSessionId。
production get 返回 questionGroups：pending 是草稿，submitted 的 answers 最后一版才是已提交答案。
不要把 draft 或旧版答案当成决定。不得代策划调用用户答题接口或伪造答案。
策划可以返回上一题、重启后恢复草稿，或修改已提交答案；以最新版为准，保留需求来源。
“提交并继续”保存答案并登记继续请求。AI 忙时等当前轮次结束，不需要策划先暂停。暂停会停止排队和当前执行；重启后由用户明确继续任务。
人数模式改变会取消待答问题并重新打开需求梳理；重新判断哪些问题仍相关，不照抄旧假设。
答题不等于批准需求文档；需求、计划和最终交付仍各有原有的版本确认。

## 登记真实版本与证据

完成保存并以正确存档/关卡重新启动或重载后，使用 production version：
~~~json
{"expectedRevision":12,"versionId":"game-v1","instanceId":"实际受管实例ID"}
~~~
来源是该实例的启动存档及 Player。切换存档/关卡后先启动匹配的新实例。
桌面计算保存目录和实际 Player 的指纹，不接受模型编造的 hash 或任意文件路径。
保存内容、程序集或资源改变后，关闭轮次和交付检查会拒绝旧版本，须登记新 ID 并复验。

证据 kind=image/video/json/text；captureType=full-cycle/inspection/action/frames/state/log。
path 相对任务工作区，可直接引用 CLI 生成的 artifacts/runtime 文件；后端计算文件摘要。
同一证据 ID 不可替换，修改或重新取证使用新 ID；reviewed 仅在真正查看后设 true。
登记玩法版本之前的材料标为 preflight，只用于需求/能力预检，不能充当正式版本运行证据。
自然过程、构造夹具、脚本操作、人工操作以及审阅范围写入 description。
R0/R8/R12 各需完整循环视频、关键状态图片和状态断言；每轮实际巡检使用本轮新证据。
新需求周期不能使用旧周期证据冒充本次验证。

## 轮次和交接

按返回 policy 的稳定维度/题目 ID 写记录，题库不要另抄一份。
checks 每项 dimensionId/status/observations/evidenceIds；
questions 每项 questionId/status/conclusion/counterexample/problem/change/recheck/evidenceIds。
每轮九个维度，后四轮各回答全部 16 题。不适用写原因，没执行不叫不适用。
必要缺陷、版本变化、证据失效或缺问答时本轮保持未完成；失败尝试不新增轮数。
后端按顺序关闭 R1—R12；R12 后代码变化需要重开最终复验，不能沿用旧交付确认。

需求、计划、交付、收尾的 Markdown 草稿可参考 assets/production 对应模板。
JSON 模板只作字段示例，不复制覆盖数据库导出。制作报告用 production report --json 生成。
三个 HTML 的固定入口为 reports/plan-report.html、review-report.html、closeout-report.html。
知识采用与公共发布分开；closeout 文档、知识分析及用户交付认可完成后才更新任务状态。
查看库用 skill list，仅在选中时用 skill read（provider/skillId）；不读取全部正文。
APP“升级流程版本”保留旧 Skill 和历史，要求任务终端停止；普通读任务不更换固定版本。

