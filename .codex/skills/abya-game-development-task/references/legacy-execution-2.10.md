# 执行额度与成果收尾（2.10）

累计耗时永久保留用于统计，不等于无进展重试次数。APP按真实保存的Lua/关卡/UI内容、当前证据支持的检查、原项验证与依赖解除分配有界续接额度。不要通过新编号、改写报告、反复捕获同一版本获取额度。每段执行争取在15分钟内保存、读回、登记并交还控制；工具空闲硬检查点额外保留3分钟收尾宽限。

已经验证的方案优先最小接线→保存→冷加载复验。暂时没有进展时可用一次15分钟收尾机会完成已有结果对账与保全，不能借此扩展范围。源码变化本身不代表成功；验证包和2.9交接门槛不变。对账未知写入后只执行未完成动作。普通问题自行修复，平台能力缺口单独记录现有公开能力及等效方案，不将其冒充源码缺陷，也不冻结无关工作。APP提供的有限额度不能保证所有外部条件都可解决。

验证围绕已批准的玩法、美术标准与可见影响。用户没有要求逐像素一致时，不自行增加逐像素完全相同的放行条件；记录原始差异并按设计标准判断，不能虚报原始数据或放松明确要求。已有有效证据按影响范围复用，避免重复完整局和全量架构查询。

# Lua 自主执行（流程 2.10）

conclude-implementation成功后，当前派发已结束；直接结束回复，不再调用report-work-unit。
工作项报告与整个阶段收尾是不同操作，不因已收尾后的重复报告被拒而重新派发工作。

计划批准后，以真实可玩闭环为首个结果：一次自然输入 → 正确权威状态变化 → 画面反馈 → 完整局 → 保存冷加载。先接通最小真实链路再扩展多人、Bot、表现；所有批准要求保留，早期检查点不表示完整验收。

bootstrap 按 `milestone=core` 登记上述链路及验证包工作，其他扩展标 `extension`。不要先生成大量依赖未来注入接口的抽象模块或以离线断言数量代替接入。

15 分钟是进度检查点，普通实现/部署/验证续接原项，不算故障。恢复及 repair-plan 消耗关联问题的实际预算，修订保留累计耗时，不新编号刷新额度。用户暂停与未知写入对账优先。

检查完成但行为失败时，登记真实失败结果并报告 `deferred`、`deferReasonCode=repairable`。APP 自动派发 repair-plan；读取失败证据，采用实际不同的修复路线，通过 revise-work-unit 修订原项后交还执行。报告修复规划完成前，原项必须已重新排队或有证据明确分类。

外部阻塞用 external-condition，未知写入用 unknown-result，并提供 resultPaths 和 nextCondition。budget-exhausted 由实际账本验证。不要把可修复 Lua/输入接线错误归为永久平台缺失。重试条件变化可用 record-work-signal 提交实际文件路径；同文件同哈希或改摘要不产生新条件。

每个目标/产出批次都有最终核对。没有包时先安排缺失的实际接入与验证；可继续的工作进入队列，不能只写总结结束。核心 lua、主输入、core-loop、save-reload 实测通过后调用 build-verification-package，生成存档、验证记录、原始证据、缺口及指纹绑定的 ZIP。

完整门槛满足用 complete-stage；conclude-implementation 只允许携带已证实的 Abya-PB 源码缺陷进入试玩。普通缺陷、未测项、疑似平台异常必须继续在 Lua 阶段自动修复/取证，核心包不是阶段放行证。没有包或普通问题未闭环时保留 Lua 技术恢复状态，不能转交玩家或伪造验收。

流程 2.9 的 reviewHandoffVersion=1：仅 repairTarget=abya-source、diagnosisStatus=confirmed，且 diagnosis 中有 reproduction/comparison/rootCause/当前 playerBuildHash，并关联当前候选/周期的实际 evidenceIds，才是可交接的源码缺陷。abya-build、超时、断连、预算耗尽和原因未知不能冒充源码根因。仍应先尝试等效方案；若相关自测确被该缺陷阻止，以 failed/blocked 和 blockedByIssueIds 明确关联，不改为 passed。没有关联的必要检查必须完成。无需重新执行已经有有效证据的测试。

APP 会为收尾遗漏自动派发 handoff-repair；读取 production current.reviewHandoffDeficits，沿用原目标修复及复验，保留预算。权限/登录等真实外部条件不能绕过；无法继续就保存技术恢复状态和证据，不当作已完成或要求玩家替代技术修复。试玩卡片由已确认源码缺陷生成，保留影响、替代方案与诊断 ZIP 导出。

