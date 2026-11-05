# Bot 任务产物 v1

以下是任务工作区文件，不是新 Runtime API；不把这些额外字段发给游戏工具。
目录 R = W/artifacts/bot-integration。复制 assets/bot-integration 的模板后填入实际值；
空目标/摘要占位不能通过检查。模板不会自动启用人机，也不会自动删除历史定义。

## 文件约定

| 文件 | 来源与用途 |
|---|---|
| plan.json | 按 plan.schema.json 记录本次修改范围和完整候选 bots；enabled 表示本次涉及人机开发 |
| definitions.before.json | 首次 get 的 archiveGuid 与完整 definitions，保留恢复基线 |
| definitions.current.json | 写入前最后一次 get 的同样两字段，作为并发变化检查材料；验收期间保留该快照 |
| definitions.candidate.json | 同样两字段；稳定 ID 合并后的完整集合；只将 definitions 发给 set |
| definitions.reloaded.json | 保存后重载再 get 的同样两字段 |
| validation.json | 按 validation.schema.json 记录静态结果、运行用例、引用和未覆盖项 |
| run.json | 开始实机前建立的独立批次记录，形状如下；更换内容/Player 后创建新批次 |
| evidence/ | 本批次的实际捕获文件，必须位于 R 内；不得指向其他任务或通过链接越界 |

run.json 使用以下字段（示意值需替换）：
```json
{
  "runId": "本次生成的 UUID",
  "startedAt": "2026-09-28T08:00:00.000Z",
  "target": {"archiveId":"实际 GUID","levelId":"实际 GUID","buildId":"player-hash 输出的构建摘要"},
  "provider": "codex",
  "archiveHash": "实际保存存档目录摘要",
  "instances": [{"id":"桌面 Host 实例 ID","role":"host"},{"id":"独立客户端实例 ID","role":"clientOnly"}]
}
```

单机实例 role=offline。不要拿进程 PID 代替桌面实例 ID；两个端必须是两个实际实例。
buildId 包含 exe、对应 _Data 全目录以及存在的 UnityPlayer.dll、GameAssembly.dll、MonoBleedingEdge。
使用 player-hash 生成，不能只用通常不随游戏代码变化的 exe 哈希；另记录构建版本/manifest。
definitionsHash 由 prewrite 输出；规范化键顺序、定义集合顺序及数字/名称难度枚举，不修改 Lua 正文。
archiveHash 对实际存档的全部文件路径及内容摘要计算，包括资源与 AI/；不包含任务证据目录。

## 证据与断言

运行证据包装实际读取结果为 JSON，至少包含：
- runId、target、provider、definitionsHash、archiveHash：与本批次一致。
- instanceId、role、capturedAt（UTC ISO 时间）：实际采集实例及时间，晚于 startedAt。
- definitionId、botId、difficulty：策略定义、真实用户身份与 Easy/Medium/Hard/Hell。
- kind：gameplay（玩法状态）、diagnostics（Bot 诊断）或 strategy（受控策略样本）。
- data：实际 CLI 返回/解析的数据；保留原始返回文件用于追溯，不人工填造观察值。

静态捕获至少包含 target、definitionsHash、archiveHash、data。
在同一已保存内容上重做 Lua/API、架构 validate/lint，分别存文件，successPointer 指向实际成功布尔值。
若工具返回文本内 JSON，解析后保留在 data，原始返回另存证据目录；不可把任意成功操作充当 Lua/API 检查。

validation.json.evidence 每项为 {id,path,sha256}；path 是 R 内相对路径，sha256 是文件原始字节摘要。
staticChecks 每项为 {kind,path,sha256,successPointer}；kind 为 lua-api、architecture-validate 或 architecture-lint。
cases 每项为 {id,kind,definitionId,status,detail,assertions}。每种 Bot、每类检查合并成一个用例。
assertions 读取 evidenceId 对应文件的 /data/... JSON Pointer；equals/contains/greaterThan/lessOrEqual 使用 expected。
changed/increased 使用 beforeEvidenceId、beforePointer，要求前后为同实例、Bot、难度且时间有序。
accepted-action 至少有一条基于 gameplay 前后状态变化的断言；diagnostics.submitted 不能代替。
client-sync 使用 matches 比较 Host 与 ClientOnly 的同一 Bot/难度字段，参数为 beforeEvidenceId、beforePointer；
两端须分别来自 gameplay 捕获，除得分外还应比较比赛/轮次身份，避免不同轮次偶然同分。
difficulty-comparison 必须引用四档实际受控样本；说明共同输入/种子与行为预期，不要求每局得分排序。
用例种类与必测集以 schema 和检查器为准：join/observation/accepted-action/settlement/restart/cleanup/
invalid-action/capacity-limit/difficulty-comparison；Manual 再加 manual-ownership，联网再加 client-sync。
所有目标 Bot 都要覆盖；至少同一档的真实 host/offline 证据覆盖完整对局，不能全用模拟数据。
uncovered 记录未承诺的额外范围；不能用它豁免必测用例。

## 本地执行

Node 20+；使用桌面随包 runtime/node.exe 或已安装 Node。S 为当前 provider 的本 Skill scripts 目录。
这些只读脚本操作任务文件；游戏发现、写入、启动和停止仍必须通过 ABYA_DESKTOP_CLI。
```powershell
node "$S/validate-bot-artifacts.mjs" --root "$R" --mode prewrite
node "$S/validate-bot-artifacts.mjs" --mode archive-hash --archive "$ArchiveDirectory"
node "$S/validate-bot-artifacts.mjs" --mode player-hash --player "$PlayerExe"
node "$S/validate-bot-artifacts.mjs" --root "$R" --mode acceptance --archive "$ArchiveDirectory" --player "$PlayerExe"
```
prewrite 只检查声明范围、集合保留、写前摘要和基本参数；candidate-checked 不等于 Lua、资源或架构已通过。
输出 stdout 为一个 JSON；exit 2 表示无效材料/断言失败，exit 3 表示状态仍为未完成，exit 0 表示本模式检查成功。
enabled=false 且定义完全保持原样时返回 not-applicable、accepted=false，不意味着整个游戏已验收。
blocked/failed/runtime-unverified 报告返回未完成；缺失运行材料时应使用这些状态，不伪造通过材料。
static-passed 仍不是运行通过；只完成部分静态检查时用 runtime-unverified 并注明范围。
删除全部人机后执行主流程的普通玩法回归，不把空 bots 当作人机运行验收通过。
