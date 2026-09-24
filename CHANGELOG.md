# Changelog

- Run storage repair on a background thread so large legacy databases do not block the desktop UI.
- Add a Settings one-click storage repair command that stops managed connections, bounds persisted logs, truncates oversized instance logs, compacts SQLite, and verifies integrity.

All notable changes to this project are documented in this file.

## [Unreleased]

### Fixed

- 修复 Git 忽略规则误排除 `src/features/logs/` 与 `src-tauri/src/modules/logs/`，补齐日志页面和日志后端源码。
- 修复运行日志真实数据仍可再次占满系统盘：每个结构化日志会话只保留最近 100,000 个 sequence 位置，消息、标识字段和原始 JSON 均设置持久化上限；托管实例 stdout/stderr 日志超过 64 MiB 时由进程监控器在线截断并写入标记，避免高频 Unity 输出同时无界增长 SQLite 与 `InstanceLogs`。
- 修复任务工作流事件日志和 SQLite 存储异常膨胀：`workflow-events.jsonl` 改为不再嵌入完整快照的紧凑 revision 记录并限制为 8 MiB，下一次保存会自动压缩超限旧日志；数据库启动时截断已 checkpoint 的 WAL、限制保留 journal、对严重空洞旧库执行一次安全 vacuum 并启用增量回收；运行日志批次改为单事务提交，避免逐事件放大 WAL 写入。
- Codex/Grok 任务终端不再在打开几十到上百 MB 的 `transcript.log` 时把完整文件同步灌入 WebView：后端只按 64 KiB 分片回放最近 256 KiB，实时会话重连时先完成快照回放再切换订阅者；前端输出历史限制为最近 2 MiB，避免高密度 ANSI 重绘导致历史显示中途卡住并连带阻塞键盘输入提交。
- 打开任务 Codex/Grok 终端时，等到容器可见且已有可测量宽高后再启动 PTY，并把行列限制在 20–500 / 5–200，避免首帧 0 尺寸触发 “Terminal size is outside the supported range.”
- 选中任务后点击右上角删除不再卡死整个窗口：删除命令改到 UI 线程外执行，先卸载该任务已打开的终端，再断开 Codex/Grok 的 Channel 与 PTY，最后才终止进程树；Windows `taskkill` 最多等待两秒后回退到 PTY killer。

### Added

- 新增独立美术风格模板 `abya-art-template-*` 的导入、校验与任务工作区同步，完整导入 `comic-arcade-ui` 的参考图和素材包。可行性评估后提供使用/不使用美术模板选项，选择使用时默认漫画街机风格；多人任务沿用该默认项并保留 DingTalk 字体要求。导入 Skill 支持 `--kind art`，普通任务模板行为保持兼容。

- 创建任务 Skill 新增模板选择首步：列出模板名称和简介，并支持“不使用模板”沿用通用流程；已有选择在后续对话中复用，模板执行保留实例、授权、架构和验收公共约束。
- 新增 `abya-import-task-template` Skill 和双提供方导入脚本，以 `abya-task-template-*` 与版本化标记识别模板；导入关卡策划的多人制作流程及全部参考文档。创建/读取任务时同步合法模板，保护未标记的用户 Skill；便携构建包含导入入口和全部模板资源。

- 受管 ABYA 开发任务 Skill 新增默认素材交付和 CustomUI 视觉验收规则：用户未指定素材来源时，由 Agent 生成玩法所需素材、导入编辑器并应用到场景对象与 CustomUI；文本必须与直接背景清晰区分，验收需检查目标分辨率下的父子边界、锚点、换行、重叠、裁切和屏幕越界。
- 受管 ABYA 开发任务 Skill 新增玩法架构清单门禁：实施前生成任务工作区
  `artifacts/gameplay-architecture.v1.json` 并通过 Runtime MCP 校验，实施后执行
  架构 lint；共享自由物体行为优先绑定 Prefab，对集中式逐帧实体管理、碰撞轮询
  和实例脚本复制给出结构诊断。Codex/Grok 便携内容同时包含版本化清单模板与
  架构参考。
