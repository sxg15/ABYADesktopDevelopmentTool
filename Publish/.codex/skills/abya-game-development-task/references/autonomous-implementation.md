# Lua 自主执行（流程 2.4）

仅当当前 policy.implementationExecution.mode 为 autonomous 且阶段为 implementation 时启用。
旧任务按原 policy；通过 APP 明确升级后才启用。需求、美术、计划和交付决定仍由用户完成。

## 工作方式

计划批准后，先 `production current` 读取有界当前状态。按调度器指定的 activeUnit 执行，
内部路线/重试不是新的用户目标。bootstrap 工作项负责从已批准计划拆出有依赖的实现、
修复与验证项，登记后写一份任务内队列说明文件，用 report-work-unit 完成 bootstrap。
不要在 Lua 阶段发布技术问答或要求用户点击“继续”；后台会自动选择可运行工作项。

`production update` 新操作：

- save-work-unit：id、goalId（已有本阶段原子目标）、title、dependsOn、issueIds、routes。
  三个列表均显式提供。依赖须先登记，最多4条实际不同路线；定义登记后不覆盖，变更另记新项。
- report-work-unit：id、status、reason、resultPaths。仅报告当前派发项；status 为 succeeded、
  failed、deferred、awaiting-condition。succeeded 必须引用实际任务内文件，服务记录摘要。
  工作项完成不代表游戏自测或交付通过。
- record-issue-handling：issueId、mode、reason、impact、affectedUnitIds。mode 为 diagnosing、
  repairing、workaround、deferred、awaiting-condition；暂缓只隔离相关工作项，保留原问题。
- conclude-implementation：summary。可做工作已收敛但不满足原完成门槛时使用。
  仅在最低可试玩检查已有实际证据时进入试玩阶段，implementation 标 execution-converged，
  不能当作 passed，交付门槛仍检查全部必要项；无可试玩证据则保留当前阶段及缺口。

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

`record-validation-run` 输入 id、version、checks、artChecks；一次真实运行的结果关联同一
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