production version 必须声明 purpose=candidate 或 fixture。fixture只写独立冻结测试快照到
record.workbench.execution.validationSnapshots，不改变currentVersion、交付候选或批准。
夹具证据register-evidence须显式version=夹具ID，不能登记成正式候选的完整通过证据。
如果旧流程已把测试版设为当前版本，通过 production activate-version 输入 expectedRevision、
sourceRevision、versionId 恢复同任务同周期同批准范围的既有冻结候选；sourceRevision来自当时
成功的版本登记回执。此操作重新核验实际文件哈希，不启动实例、不覆盖编辑源、不新增通过记录。
不要为了恢复候选重抓仍在保存/清理中的目录，也不要让固定题序夹具留作最终交付版本。

工作项 kind 区分 implementation、deployment、validation、recovery（默认 implementation）。
按功能与目标拆分，单项约5–15分钟；恢复项必须绑定稳定 issueIds，不重新编号逃避预算。
恢复工作同故障/构建/范围共享30分钟恢复时间，每路线15分钟、无完成最多两次，单工作项总计60分钟。
模型单轮15分钟后由 APP 在无在途工具的检查点中断并续接；在途写入先完成或对账，不盲目重发。
暂停不计入执行预算，进程退出后的未知区间不算模型工时。不得用新工作项包装同一恢复问题。
无可做项时后端会汇总成果、未完成项及问题，缺少可玩证据时明确结束本轮但不通过验收。
实现、部署和验证分别记录；编辑器不可用时仍完成代码与可行的离线验证。
验证复用 [运行测试工具](runtime-scenarios.md)，不要重复搭建 CLI、轮询和回执框架。

save-issue 的 repairTarget 只是处理归属。默认原因待确认；diagnosisStatus=confirmed 必须附
diagnosis.reproduction、comparison、rootCause、当前 playerBuildHash 以及已审阅 evidenceIds。
此检查只验证材料齐全，AI 仍须实际复现和对照，不能把尾部重复日志当作根因。

仅当当前 policy.implementationExecution.mode 为 autonomous 且阶段为 implementation 时启用。
旧任务按原 policy；通过 APP 明确升级后才启用。需求、美术、计划和交付决定仍由用户完成。

## 工作方式

计划批准后，先 `production current` 读取有界当前状态。按调度器指定的 activeUnit 执行，
内部路线/重试不是新的用户目标。bootstrap 工作项负责从已批准计划拆出有依赖的实现、
修复与验证项，登记后写一份任务内队列说明文件，用 report-work-unit 完成 bootstrap。
不要在 Lua 阶段发布技术问答或要求用户点击“继续”；后台会自动选择可运行工作项。

`production update` 新操作：

- save-work-unit：id、goalId（已有本阶段原子目标）、title、dependsOn、issueIds、routes。
  三个列表均显式提供。依赖须先登记，最多4条实际不同路线；2.8 用 revise-work-unit 修订失败/暂缓原项，保留定义历史，不追加重复目标。
- revise-work-unit：id、expectedDefinitionRevision、reason、routes、dependsOn、resultPaths。不得改运行中或成功项，不能产生依赖环或重置实际预算。
- classify-work-unit：id、expectedDefinitionRevision、reason、deferReasonCode、resultPaths、nextCondition。未知结果不靠分类改写消除。
- record-work-signal：unitId、reason、paths。APP 核验文件哈希，只重评相关依赖。
- report-work-unit：id、status、reason、resultPaths；暂缓另带 deferReasonCode，外部或未知结果另带 nextCondition。仅报告当前派发项；status 为 succeeded、
  failed、deferred、awaiting-condition。succeeded 必须引用实际任务内文件，服务记录摘要。
  工作项完成不代表游戏自测或交付通过。
- record-issue-handling：issueId、mode、reason、impact、affectedUnitIds。mode 为 diagnosing、
  repairing、workaround、deferred、awaiting-condition；暂缓只隔离相关工作项，保留原问题。
- conclude-implementation：summary。核对原成果、完整检查与实际包后使用。
  2.9 仅在剩余事项全部由当前证据确认的 PB 源码缺陷解释时进入试玩；否则保留 Lua 修复状态。
  execution-converged 不等于 passed，交付门槛仍检查全部必要项。

普通错误自行修复并自测。疑似 PB 问题先保存现象、操作链、干净对照；不要仅凭调试层存在
就归为源码缺陷。确认后查已有能力，尝试等效实现，再验证。无方案时保存问题和影响，
暂缓相关工作，继续其他可做项。不要修改 AbyaPB 源码、删除要求或伪造通过来消除问题。

实现方式可以替换，但改变玩法、多人范围、胜负条件或明显视觉目标须标为候选差异，
保留原需求，到统一试玩结果中集中决定。用户反馈的复验仍由用户完成；实现中的 AI
自测问题则自行修复/验证，不自动转换为人工复验待办。

同路线最多两轮无进展尝试；连续失败换不同路线，再失败则暂缓。不要仅换说法、改摘要
或重建目标来重置预算。相关构建/实际存档/问题解决条件变化后可重新评估。
用户暂停、强制停止、进程重启不会触发自动恢复。权限或登录缺失不能绕过；先保存并
完成独立工作，合并展示一次准确的外部条件，不持续向用户发送相同请求。