- 托管游戏实例启动契约新增 `--abya-mcp-auto-approve=true`，强制放行 Runtime
  MCP HighImpact 授权且不写进游戏设置；启动参数文档与 `game-instances` Skill
  同步记录该参数。
- 桌面 MCP 配置模板新增 `Grok` 选项，目标路径为
  `~/.grok/config.toml`，并使用 Grok 1.0.13 支持的 `headers` Bearer 认证字段，避免误用 Codex 的 `http_headers` 导致认证被忽略。
- Grok 新增可实际发现的项目级
  `.grok/skills/abya-game-development-task` Skill 与启动参数参考；创建或读取任务时会与 Codex Skill 一样幂等部署到任务工作区，绑定 Desktop MCP 时显式使用 `provider: "grok"`，且保留两个提供方目录中的用户自建 Skill。
- 开发任务终端新增用户显式 `Codex / Grok` 提供方选择；每个任务记住最后选择，两个提供方拥有独立多对话、重命名、历史恢复、PTY 与完整输出滚动视图，切换提供方不会停止已运行会话。
- 新增 Grok CLI 本地发现、任务工作区固定启动、UUID `--session-id` 创建和 `--resume` 历史恢复；通过 Grok 原生 ACP `updates.jsonl` 映射计划、工具调用和回合完成状态到任务步骤条，并在事件不可用时显示兼容性警告。
- Desktop MCP 对话绑定新增可选 `provider: codex | grok`，省略时保持 Codex 兼容；Codex 与 Grok 均可将游戏实例、Runtime MCP、测试和其他桌面工具行为关联到当前计划步骤。
- 新增项目根目录 `.grok` 便携内容目录；构建时与 `.codex` 一起完整复制到 `Publish`，且不复制用户目录中的 Grok 认证、会话、日志或缓存。
- Codex 对话支持在任务终端列表中使用铅笔图标内联重命名，名称持久化到任务工作区且不会替换已有原生 Codex session。
- 任务标题和分栏之间新增始终可见的 Codex 执行步骤条：每个用户回合展示横向计划节点与箭头，悬停或键盘聚焦显示最近行为，点击节点打开完整时间线抽屉；切换到游戏实例分栏后仍保留。
- Codex `app-server` 原生可观测桥接使用随机 loopback WebSocket、临时 capability token 和远程 TUI，采集计划、命令、文件修改、MCP、Web、子代理及完成状态；无计划行为继续执行但归入“未规划操作”，旧 Codex 保留 PTY 并显示不可观测警告。
- Desktop MCP 新增 `development_conversation_bind` 与 `development_conversation_activity_report`，绑定后自动记录后续桌面工具的开始、完成或失败状态，并将游戏实例编辑、运行和测试等领域行为关联到当前计划步骤。
- 每个对话工作区新增 `workflow.json` 最新快照与 `workflow-events.jsonl` 追加事件记录；只保留白名单行为元数据，不保存推理正文、原始命令、命令输出、补丁正文或 MCP 返回结果。
- 游戏实例新增 `background` / `visible` 窗口可见性模式；新实例默认将窗口移出显示区域并从普通任务切换中隐藏，不保持前台焦点，同时避免 `SW_HIDE` 中断 Unity D3D11 出帧，保留 Runtime MCP、UI 查询和截图能力。运行中的窗口化托管实例可从界面或新增的 `game_instance_set_window_visibility` Desktop MCP 工具无激活地恢复显示或返回后台。
- 新增受管 `abya-game-development-task` Skill，覆盖游戏类型/单多人分类、设计采集、只读实例可行性分析、用户确认门、Lua 优先实施、卡死恢复和单人/Host + ClientOnly 验收流程；创建或读取任务时会幂等同步到任务 workspace。
- Codex 任务终端新增 `ABYA_DEVELOPMENT_TASK_ID` 与 `ABYA_DEVELOPMENT_WORKSPACE` 环境变量，使 Skill 能约束当前任务实例范围。
- 游戏实例新增 `editor`、`offline`、`lan-host`、`lan-client` 和 `igp-hosted` 启动模型，使用生产 `--abya-launch-*` 参数快速加载存档/关卡或加入 LAN 游戏，并兼容读取旧 `normal`、`host`、`clientOnly` 记录。
- 每个受管实例新增独立随机 Runtime MCP 端口、43 字符 URL-safe Bearer Token 和强制自动启动；RuntimeBridge 改为认证 HTTP MCP 客户端并保留文本与图片 content block。
- 桌面 MCP 新增实例状态等待、Runtime MCP 等待、启动报告读取、`game_runtime_list_tools` 和 `game_runtime_call_tool`；旧实例 MCP 工具名保留为兼容别名。
- 实例停止结果新增正常退出请求、强制终止、超时和最终存活状态，并在正常退出等待后通过 Windows Job Object 强制清理进程树。
- 新增项目根目录 `.codex` 用于存放项目级 LLM 内容；便携构建会将其完整复制到 `Publish/.codex`。
- Codex 历史对话恢复原生 session，并在应用重启后回放任务工作区中的终端历史；长输出回放改为分块传输。
- 读取任务对话列表时会为旧格式对话回填同一任务工作区中的 Codex 原生 session，避免重启后历史对话被误开成新 session。
- 终端扩大滚动缓冲，任务执行中用户滚动位置不会被新输出拉回底部，并增加回到最新输出的图标按钮。
- 增加“完整输出历史”滚动视图，兼容 Codex 全屏 ANSI 重绘导致 xterm 原生回滚区无法保留历史的问题。
- 可在设置中选择任务工作区根目录；每个开发任务自动创建带唯一 ID 的独立工作区，旧任务首次读取时自动补齐工作区。
- 每个任务支持多个独立 Codex 对话，对话元数据和终端输出转录保存到任务工作区的 `conversations` 目录；每个对话拥有独立 PTY，且不复制 Codex 认证配置。
- Codex CLI 检测补充读取 Windows 当前用户/系统注册表 `PATH` 与 `%APPDATA%\npm`，修复资源管理器早于 Codex 安装启动时桌面工具误报终端不可用的问题。
- 开发任务详情新增“游戏实例 / 终端”分栏；每个任务可保留一个内嵌 Codex CLI PTY，会话跨分栏和任务切换继续运行，并以开发工具进程的当前工作目录作为 Codex 工作区。
- 本地 Codex CLI 检测、Windows `codex.cmd`/原生可执行文件启动、Tauri Channel 输出、xterm 输入与尺寸同步，以及任务删除和应用退出时的终端进程树清理。
- Codex PTY 声明 `xterm-256color` 与真彩色能力；Tauri 开发模式将内部 `src-tauri` 启动目录归一为工具仓库根目录。
- “传输存档”工作区，支持先选择声明 `archive-transfer-v1` 的已连接托管或外部实例，再从固定游戏存档目录扫描或手动选择 `Main.PBArc`，查看存档摘要、启动同步、跟踪进度和取消活动传输。
- 完整存档文件夹校验与传输服务：Deflate ZIP、SHA-256、256 KiB 二进制分块、四块确认窗口、每目标单传输、全局双传输限制、SQLite 状态历史和中断/临时包清理。
- 存档传输 WebSocket 协议，包括能力发现、用户确认、累计字节确认、游戏侧处理进度、结果与取消消息，以及版本化 `ABAT` 二进制帧。
- 五个桌面 MCP 存档传输工具，用于发现目标、发现来源、启动、查询和取消传输。
- `game-archives` 与 `archive-transfer` 独立模块及维护 Skill，并补充桌面/游戏两侧协议文档。
- LAN game-connection gateway with UDP discovery on private IPv4 adapters,
  WebSocket sessions, heartbeat handling, duplicate replacement, bounded log
  batches, persisted-before-ack log ingestion, and archive transfer framing.
