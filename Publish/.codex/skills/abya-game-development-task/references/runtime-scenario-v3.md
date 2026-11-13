# 运行测试 v3

先在计划阶段创建 artifacts/runtime-tests/plan.json，格式见 [模板](../assets/runtime-tests/plan.template.json)。
requirementsHash 必须来自当前已批准需求；用例引用原文并说明角色、前置状态、操作、预期和证据。
模板中的占位内容必须替换，不可预填 passed/status。实现前确定预期，发现有效新要求后修订用例。

## 执行

沿用 runtime-scenario.mjs；schema 为 abya.runtime-scenario/v3，roles 将 Host/Client 等角色映射为当前任务实例 ID。
version/buildHash/archiveHash 绑定当前登记候选。正式调用前执行器读取 production get 和各端 runtime describe；
缺能力在任何场景写操作之前失败，转为同事负责的 CLI 缺口，不假造能力。
场景仍须实际断言存档/关卡身份；登记候选核验不代替进程中加载内容核验。
所有调用走 CLI，无 Computer Use。

steps 使用 id、role、action、arguments、assertions；具体参数先查当前能力 schema。
公共动作表见 [scenario-actions.mjs](../scripts/scenario-actions.mjs)。
key.tap 组合 key.down、等待 holdFrames（默认2）、key.up；pointer.drag 组合实际位置序列、按下、逐帧移动和松开。
drag 的 points 提供2至128个 x/y 位置，arguments 提供当前 schema 支持的 button/origin/scopeId。
event-click 只证明事件绑定，不证明遮挡与真实指针命中；输入路径与观察路径必须分别验收。
动作后读取权威业务状态与画面；accepted=true 不等于游戏行为正确。

assertions.path 为相对 CLI data 的字段数组。每项恰好一个操作：
equals/notEquals/greaterThan/atLeast/lessThan/atMost/exists。
缺失字段只允许 exists:false 通过。前序引用写为 {"$ref":["before","score"]}，只能引用之前完成步骤的实际返回。
同一目录再次运行仅复用与输入和结果指纹一致的已完成步骤；失败或未知步骤先核对原回执。
完整响应保存 response-<stepId>.json，主回执保留角色、动作、实例、请求、结果指纹和断言。

## 通用组合

| 用例 | 组合 | 观察 |
|---|---|---|
| 单人闭环 | 冷启动→身份读回→开始输入→合法行动→状态轮询→结算→重开 | 阶段、计分、再次可操作 |
| UI与拖拽 | 树/节点定位→真实指针移动/按下/拖动/松开→状态读回 | 命中、取消、越界与禁用行为 |
| 双端同步 | Host/Client身份读回→指定角色输入→两端状态/截图 | 权威结果、身份可见性、重复提交一次计分 |
| 保存重载 | 保存→记录实际版本→另行独立冷加载→身份与状态断言 | 同一成果、无夹具残留、继续完整循环 |
| 输入恢复 | 按住→状态切换/失焦→松开→清键→重新输入 | 无粘键、无越权输入 |
| 特效生命周期 | 触发→中途取消→再次触发→销毁→状态读回 | 对象、资源、输入恢复 |

实例启动、冷加载、断线/重连和系统焦点使用当前已发现的桌面能力进行外层编排；缺项记录未覆盖。
测试作用域中的按住动作必须在成功和失败清理路径释放；未知写入先对账，不能把清理发送当作成功证明。
每个用例结束登记实际证据及 record-validation-run；脚本成功只证明列明断言，人工试玩结论独立。