## 少读、准确写、先对账

首次接入核对 doctor/capabilities；同一运行批次复用已核对的能力和命令映射，实例/构建/
schema 变化或能力拒绝时再查。实例先按任务和用途列出，复用适合的托管测试实例；不接管
用户试玩窗口，不丢未保存内容。冷加载、多端拓扑验证仍使用相应隔离实例。

当前摘要不包含完整历史；按需 production get 或读取引用文件。不要每条写入前全量 get。
串行写入沿用成功响应的新 revision；遇到冲突、新用户输入、版本变化或未知结果先对账。

`production batch` 输入：expectedRevision、batchId（UUID）、items[{operation,data}]。
每项生成 actionId=batchId:index，按顺序 CAS 提交；首个拒绝后其余未执行，先前成功项保留。
支持登记证据/自测/还原记录/工作项/问题等，不能批准文档、关闭用户反馈或代替外部游戏写入。
重试同批同输入返回原收据；同 ID 不同输入冲突。不能用新批次 ID 掩盖未知的旧写入。
`production receipt` 输入 actionId，查询原登记结果。不要把 complete=false 当作整批失败重放。

运行工具使用稳定 `--request-id`。`runtime receipt` 输入 instanceId、operationId，查询其收据。
同 ID 相同输入返回原完成收据（不缓存原始响应正文）；pending/outcomeUnknown 不重放，先读实际游戏状态或原输出。
明确未执行后修正调用再发新的操作；不得把取消当回滚。收据不代替读回和运行验证。

## 验证与版本

`record-validation-run` 输入 id、version、purpose（candidate/fixture）、checks、artChecks；fixture 使用独立登记的快照版本，可记录通过/失败，不写入正式候选自测。一次真实运行的结果关联同一
runId，再由后端登记已有自测/美术矩阵。检查项仍要满足原证据和版本规则，不虚构检查。
可放入 production batch。重复 runId 不重复登记，不同输入必须使用新的真实运行记录。

新策略捕获版本时保存验证过的独立存档快照，记录 editableArchiveSource；正式试玩从
快照再复制运行副本，避免编辑器继续保存污染已登记物。完整 SHA-256 仍核对。
validationImpact 为保守的影子差异清单：未知 PBArc/PBConf 字段或 Player 变化按全量重测，
不凭文件名忽略 Main/Index。证据继承仍须明确来源和不受影响范围；旧录像不冒充新采集。

短暂反馈先安排采集，再做输入。可用已发现的状态能力等待目标状态后采集；没有事件能力
时用有界状态检查或预先录制，不能发明不存在的事件接口。实际打开图像核对后才登记。

完成前先走原 complete-stage 门槛；全部可做工作收敛但有缺口时集中记录，并调用
conclude-implementation。不得为了3–4小时目标删测试、吞问题或将 execution-converged 当 passed。

进入自主阶段前核对原生执行设置：只读或人工逐项审核可能仍要求命令确认。由用户在
现有执行设置选择适用模式；APP/Agent 不静默修改权限。选用 AI 自动审核后仍遵守原生
审核结果。工作项报告结果后结束当前回复，由 APP 自动派发下一项，不询问是否继续。

mode=workaround 必须提供 scope：equivalent 或 candidate。equivalent 还须提供当前版本
已审阅 evidenceIds；服务绑定问题指纹、需求/计划 hash、cycle/version，并在门槛处重新验文件。
经过验证的等效替代允许本任务继续完成，但原 PB 问题仍为未解决并展示；candidate 或缺证据
的替代不能豁免验收。问题、版本或批准范围变化即失效。不要直接在 save-issue 填写 executionHandling。

核心预览的主输入用 save-self-test id=playable-input 记录真实自然输入引起正确状态变化的证据；未单独登记时使用 input。失焦、异常输入等完整 input 检查仍单独保留，不能因核心预览可玩就把完整输入验收标为通过。完整交付继续要求全部原检查。

2.8.1：核心四项有效检查登记完成后 APP 自动生成第一份验证包，其他工作继续推进；最终核对仍调用 build-verification-package 更新最新检查和缺口。检查保存成功但自动打包失败时返回 packageWarning，不重放已登记检查。
试玩使用 production start-acceptance：APP 将冻结存档复制到工作区根目录的短路径 .abya-play-runs/<运行ID>/Archives，绑定任务 owner 标记并传入独立 data root。旧 Player 无需识别工作区外部路径，不把冻结快照作为编辑档。不要自行启动非托管进程或改源快照来绕过加载失败。

对账恢复使用 reconcile-work-unit：沿用 revise-work-unit 的字段，另提供 reconciliation.operationIds、outcome（applied/not-executed/partial）及 readbackPaths。必须先实际读取原请求与目标状态；APP保存原始成功读回文件的哈希及对账说明。只登记尚未执行的不同剩余路线，不能重放原写入；对账不关闭问题或产生验收通过。
