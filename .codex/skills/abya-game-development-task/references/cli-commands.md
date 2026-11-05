# ABYA CLI 工作流

使用环境变量 ABYA_DESKTOP_CLI 指定的可执行文件。参数均为 JSON 对象，
通过 --input-file 文件路径或 -（stdin）传递；--json 返回单个 JSON 文档。
先运行 doctor 和 capabilities。capabilities 返回准确的命令 ID、参数 schema 与风险说明。
不要根据名称猜参数。下面左侧为命令，右侧为目录中的命令 ID。

| CLI | 命令 ID |
| --- | --- |
| task list/create/update/status | development_task_list/create/update/set_status |
| production get/update/version/report | development_production_get/update/version/report；确认由 APP 用户操作，不能通过 CLI 批准 |
| skill list/read | development_skill_list/read |
| recording tools/start/get/stop | game_recording_tools/start/get/stop；实时视频，无音频 |
| conversation bind/report | development_conversation_bind/activity_report |
| instance list/get/launch/stop/window/wait/launch-report | game_instance_list/get/launch/stop/set_window_visibility/wait_for_state/get_launch_report |
| runtime status/wait/list/run/cancel | game_runtime_get_state / game_instance_wait_for_cli / game_runtime_list_tools / game_runtime_call_tool / game_runtime_cancel |
| archive list/targets/sources/transfer/get/cancel | game_archive_list / game_archive_transfer_target_list/source_list/start/get/cancel |
| archive create/creation-status | game_archive_create / game_archive_creation_status；通用任务空白初始化及回执查询 |
| log start/stop/sources/sessions/query | game_log_collection_start/stop / game_log_source_list/session_list/query |

runtime run 的输入为 {"instanceId":"桌面实例 ID","toolName":"能力 ID","arguments":{}}。
调用 read_me_first 后，动态发现能力；搜索、描述分别调用 capability_search 和 capability_describe。
桌面服务负责映射真实 PID、稳定会话、请求 ID、截图目录及运行时 CLI。
实例 ID 不等同于 PID；禁止直接控制其他任务的实例。

示例（参数文件由 AI 写入当前任务工作目录）：

~~~powershell
& $env:ABYA_DESKTOP_CLI runtime run --input-file ./runtime-request.json --json
~~~

所有截图返回绝对文件路径，必须用当前 provider 的图片读取能力打开；生成路径不代表视觉验收完成。
窗口保持后台渲染。通过 instance window 显示窗口以供人工检查，再恢复后台。
需要取消时调用 runtime cancel，传入 instanceId 和 operationId；取消请求不保证回滚。
保存日志、状态断言、Lua/APC 追踪及图片，完成原有单机或独立 Host + Client 验收。

流程2.0验收入口：
- production start-acceptance → development_production_start_acceptance，启动任务当前版本的可试玩实例。
- production acceptance-status → development_production_acceptance_status，读取进度、版本与实例。
两者绑定当前任务；启动前验证保存版本，不会使用全局旧运行包替代任务运行包。
策划关闭反馈和接受作品没有Agent CLI入口。
## 2026-11-05 方案基础能力预检与自动清理

capabilityPreflightVersion=1 的任务在方案阶段运行固定创建/加载/保存/冷加载检查，仅使用 APP 当前包。production preflight / development_production_preflight（reuseExisting、retry 默认 false）从认证上下文取得 taskId，无任意 Player 或测试目录参数。APP 进入方案自动执行，CLI 提交方案遗漏预检时先执行并要求刷新记录。
独立 .abya-preflight/<taskId>/<runId> 具有 owner 标记；不放宽普通 authoring 权限。首次失败立即登记 preflight:basic；超时/上下文不可用为普通待定位问题，真实能力目录缺项/必需契约缺失才标 PB 包兼容问题。此问题只能由实际预检回执更新。
reuseExisting 仅复制本任务唯一编辑存档，实测保存冷加载后记为已验证替代，保留原问题。方案引用当前报告并说明替代路径及限制；未测、失败、证据变化、包变化或清理未完均不能进入实现。旧任务未显式启动或升级时不追溯撤销既有批准。
回执保存在 artifacts/capability-preflight，临时数据自动清理：停止本次实例、核对 OS PID 已退出、验证 UUID/路径边界/owner/无链接后删除。失败/未知也先停止并核对；重启及每 30 秒继续清理 pending，不重放创建、不停止外来 PID、不删正式存档。只保留诊断回执和日志。无 owner 的空目录可回收，非空且归属不明则保留并报告。