- Managed and external game-source persistence with separate process and
  connection states. User-started external games remain outside development
  tasks, collect logs automatically, and expose no process or MCP control.
- Settings controls for gateway status, port, preferred launch adapter,
  broadcast enablement, advertised addresses, connected count, restart, and an
  explicit unauthenticated-LAN warning.
- `game_log_source_list` desktop MCP tool and gateway state in
  `desktop_get_capabilities`.
- Copy-ready desktop MCP configuration templates in Settings for Codex, Claude
  Code, Visual Studio Code, Cursor, Windsurf, Gemini CLI, and generic MCP JSON,
  using the active endpoint and Bearer token.
- Twenty-eight authenticated desktop MCP tools for capability discovery,
  development-task workflows, typed game instance launch and stop, archive
  discovery, selected-game MCP inspection and calls, and structured log
  collection and queries.
- Modular desktop MCP protocol, registry, and application-service dispatcher
  with strict JSON inputs, standard MCP tool results, side-effect annotations,
  and focused registry/dispatch tests.
- Desktop MCP tool catalog documenting inputs, side effects, security
  boundaries, and the generic selected-game MCP forwarding risk.
- Deletion controls for finished game instances and stopped structured log
  sessions, including confirmation prompts, active-state protection, and
  cascading cleanup of associated log events.
