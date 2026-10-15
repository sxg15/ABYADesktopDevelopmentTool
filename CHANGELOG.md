# 开发分支流程澄清（2026-10-15）

- 所有开发在 CLI 上执行；开始前同步最新 dev，完成并推送 CLI 后再同步到 dev，最后保持工作分支为 CLI。
- 将美术参考图上传提交同步至 CLI，并统一 AGENTS、README 和发布 Skill 中的分支约束。

# 美术节点参考图片上传（2026-10-15）

- 美术节点增加独立的策划参考图片区域，支持多图选择、Ctrl+V 粘贴、参考说明、点击放大和移除。
- 图片保存在任务工作目录并登记到制作记录，重启后仍可读取；不要求先有 AI 成果。
- 提供“让 AI 参考并继续”，使用原会话及既有人工确认流程；参考图不充当成果或验收证据。
- 校验大小、格式、任务状态及记录版本，防止重复点击、异步串任务，并明确展示部分上传失败。
- 更新完整运行包为 art-reference-upload-20261015，保留客户端管理、终端与验收修复，提交至 dev。

# 游戏客户端管理合入 dev（2026-10-15）

- 将远端客户端管理、设置页 GitHub 浏览器登录和 ZIP 下载修复合入 dev。
- 保留 dev 的 Codex 大历史传输、验收界面、程序缺陷提醒、AI 修复入口和后台通知更新。
- 合并源码后重新构建完整便携包，使用 game-client-dev-20261015 标识，并执行完整检查和发布清单校验。

# Codex 长会话读取修复与分支同步约束（2026-10-15）

- 将 Codex app-server 的 WebSocket 单帧和完整消息接收上限显式设为 128 MiB，修复超过默认 16 MiB 的历史响应导致接回原对话失败。
- 响应超限时返回专门诊断，不再误报普通断线；保留原会话身份，不截断历史、不自动重放任务。
- 增加真实回环 WebSocket 回归，覆盖 11 轮大历史、分片消息、后续请求和超限拒绝。
- 更新项目约束：CLI 完成后自动将源码、Skill 与已验证完整运行包同步到 dev，核对远端包含关系；不更新 main。

# 验收界面重复与响应阻塞修复（2026-10-14）

- 修复人工试玩和反馈面板使用相同组件 key，切换阶段后残留、叠加验收卡片的问题。
- 验收状态查询改为单次完成后再调度，隐藏页签停止查询；忽略过期结果并立即防止重复启动点击。
- 验收启动和状态读取移入阻塞工作线程；状态查询使用只读任务信息，并优先使用当前启动进度。
- 合并 dev 的 Windows 通知功能与此前程序缺陷提醒、AI 修复入口，保留既有任务与人工决定。
- 后续源码与完整运行包只同步 dev，不再推送 main 或 CLI。

# 游戏客户端下载修复（2026-10-15）

- 修复下载请求重复携带 JSON 与二进制 Accept 值，导致把 GitHub 文件说明当作 ZIP、误报大小校验失败的问题。
- 下载只请求二进制；JSON、XML 或网页响应单独提示，不进入安装流程，大小与 SHA-256 校验继续保留。
- 增加完整请求头断言、异常响应拦截、失败保留当前游戏选择，以及可选的真实 Release ZIP 下载和隔离安装验证。

# GitHub 浏览器登录与设置入口（2026-10-14）

- GitHub 登录统一放到设置页，客户端管理未登录时提示状态并提供“前往设置登录”按钮，跳转后定位账号区域。
- 使用注册的 OAuth 应用打开浏览器授权，移除手动 Token 输入；支持一次性验证码、复制、取消、过期/拒绝后重试和仅本次登录。
- 原生服务加密保存访问与刷新凭据并自动续期，兼容旧登录记录；并发刷新串行处理，取消或退出后旧请求不能恢复登录。
- 登录成功或退出后刷新设置中的远端版本列表，保持游戏来源与版本草稿；增加授权协议、加密重启、并发和设置交互验证。

# 远端游戏客户端管理（2026-10-14）

