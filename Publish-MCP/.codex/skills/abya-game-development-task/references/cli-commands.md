# ABYA MCP 工具映射

本分支保留 Codex 会话驱动，ABYA 业务操作只通过已配置的 abya MCP 服务。
不执行 abya-desktop、tools/abya、旧 HTTP 命令端点或调用这些入口的脚本。
工具 schema 以当前 MCP 目录为准；参数是 JSON 对象，不是 Shell 命令。

| 工作 | MCP 工具 |
|---|---|
| 当前工作 | development_production_current |
| 完整制作记录 | development_production_get |
| 登记文档、问题、证据、自测、工作项 | development_production_update |
| 分批登记与原收据查询 | development_production_batch / development_production_receipt |
| 保存实际版本 | development_production_version / development_production_activate_version |
| 运行基线 | development_production_runtime_baseline / game_runtime_qualify_player |
| 实例管理 | game_instance_list / game_instance_launch / game_instance_stop |
| 实例就绪与启动报告 | game_instance_wait_for_mcp / game_instance_get_launch_report |
| 当前 PB 工具目录 | game_runtime_list_tools |
| 调用 PB 原生 MCP 工具 | game_runtime_call_tool |
| 查询原操作与请求取消 | game_runtime_operation_receipt / game_runtime_cancel |
| 查看和登记计划活动 | development_conversation_activity_report |
| 保存报告 | development_production_report |
| 开始正式试玩 | development_production_start_acceptance |

APP 自动绑定当前任务、会话及提供方；不要将其他任务的 ID 放入参数。
新实例先用 game_runtime_call_tool 调用 read_me_first；按需调用 capability_search、
capability_describe 发现真实工具和参数，再调用具体能力。禁止按名称猜字段。
MCP 网关会再次核对实例归属及阶段权限，PB 内部按开发能力白名单授权。

MCP 连通不等于游戏已初始化；检查实际存档、关卡及必要能力的就绪状态。
工具 isError 与内部 valid/ready/assertions 分别核对；截图返回后必须实际查看。
写入结果未知时先查原操作收据和目标状态，不自动重放或制造第二个任务。
审批、回答策划问题、关闭人工反馈和接受作品仍由 APP 用户入口完成。

原说明中 production update 等简写表示对应业务操作，请使用上表的 MCP 工具，
operation/data 等业务参数保持不变。文件读写、离线数据检查可使用 Codex 自带能力，
但不能用它们绕过 MCP 去启动游戏、写入运行实例或改制作状态数据库。

对运行写入可提供稳定的 requestId（UUID）；返回的 abyaOperationId 用于查询原操作。未知结果先查询收据与实际状态，不能换新 ID 直接重放。
