# 自主执行契约（2.11）

先看 production current 的当前策略。未启用 inPlaceRecoveryVersion=1 的固定旧任务按 [旧执行协议](legacy-execution-2.10.md)，不混用恢复规则。

## 交付结果

按已批准规格实现能运行、能验证的游戏。先完成真实输入、权威状态变化、画面反馈、完整局及保存冷加载，形成可玩检查点，再补齐其余要求。
普通技术步骤连续执行，不索要用户“继续”。需求、美术、计划、交付仍使用 APP 的人工确认入口。
可玩检查点可独立预览；预览不是正式试玩通过。必要缺陷不得隐藏，正式试玩仅可携带当前证据确证的 PB 源码缺陷。

## 失败判断

默认使用任务已选的稳定 Player。先核对自己的实现、调用参数、实例、实际存档版本和验证方法；日志报错或工具超时不等于 PB 缺陷。

| 实际观察 | 下一步 |
|---|---|
| 未测 | 安排与要求相符的最短有效验证 |
| 验证工具不可用 | 查已发现的等效工具或真实输入/读回路径 |
| 验证断言失败 | 检查断言、前置条件、版本与观测方式，再判断功能是否失败 |
| 实际功能失败 | 修复实现或采用等效方案，验证相同必要要求 |

同一假设没有新证据时，不重复原操作或重写诊断。可以换实现和验证方法，不可降低要求；按键通过不能代替鼠标按钮验收。
当前版本的证据若排除了原判断，可以用 diagnosisStatus=excluded、diagnosis.exclusionReason 及当前证据记录排除结论，再按实际情况关闭误报；不必为了证明“修复过”而改代码，也不能据此倒推历史版本从未有问题。
普通失败在当前工作项内解决。需要交还调度器时，用 report-work-unit 报真实结果及 observation；APP 在同一 ID 内有界复核/换路径，不新增 repair-plan/handoff-repair。
仅当范围、依赖或原计划确实改变时修订工作项；保留 ID、累计时间、原结果和历史。

## 记录接口

report-work-unit：id、status、reason、resultPaths。status 为 succeeded/failed/deferred/awaiting-condition。
暂缓另带 deferReasonCode；外部条件或未知结果另带 nextCondition 和原始回执路径。
observation：kind、origin、expected、observed。
kind：not-tested / verification-unavailable / verification-failed / functional-failure / none。
origin：tool-contract / runtime-state / data-version / verification-method / implementation / platform-suspected / permission / service / unknown。
origin 是待核对归因，不是确诊，也不自行创建权限等待。真实权限请求仍由原生权限控制处理；dependency 只对应实际尚未完成的工作依赖。平台确诊走 save-issue 的构建、复现、对照、根因和当前证据约束；diagnosis.excludedCauses 分别说明排除 invocation/runtimeState/dataVersion/verificationMethod 的依据。
save-self-test 的 outcome 分别记录 not-tested / verification-unavailable / verification-failed / functional-failure / passed / not-applicable，与 status 对应；默认失败只表示验证未满足预期，声明实际功能失败须有当前证据。

额外检查用 register-acceptance-check 关联 id、expected、basis、required。
必要要求使用 approved-requirement，并提供 documentKind（requirements/plan）和当前批准文档原文 quote；严重实际缺陷使用 product-integrity 与 issueId。
额外优化使用 optional、required=false，不自动扩大交付门槛；新发现的严重功能缺陷仍处理。
不适用须有当前证据与 applicabilityReason；核心运行、输入和保存检查不能标为不适用。
需要增量复验时，检查声明 dependencies（冻结存档清单中的实际相对路径）；register-evidence 携带 checkId，后端记录依赖与构建指纹。reuse-evidence 只允许批准范围、构建和依赖均未改变，未知内容影响仍要求复验；不能仅凭“未受影响”的文字继承通过。
依赖默认按完整文件校验；确需区分布局时可以声明 `{path,projection:"ui-bindings"}`，仅限逻辑类基础检查且只识别 PBUI v1 的 Layout 之外全部内容。`content` 投影只排除已解析存档的 LastEditTime，不忽略绑定、规则、索引哈希或客户端版本。布局变化的输入、视觉及保存重载仍必须实测；范围限定证据不能用于其他检查。

## 时间与恢复

时间片只是保存进度并续接，不是功能失败。累计耗时保留，整个目标共享额度；报告数量、时间戳和新编号不会增加额度。
当前 Lua 原子目标默认共享 180 分钟主动执行额度，额外保留至多 15 分钟收尾；不是每个子项各拿一份无限续期，也不是完整游戏必定在此时间内通过的承诺。
显式升级时单独记录新策略额度的起算点，不清除历史累计时间；之后普通重试、版本登记和恢复都不重置总额度。
普通失败最多三轮原项恢复：核对假设、改变方法、保全收尾。额度或路线耗尽后，APP 保留成果并建立内部技术处理项，其他独立工作继续。
技术队列不是给策划的“继续”待办，也不是通过验收。真实权限或外部条件不能绕过。
用户暂停、强制停止和进程重启不隐式恢复。未知写入先查原收据和真实目标状态；取消不是回滚，不盲目重放。

## 工具与证据

使用任务托管 CLI/实例，按实际 schema 调用。只读可有界重试，写入保留稳定 request ID。
同对象、版本、环境和检查目的的有效结果复用。先读已有回执，只补未覆盖部分，不重复全量查询或整局测试。
使用 [运行测试执行器](runtime-scenarios.md) 管理输入、等待、证据和失败分类，不为每个游戏重新搭建 CLI/轮询框架。
Player 基线只证明列明且实际验证的能力，不证明任意玩法无缺陷，也不免除本游戏验证。
candidate 与 fixture 分开，证据绑定实际构建、存档、运行和批准范围。接口细节按需读取 [任务资料](production-records.md)。

## 收尾

核心检查有效后保留/更新真实验证包。满足全部门槛再 complete-stage，其他情况如实 conclude-implementation，不为结束回复把未完成写成成功。
收尾成功后不重复 report-work-unit。最终交付由用户验收；外部协调对话手工介入不得计为原生工作流独立完成。