- 新增左侧游戏客户端管理页，固定连接 sxg15/PlatverseOutput，支持 Token 登录、退出与 Windows 当前用户加密记住登录。
- 支持创建/编辑 Release、上传产物、确认删除版本或产物；包含私有仓库、只读权限、草稿、不可变版本与空列表处理。
- 完整 Windows Unity ZIP 包按用户指定目录独立安装，保留旧版本；展示下载/解压进度并支持取消。
- 校验大小与远端 SHA-256，拒绝越界路径、链接、设备名、文件冲突和超限解压；失败不会替换已有客户端。
- 设置页新增本地 EXE/远端版本选择，保留原本地路径，多 EXE 时明确选择；下载完成后需保存才切换使用。
- 新版本仅提供可选更新提醒，旧版本可继续离线使用，已有任务固定的游戏版本保持原有优先级。
- 增加隔离安装、旧设置迁移、凭据隔离、GitHub 协议、更新判断与界面交互回归测试。

# Windows 后台人工操作提醒（2026-10-14）

- 全部进行中任务共用后端人工待办投影；后台每 3 秒补偿检查，前台保留操作指引。
- 新增原生 Toast、当前用户应用身份与 COM 点击激活，兼容便携版与通知中心。
- 问答、文档版本、原终端等待、试玩与阻塞按具体身份只提醒一次；SQLite 持久去重，基线和前台待办不补弹。
- 点击恢复并定位对应任务与入口，重新核对过期状态，不自动回答、批准、继续或新建会话。
- 设置页新增默认开启的后台提醒开关、系统不可用原因与测试入口；测试环境使用独立身份。
- 后台只读接口不创建工作区、不同步 Skill/原生会话；正常关闭时清理通知，保持原有退出行为。

# 普通问题的 AI 自动修复入口（2026-10-13）

- 普通未解决问题增加“让 AI 自动修复”，绑定当前 Codex 原会话、问题 ID 与记录版本；源码／构建包问题保留程序处理提醒。
- 空闲时发起修复，忙碌时排队，未连接时接回同一会话后投递；重复点击不重复发送，不覆盖其他待办。
- 投递前复核问题是否变化、解决或改判为程序修复；人工问答和审查仍保留，不自动标记修复成功。
- 持久化修复请求与回执，结果不明时要求核对；暂停后不自动恢复。Grok 下明确提示使用 Codex 原会话，不模拟键盘提交。

# Lua 阶段的程序缺陷红色提醒（2026-10-13）

- ABYA 游戏源码、构建包缺陷在任务顶部和 Lua 阶段显示红色提醒；问题卡片明确注明“需要程序修复”，优先排列。
- 新增可选 repairTarget 归属字段并校验；旧记录仅在明确请求程序排查修复时标为疑似，不把普通 Lua 问题判为程序缺陷。
- 已解决问题退出红色待办提醒，报告保留源码／构建包归属；同步 Codex/Grok 记录说明，不改变既有阶段门槛或升级历史任务。

# 重建会话的启动检查与恢复入口（2026-10-13）

- 将 APP 的受认证 CLI 连通性检查与工作区独立沙箱预检分开；服务正常时，沙箱预检异常不再锁死原生终端与执行设置。
- 沙箱异常展示具体分类、退出码和系统错误码；仍保留实际工具执行的沙箱与审批，不自动发送任务或改动用户配置。
- 删除、新建和归档期间暂停会话刷新，防止窗口恢复焦点时重建空列表或覆盖新会话；删除前解除旧终端视图。
- 清除已删除会话的后台续作索引；新增真实删除/重建、沙箱失败与服务停止隔离测试。

# 终端中文输入法位置稳定（2026-10-13）

- 修复 AI 输出重绘期间，中文拼音与候选框跟随临时输出光标上下跳动的问题。
- 选词期间固定 xterm 输入法定位，提交、取消或失焦后释放；不阻塞终端输出，不自行拼接或重复发送中文。
- Codex/Grok 共用修复；新增连续组合输入、提交时序、失焦清理和真实 xterm 重绘回归。

# Codex 原会话恢复修复（2026-10-13）

- 区分写入者占用、请求超时、会话缺失、归档和后端断开，保留原生错误码、方法与诊断编号。
- 失败连接清理本次创建的后端，避免残留占用；不终止其他客户端、不归档切换、不新建替代对话。
- 终端连接状态与任务执行状态独立显示；接回后显示“任务等待继续”，不会自动重发已有操作。
- 恢复失败使用完整可换行提示和“接回原对话”入口，显式区分重连与继续任务。
- 增加占用后重试、脱敏诊断、失败资源释放、待续作状态及界面恢复回归。

# 拉取即用的完整仓库分发（2026-10-13）

