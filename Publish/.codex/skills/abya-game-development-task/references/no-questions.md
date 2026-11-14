# 不提问模式

每次接续和阶段转换先读 production get 的 record.noQuestions。enabled=true 才使用此模式；runId 表示本轮授权。关闭或本轮结束后停止自动操作。用户明确暂停或停止时停止。

模式授权来自 APP 项目按钮。根据已有文档、图片和任务说明自行补足合理假设，写入方案；自行审阅需求、美术和执行计划，连续完成实现、实机检查、修复及打包。使用当前 APP 指定的 Player，按实际接口校验执行。平台强制权限请求在原生界面处理，不能伪造同意或通过回执。

## 自主审阅接口

操作使用当前绑定任务的 `production respond --input-file FILE --json`。参数统一有 expectedRevision（最新 revision）、operation、userMessage（自动操作填空字符串）和 data。自动操作的 data 都带最新 runId。每次写入后回读新 revision。后端在本次写入中再次检查模式、runId、文档版本和实际门槛。

- `auto-review` 的 data 为 runId、kind（requirements / resources / plan）、documentHash、review。review 写明完整性核对、已满足的约束和采用的假设。通过记录注明 AI 自审；文档仍须完整送审并满足原门槛。
- `auto-answer` 的 data 为 runId、action（set-player-mode / submit-answers）、answer（原接口对应的数据对象）。用于处理开启前已经存在的问题组或敲定玩家模式，决策写入记录。
- `auto-block` 的 data 为 runId、reason。只有真实外部权限、资料或环境依赖无法自行解决时使用。写明已查的证据、可继续的工作、恢复条件。参数错误、旧问题卡和未试过的调用按 lua-recovery-cases.md 自查。
- `auto-finish` 的 data 为 runId。先取得本轮新版本对应的试玩 ZIP，实测输入、反馈、结算、重开及冷加载，恢复编辑档并关闭自测实例，再调用。后端核对新版本、内容变化、当前实测证据和包哈希，并复用正式 Lua 完成交接的美术、问题及自测门槛；缺项继续自行补齐，通过后停止本轮并展示可继续反馈的试玩入口。

基础预检使用 `production preflight` 原入口。开启模式已涵盖本任务的隔离预检授权；复用同范围有效回执。普通技术问题自行修复，保留未知写入并核对真实结果。

如果已有试玩包且没有新的待办，说明当前没有待处理修改，用 auto-block 等待新需求，不为凑版本号重复打包。当前版本只用作开启时基线。交付签收和真人主观评价保持为实际用户决定。
