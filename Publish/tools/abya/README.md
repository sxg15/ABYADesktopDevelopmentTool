> 此目录是桌面随包快照。这里的 npm test 运行 12 项运行时/桌面契约测试；包含 Unity 源码边界的全部 26 项测试在 AbyaPB/Tools/AbyaCli 中运行。

# ABYA CLI（开发预览）

这是项目自研能力的命令行入口，不代理 Unity Editor MCP。当前仅在 Windows
提供自动实例发现；需要 Node.js 20 或更高版本。Unity Editor Play Mode
会自动启动 CLI 宿主；正式 Player 仍必须用 `--abya-cli-autostart=true`
显式启动，默认不开启。

在 `Tools/AbyaCli` 下执行：

```powershell
npm ci
node ./bin/abya.mjs status --json
node ./bin/abya.mjs capability list --json
node ./bin/abya.mjs capability describe runtime_get_status --json
'{}' | node ./bin/abya.mjs capability run runtime_get_status --input-file - --json
```

多实例使用 `--instance <pid>`；`--project <absolute-path>` 可按项目路径选择。
若选择结果有歧义，命令会报错，不会自动挑选一个进程执行写入。桌面 Player
的 `--project` 对应构建的 `*_Data` 路径。连接已知本机实例时也可传入
`--endpoint http://127.0.0.1:<port>`，并通过当前进程环境变量
`ABYA_CLI_TOKEN` 提供令牌，不在命令行参数中传令牌。

实例登记保存在当前 Windows 用户的
`%LOCALAPPDATA%\AbyaPB\Cli\instances\`，含临时访问凭证；不要分享或
加入版本库。退出 Unity 后会清除本实例登记。CLI 仅访问回环地址。

打包后的 Player Agent 目前只允许 `PlayerCapabilityAllowlist.cs` 中的
自研能力；新注册能力不会自动开放。白名单仍须在真实 Player 上完成发布
审核。内置 APA 也按该清单过滤，Editor 开发态维持原能力目录。
旧自研协议入口已退役，Editor 与 Player 均使用 CLI。

`--json` 将一个 JSON 文档写到 stdout，诊断写 stderr；返回码 0 表示成功、
2 为参数错误、3 为实例不存在或歧义、4 为认证错误、5 为能力不可用、
6 为执行失败、7 为超时或取消后结果不确定。截图须指定
`--output-dir <path>` 保存原始文件，避免在终端输出 base64。

Ctrl+C 会请求 Unity 取消未完成的操作，但已进入提交阶段的修改可能已经
生效。此时退出码 7 不代表回滚，需查询实际项目状态再决定是否重试；
CLI 不自动重试写入。


## 构建命令

新增 build/check 子命令，独立启动 Unity，不使用运行时实例发现和令牌。
原有 status/capability 的参数、协议及退出码保持不变。

```powershell
node ./bin/abya.mjs check --project "D:\AbyaCliValidation\AbyaPB" --config BuildConfigs/windows-release.json
node ./bin/abya.mjs build --project "D:\AbyaCliValidation\AbyaPB" --config BuildConfigs/windows-release.json --json
node ./bin/abya.mjs build --help
```

可执行 npm link 注册当前目录的 abya 命令；不需要安装全局发布包。
Unity 定位依次使用 --unity、ABYA_UNITY_PATH、常见安装目录，并要求完整版本一致。
--output-dir 覆盖配置；--auto-fix 仅用于 build；--timeout 默认 7200 秒。
场景及发布配置参见 [配置说明](../../BuildConfigs/README.md)。
完整流程、独立副本准备和报告协议见 [自动化打包说明](../../自动化打包说明.md)。

构建子命令退出码：0 成功，2 修复未完成，3 预检失败，4 构建失败，
10 参数或执行异常，20 启动失败，21 超时，22 取消，23 报告异常。
--json 的 stdout 始终为单个结果对象；日志和进程诊断写 stderr。
使用 Logs/AbyaBuildCli/<任务ID>/result.json 与 unity.log 定位失败。
构建成功要求本次任务报告和进程退出码同时确认；不自动重试失败任务。

在本目录运行 npm test，覆盖运行时 CLI 回归、配置及构建进程生命周期。

## 桌面开发管线

桌面软件仍然是任务和实例管理主体，新增 abya-desktop 命令入口。
托管游戏同时具有 CLI 显式启动参数、桌面实例身份和 ABYA_CLI_DEVELOPMENT=1 进程环境时，
ExternalCli 使用 DesktopCliCapabilityAllowlist 的固定开发能力快照。内置 APA 的白名单不变。
这属于当前 Windows 用户拥有的开发进程授权，不是针对同用户恶意进程的安全隔离。
ABYA_CLI_SESSION_ID 固定资源主体，ABYA_CLI_REQUEST_ID 关联一次操作，
ABYA_CLI_DESKTOP_INSTANCE_ID 验证所选游戏确实属于本次桌面启动。
取消命令为 abya cancel <requestId> --instance <pid> --json，必须使用原会话身份。
批准拒绝返回 success=false / executionState=not_executed；超时结果不确定，禁止自动重试写入。