- 正式 Publish 运行包纳入 Git，包含 APP、CLI、Node、FFmpeg 及 Codex/Grok 工作流资源。
- 随 Node 运行时附带对应版本的官方授权文件，并在后续发布时检查版本匹配。
- 新增相对路径“ABYA 开发工具.cmd”，不再依赖开发者本机绝对路径快捷方式。
- 发布生成逐文件哈希清单；启动前检查完整性，完整检查可检测损坏、漏传及被忽略的文件。
- 固定运行包字节，避免 Git 换行转换造成签名不一致；拒绝将数据库和账户凭据纳入发布包。
- 补齐策划启动说明，并以独立路径的新检出验证分发完整性。

# 行动分栏与显著操作引导（2026-10-12）

- 按确认的“行动分栏”设计，将指引放到任务标题下方，独立于页签滚动区域；左侧状态与要求，右侧处理位置与操作按钮。
- 使用 24px 标题、16px 操作要求、15px 按钮及蓝/橙/紫/红/灰状态配色；等待状态为次要入口，人工待办为主操作。
- 页签显示待回答、待审查和终端待处理标记；“查看并审查”直接打开对应版本，默认不选择通过。
- 人工待办变化仅短暂强调一次，不抢焦点；状态读取失败清除过期待办。窄窗口按内容宽度重排，低窗口保留终端空间。
- 新增状态、直接审查跳转和过期数据回归，以及真实 TasksView 的隔离浏览器预览；UI 测试包使用独立发布标识。

# 交互引导与美术确认（2026-10-12）

- 问答增加“其他”与独立补充说明，移除查看全部答案、手动保存和取消问题按钮；自动保存保持输入焦点，提交成功自动关闭。
- 统一展示执行等待、阶段问答、成果审查及终端人工交互指引；继续与连接反馈保留在主页面。
- 阶段确认和成果反馈移入二级窗口，默认不选择结论，核对当前文档和关联视觉版本；空成果、草稿、缺失文件不能反馈。
- 概念图点击直接预览，支持缩放、拖动、切换与键盘返回；同步完善弹窗焦点、响应式布局和错误提示。
- 流程 2.1 增加独立美术需求确认单及 APP 批准节点，支持新增/复用/无美术三种范围；同步 Codex/Grok 模板、报告和升级兼容。
- 增加自动保存并发输入、输入法、预览、版本变化、反馈门槛及美术审批回归。

# APP 0.4.0 验收与反馈流程（2026-10-10）

- 修复 Codex 能力检测命令缺少 Windows 后台运行标志的问题，避免 `app-server --help` 打开额外终端窗口。
- 一键验收绑定任务实际存档和Player，统一APP/CLI启动，核对玩法初始化与双端就绪，处理重复启动和失败清理。
- 资源节点增加版本化图片/视频预览、方案对照和计划图片摘要绑定，支持策划上传反馈附件。
- 流程2.0取消固定8＋4，使用必要自测、按影响范围复验和人工反馈闭环，最终确认仍绑定具体作品版本。
- 显式升级保留旧任务批准文档、轮次与证据，不补造新自测；同步Codex/Grok Skill、报告与时间统计。

# APP 0.3.0 制作工作台与终端设置（2026-10-09）

- 制作流程采用顶部阶段导航，按阶段展示成果、草稿、问题、问答和迭代记录。
- 顶部对话步骤支持 CLI 结构化计划登记，保持活动与步骤关联。
- 增加原生模型、思考强度和权限快捷设置，支持后续轮次应用及会话恢复。
- 修复模板补登记和阶段状态，新增阶段问答、问题影响范围和保留原批准的兼容升级。
- 按钮提示默认放在上方，统一简洁自然的进展和报告规则；增加历史数据与执行设置回归。
- 保留既有终端输入和按窗口录制修复；工作流版本更新为 1.3.0。

# 托管窗口录制修复（2026-10-09）

- APP 0.2.2 合并终端输入恢复与窗口录制修复，正式发布标识 terminal-and-recorder-fix-20261009。

- 使用随包 FFmpeg 的 Windows Graphics Capture 按已验证 HWND 捕获 GPU 游戏窗口，避免 GDI 把遮挡窗口写进录像。
- 拒绝零窗口句柄，不回退为桌面/显示器或名称匹配捕获；保持实例归属、无音频和可见性恢复约束。
- 增加捕获参数与遮挡窗口像素内容回归检查；录制完成仍须实际画面审阅。

# 终端恢复输入修复（2026-10-09）

