# 人机开发：按需接入玩法

仅在本次需求涉及新增、修改、删除或验收人机时加载本指南。已有未涉及的 Bot 定义应保留。
本指南复用当前任务、模板、美术选择及实施授权，不重新发起相同的需求/确认流程。
Bot 是服务端执行 Lua 策略的正常玩法参与者；复用现有 IUser、收件箱和生命周期。

## 可行性与设计

通过 ABYA_DESKTOP_CLI 的 doctor、capabilities 及托管实例的 read_me_first 获取真实契约；
主流程在同一实例/版本取得的有效结果直接复用，变化或证据不足时再查。
用 runtime describe 或 runtime run 包装 capability_describe 查询精确参数；不恢复 MCP 入口。
确认 editor_get_bot_definitions、editor_set_bot_definitions、架构 snapshot/validate/lint、
editor_save_archive、Lua API 发现和 runtime_bot_diagnostics 在实际 Player 中可用。
确认 abya.bots.add/remove/receive/submit_action 和所需网络下行、角色创建及归属接口。
源码存在不等于已发布的 Player 支持。缺必要能力时记录具体缺口，继续独立工作，不伪造运行结果。

在任务规格中明确用途、席位、人数上限、补位/移除条件、信息可见性及难度目标。
先定义动作通道/payload、权威处理器、观察通道、首次报名握手和接受证据，再生成决策逻辑。
消息观察用 send_to_user 下行及 bots.receive 收取；先 hello 再等快照，换轮清空缓存。
公开对象观察写清对象与字段。不得读取其他脚本的私有 abya.data，也不能用难度扩大可见范围。
submit_action 成功仅代表投递；按玩法 revision、题号、合法位置、得分或结算验证动作被接受。

## 任务产物与脚本

按 [bot-artifacts.md](bot-artifacts.md) 创建 W/artifacts/bot-integration 下的计划、定义快照及报告。
plan.json 的 bots 与完整候选定义对应；同时纳入现有 gameplay-architecture.v1.json。
任务计划和报告不是存档字段；真正保存内容仍由编辑工具写入 AI/manifest.json、Definitions/、Scripts/。
每个策略是完整自包含 Runtime/Server ABYA-LUA 3.0；entry 为独立初始化函数，决策放在 OnThink。
先动态发现公开 API，再校验元数据、语法、执行侧和绑定目标。本地文件校验器不是 Lua 编译器。
OnThink 非阻塞、每次有限消费消息；处理无动作、目标消失、玩家退出、换轮和重开。
四档均定义实际行为差异；ThinkInterval/ReactionDelay 控制调度，Accuracy/Lookahead 由策略使用。
保留正常权威规则；Bot 不直接修改权威计分。Manual 玩法仍需用户加入时创建、绑定和同步角色。

## 分阶段校验与写入

完整 Bot 架构校验读取已加载定义，因此新定义使用下列有限暂存例外。
该例外同时适用于公共 Skill 与多人模板，不允许屏蔽确定性错误，也不允许先保存后补校验。

1. 确认精确存档/关卡和脏状态；保留原有修改，并在任务副本中验证。
2. 确认同一存档只有本任务负责写入。当前接口无版本锁；多个 Agent 不得同时编辑该存档。
3. 读取完整定义至 definitions.before.json；检查已存在的 Bot 清单及基础架构。
   待新增定义放在 plan.json，基础检查只描述当前实际存在内容，不能声称完整候选已经通过。
4. 按稳定 ID 生成 definitions.candidate.json，保留其他定义；记录 changedDefinitionIds 和 removedDefinitionIds。
   删除只能来自明确需求；空数组会清除全部定义，另须 allowRemoveAll=true。
5. 校验候选 Lua/API/四档参数及资源。LuaTextResourceID 存在时运行时优先读它，必须验证真实资源正文。
6. 写前重新读取至 definitions.current.json，执行本地 prewrite；内容变化则重新合并。
   架构 snapshotRevision 只描述拓扑，不是 Bot 内容版本。本地摘要比对也不是原子版本锁。
7. 通过 editor_set_bot_definitions 提交完整候选的 definitions 数组，保持可撤销且尚未保存。
   get 可能返回数字枚举，set 要求名称；规范化 0/1/2/3 为 Easy/Medium/Hard/Hell，其他内容不改写。
8. 立即读回并重新取得架构快照，将全部候选 bots 加入完整清单并校验；通过后再修改必要的玩法绑定。
9. 完成观察、动作处理、角色、加入/移除实现，运行完整架构 lint，修正所有 error 和未解释 warning。
10. 保存并重载，读回 definitions.reloaded.json，确认 AI/ 文件、脚本来源、参数和绑定一致，再运行验收。

仅改变删除范围的任务不再要求不存在的 Bot 运行；仍需完成残留清理及普通玩法回归。
未涉及人机的普通任务不必生成这些附加产物，更不能用 bots:[] 抹掉已有定义。

## 失败与恢复

- 超时、取消和 outcome_unknown 后先读回，确认未执行/已执行/仍不确定，不自动重复写入。
- 校验失败时仅撤销确认属于本次且仍位于正确操作位置的修改；不撤销用户后来的操作。
- 当前定义仍等于本次候选、无其他写入时才可恢复 before 集合，恢复后必须读回比对。
- 并发变化或操作归属不明时保留 before/current/candidate 证据并停止写入。
- 不通过重载、恢复整个存档或保存无关脏内容来强行消除失败。

## 真实验收与交付

托管实例先验证加入、观察、动作接受、玩法推进、结算、重开、退出清理，再扩展策略。
联网玩法使用 Host 与独立 ClientOnly 的相同对局，核对 Bot 身份、归属对象、界面与结算。
执行非法动作拒绝、16 个 Bot 上限/加入失败；Manual 模式检查角色权威与远端代理。
四档使用同输入或可复现随机条件比较，至少一档覆盖完整实机对局，报告其他档位未覆盖部分。
游戏规则不同，接受断言必须按玩法生成；不能用固定心跳、提交数或 Running 状态代替。
保存原始 CLI 数据与日志，在报告中用 JSON Pointer 明确断言位置，不能另编一个成功 JSON 作证据。
验收前对实际存档全目录和 Player 构建（exe、Data 与运行库）计算摘要，并绑定 run.json 的批次、实例、provider。
保存内容或 Player 变化后原验收失效；只更新报告中的哈希不能恢复其有效性，必须重新运行。
执行本地 acceptance 检查；exit 0 的 passed 才表示材料与断言通过，exit 3 表示未完成，exit 2 表示无效。
检查器检查材料一致性及明确断言，不能证明捕获数据的真实性或任意策略正确性；保留审阅原始证据。
分别报告静态检查、受控策略、Host、ClientOnly、未覆盖范围；未测试连玩不能宣称连玩通过。
实际任务完成仍需用户接受结果。桌面计划步完成或校验器通过都不自动更改任务数据库状态。
