# ABYA CLI 工作流

使用环境变量 ABYA_DESKTOP_CLI 指定的可执行文件。参数均为 JSON 对象，
通过 --input-file 文件路径或 -（stdin）传递；--json 返回单个 JSON 文档。
先运行 doctor 和 capabilities。capabilities 返回准确的命令 ID、参数 schema 与风险说明。
不要根据名称猜参数。下面左侧为命令，右侧为目录中的命令 ID。

| CLI | 命令 ID |
| --- | --- |
| task list/create/update/status | development_task_list/create/update/set_status |
| conversation bind/report | development_conversation_bind/activity_report |
| instance list/get/launch/stop/window/wait/launch-report | game_instance_list/get/launch/stop/set_window_visibility/wait_for_state/get_launch_report |
| runtime status/wait/list/run/cancel | game_runtime_get_state / game_instance_wait_for_cli / game_runtime_list_tools / game_runtime_call_tool / game_runtime_cancel |
| archive list/targets/sources/transfer/get/cancel | game_archive_list / game_archive_transfer_target_list/source_list/start/get/cancel |
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