- 历史回放移除旧终端控制指令，避免把历史查询和输入模式带入新的终端握手。
- 固定 portable-pty 的最小 Windows 补丁，关闭父控制台光标继承，保留依赖许可证与补丁来源。
- 管道输入与终端尺寸调整放入阻塞线程池；连接未完成时延后尺寸更新，连接和继续后恢复输入焦点。
- 已连接的终端即使模型暂停也能点击暂停关闭，避免失去修复连接的入口。
- 增加回放控制指令隔离、连接期间尺寸更新及输入焦点测试。

# 文档文件位置入口（2026-10-08）

- 需求、执行计划、交付文档增加“在文件资源管理器中显示”，打开所在目录并选中文档。
- 复用任务文件路径校验，打开失败时显示错误；不改写文档、不影响用户确认状态。

# 续接队列与正式发布入口（2026-10-08）

- 快速答题与文档决定在原回复结束后续接；答案保存与继续意图一次提交，重复点击合并。
- 暂停撤销待执行请求，重启不自动执行；不确定的发送结果保留记录并要求核对。
- 统一步骤栏和终端状态，区分继续任务、重新连接与历史输出；补充受控竞态回归。
- APP 升至 0.2.0，工作流 1.2.1；内嵌版本/构建标识、数据目录单实例与 Windows 正常数据上下文启动。
- 验证包通过后可备份替换 Publish 并创建固定入口，不迁移或覆盖用户数据库。

# AI 制作流程 APP/CLI 接入（2026-10-03）

- 新建任务选择完整/局部范围和提问模式；增加制作流程、版本确认、问题/轮次、报告及 Skill 库页面。
- 制作状态由数据库与修订历史维护，CLI 提交产物；用户在 APP 确认具体版本，不能通过 CLI 自行批准。
- 从受管实例的实际存档和 Player 计算版本指纹，检查旧证据、缺失问答、未关闭问题及十二轮顺序。
- 制作任务固定 Skill/题库，普通读取不刷新；显式升级保留备份和历史，重新确认适用文档。
- 增加受管窗口实时录制工具，15 fps H.264、无音频；录制时显示窗口且不抢焦点，结束恢复原状态。
- 生成三类固定报告和可恢复 JSON；提供包含固定录制依赖的隔离便携包，不覆盖正在使用的 Publish。

# AI 制作流程第一阶段（2026-10-02）

- 主 Skill 收窄到完整玩法、较大改动及明确指定阶段；按需读取，局部修复不自动启动完整流程。
- 固定需求、计划、交付三个版本确认点，新增提问模式、阶段资料模板和 R0/8＋4 题库约定。
- 现有素材优先，模板选择随文档确认；同步消除多人、美术模板及导入器的旧选择规则。
- 统一维护首批 Codex/Grok 工作流资源并检查差异；任务部署验证覆盖新增资料和既有轮次保留。
- 此阶段仅落地 Skill/资料契约，尚无 APP 阶段界面、后端确认门禁或新的流程 CLI 命令。

# 复制全部对话历史

- Codex/Grok 终端右上角增加“复制全部对话历史”，支持运行中、已退出及历史视图。
- 从磁盘完整终端记录读取并清理 ANSI 控制串，不受 256 KiB 回放与 2 MiB 历史视图限制；后台直接写入 Windows 剪贴板。
- 增加复制中、成功、空历史和失败提示，以及长历史、中文、跨块控制串与重试验证。

# 人机开发流程接入

- 增加可选真实 Player 人机就绪检查，分别核对两种 provider 的定义读取、诊断与 receive/submit_action API，避免将旧游戏包误作可用测试版本。

- Codex/Grok 公共任务 Skill 与多人模板接入按需人机规划、四档策略、候选暂存、完整架构校验及真实双端验收。
- 增加任务侧计划/报告 Schema、定义快照与只读 Node 校验器，检查无关定义保留、写前变化、保存内容、Player 和运行证据归属。
- 兼容运行时数字难度枚举与写入字符串枚举；超时后要求读回，不将拓扑版本当作定义版本锁。
- 发布预检和新旧任务部署测试覆盖完整人机资源；增加失效证据、独立客户端、四档覆盖和结果状态的反例测试。
- 发布脚本支持固定隔离目录的 -Staging 验证，桌面程序运行时可验证新包而不覆盖正在使用的程序。
- 本地材料检查与真实玩法验收分别报告，不自动改变用户任务状态；实机覆盖及阻塞见 docs/BOT_PIPELINE_VALIDATION.md。

# 终端粘贴与 Codex 会话整理