- Modular Rust services for task CRUD, typed ABYA launch profiles, managed
  Windows Job Object process trees, archive discovery, game MCP sessions,
  structured runtime log collection, and the authenticated loopback desktop
  MCP foundation.
- Tauri command adapters for settings, tasks, instances, runtime connectivity,
  log sessions, and desktop MCP state.
- Operational bilingual React workspace with task lifecycle controls, visual
  game launch profiles, instance details and termination, structured log
  filters, executable selection, and desktop MCP settings.
- Unity-verified ABYA launch, game MCP, archive, and runtime log contract
  documentation for future LLM maintenance.

### Fixed

- 任务切换后步骤条继续跟随该任务最后实际选择的 Codex 对话，并忽略旧工作流请求或旧终端事件；删除对话时同步清理原生线程绑定，避免迟到通知重新创建已删除的工作流目录。
- 新建 Codex 对话的 app-server 线程会先通过 `thread/inject_items` 持久化受管开发约束，再交给远程 TUI 恢复，避免无首回合线程因缺少 rollout 而降级或打开失败；app-server 初始化失败时同时清理子进程和临时令牌文件。
- Windows 上重复更新 `workflow.json` 时使用唯一临时文件和兼容覆盖流程，并串行化原生 Codex 与 Desktop MCP 工作流写入，避免已有目标文件或并发事件造成快照失败或丢失。
- 移除与当前 Unity 实现不一致的 WebSocket reverse MCP 请求/响应合同；LAN 网关继续专注于结构化日志和存档传输。
- Codex 终端增加始终可见的纵向滚动轨道与滑块，长输出可通过鼠标滚轮或拖动滚动条完整查看，并为滚动条保留稳定空间避免遮挡终端文字。

### Security

- 游戏 Runtime MCP Token 只存在于桌面进程内存和子进程启动参数中，持久化启动参数统一写为 `[REDACTED]`，不会进入数据库、日志、启动报告、实例详情或 MCP 结果。
- The unauthenticated LAN game gateway is explicitly isolated from the
  authenticated loopback-only desktop MCP server.
- Managed Runtime MCP ports and tokens are generated per instance and kept out
  of persisted state and desktop MCP responses.
- The desktop MCP listener enters Tokio only from Tauri's managed async runtime.
- Long task titles no longer compress or wrap the task action controls.
- Portable publishing no longer depends on the optional `Get-FileHash` cmdlet.
- Desktop MCP restarts tolerate the previous loopback listener's short
  shutdown window.

## [0.1.0] - 2026-08-25

### Added

- Initial Tauri desktop application foundation.
- Persistent development tasks and managed ABYA game instances.
- Game MCP integration and structured runtime log collection.
- Bilingual Chinese and English interface.
- Local Streamable HTTP MCP foundation with an empty tool registry.
- Portable Windows release pipeline targeting the `Publish` directory.