- 接管 Ctrl+V/Ctrl+Shift+V，读取 Windows 原生剪贴板；文本按 bracketed paste 插入，图片才交给 Codex 原生图片粘贴。
- 长文本按 Unicode 安全边界串行发送，避免 IPC 超限、重复粘贴及意外提交。
- 自动注册或复用 ABYA 任务对应的 Codex 项目，同步历史会话归属和名称。
- 增加归档/恢复和已归档列表；通过原生 API 同步，阻止归档会话自动复活，删除本地记录前保留原生归档历史。
- 刷新或重新聚焦时同步外部归档/恢复；原生同步失败时保留本地列表并明确显示错误。

# Windows 沙盒 CLI 连接修复

- 托管终端使用本机命名管道和会话级短期凭据，避免沙盒用户解密原用户的 DPAPI 令牌。
- 校验管道客户端身份与任务/会话/提供方；终端停止、凭据过期和服务重启会撤销访问。
- 默认工作区移至用户目录外层；旧默认工作区在打开终端前复制迁移，保留原目录作为恢复副本，自定义目录不自动迁移。
- Codex 打开终端前通过真实 Windows 沙盒执行 CLI 自检，区分目录启动失败和服务连接失败。
- 增加真实沙盒、模型工具调用、凭据隔离和工作区文件保留测试。

# Codex 后台版本与任务环境修复

- CLI 更新后不再让新终端继续复用旧 app-server；同时识别退出的后台进程。
- 保留模型选择、原生会话与历史；其他活动终端不会被自动中断。
- 新建和恢复后台会话时按会话注入五个 ABYA 环境变量，避免远程工具执行环境缺少 CLI 和任务身份。

# 纯 CLI 开发管线（开发中）

- 保留现有桌面应用，新增 abya-desktop 命令入口并移除桌面协议适配器。
- 游戏操作改用随包 Abya CLI，加入托管实例身份、稳定会话、取消和图片文件输出。
- 更新双 provider 工作流、设置界面和发布依赖。
- Unity/真实 Player 的完整验收结果见 docs/CLI_MIGRATION_VALIDATION.md。

# Changelog

## 0.3.0 — 2026-10-09

- 将制作流程改为顶部阶段导航，按阶段集中展示成果、草稿、问题、问答和迭代记录。
- 复用顶部对话步骤，增加CLI结构化计划登记，保留步骤对应的活动历史。
- 增加模型、思考强度与权限快捷设置，通过原生会话接口生效并随会话恢复。
- 修复模板补登记与阶段状态，支持阶段问答、问题影响范围以及保留批准的兼容升级。
- 按钮提示优先显示在上方，统一面向策划的自然语言进展和报告规则。
- 保留终端输入及按窗口录制修复，增加历史数据、状态、设置与界面回归。

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

- 当前 Codex 桌面项目列表与 CLI 原生项目注册不完全同步时，提供只读任务路径与一次性添加项目引导。
# 工作流首轮测试修复（2026-10-08）

- 人数模式独立记录为未明确、单人或多人；需求问答支持上一题、整组预览、草稿恢复和已提交答案修订。
- 需求问答在 APP 弹窗中处理；终端页也会提示新问题，关闭后保留草稿与继续填写入口。
- 需求问题由 CLI 发布到 APP，答题由策划完成；保留三处文档确认。Codex 续接校验原生 ID 和答案版本，持久回执防止重复执行。
- 每个 Codex 对话独立运行后端，暂停先中断原生执行，再关闭后端及撤销授权；准备失败不静默创建新会话。
- 恢复失败保留界面历史；会话信息显示原生 ID，并提供有备份、限当前目录的旧会话关联修复入口。
- 流程更新至 1.2.0，保留旧任务固定版本；新增隔离 Recovery 测试包、连接事件与原生用量快照。
# 修复问答中断续接（2026-10-08）

- 继续操作核对原生执行状态；中断/失败可创建新尝试，执行中/已完成不会重复发送，旧回执保留。
- 终端未打开时自动连接发布问题的原会话再继续，不静默新建对话，不重放结果未知的写入。
- 已处理、正在执行和后续对话已继续分别反馈，消除“提示已发送但没有执行”的误报。
- 未答完时定位缺失问题；填写校验不再要求刷新记录，修正答案后恢复自动保存。
# 图标说明与报告目录（2026-10-08）

- 图标按钮显示 APP 内悬停说明，键盘聚焦也可查看，覆盖弹窗和禁用按钮。
- 阶段报告按钮改为打开所在文件夹并选中对应报告；保留固定路径校验和报告更新。
