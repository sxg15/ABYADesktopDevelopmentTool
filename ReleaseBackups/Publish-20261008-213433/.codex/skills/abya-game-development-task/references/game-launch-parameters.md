# ABYA 游戏启动参数

来源：`C:\Users\Administrator\Desktop\ABYA游戏启动参数列表.md`

同步日期：2026-09-01

适用于首场景为 `UGCStartScene` 的 Windows 打包程序。桌面开发工具和 Skill
必须使用结构化字段生成这些参数，不接受任意原始命令行。

## 参数规则

- 参数支持 `--key=value` 和 `--key value`。
- 路径或用户名包含空格时由进程启动 API 负责正确传参。
- `--abya-launch-archive-guid` 与 `--abya-launch-archive-path` 至少提供一个。
- `--abya-launch-level-guid` 可省略；省略后使用存档起始关卡。

## 生产启动模式

| 模式 | 行为 |
| --- | --- |
| `editor` | 进入编辑器场景并加载指定存档和关卡。 |
| `offline` | 直接加载指定存档并进行非联机游戏。 |
| `lan-host` | 创建 LAN 房间并以 Host 身份进入指定存档。 |
| `lan-client` | 使用本地存档资源连接 Host；实际关卡由 Host 决定。 |
| `igp-hosted` | 使用 IGP 房间启动上下文继续托管联机流程。 |

## 生产参数

| 参数 | 说明 |
| --- | --- |
| `--abya-launch-mode` | `editor`、`offline`、`lan-host`、`lan-client` 或 `igp-hosted`。 |
| `--abya-launch-archive-guid` | 本地存档 GUID。 |
| `--abya-launch-archive-path` | 本地存档目录绝对路径；与 GUID 同时提供时必须指向同一存档。 |
| `--abya-launch-level-guid` | 可选关卡 GUID。 |
| `--abya-launch-host` | `lan-client` 的 Host 地址，默认 `127.0.0.1`。 |
| `--abya-launch-host-port` | LAN 端口，范围 `1-65535`，默认 `7777`。 |
| `--abya-launch-report` | 启动状态 JSON 文件，状态为 `resolving`、`launching`、`ready` 或 `failed`。 |
| `--abya-launch-exit-on-failure` | 启动失败时是否以退出码 `1` 关闭。 |

## Runtime CLI 参数

桌面使用 --abya-cli-autostart=true，CLI 自动发现托管 PID 并验证桌面实例身份。
临时凭证由游戏生成，不出现在 AI 参数或日志中。开发授权仅在桌面启动的进程启用。

## 桌面开发工具连接

托管实例同时接收：

```text
--abya-devtool-autoconnect=true
--abya-devtool-protocol=1
--abya-devtool-endpoint=ws://<host>:<port>/game/v1/connect
--abya-devtool-instance-id=<instance-id>
```

WebSocket 用于结构化日志和存档传输；Runtime CLI 使用独立的 loopback HTTP
连接。

## IGP 托管

`igp-hosted` 可使用 `--arena-launch-ticket`、
`--arena-bootstrap-endpoint`、`--arena-bootstrap-secret`、
`--igp-ipc-pipe` 和正整数 `--igp-app-id`/`--arena-app-id`。这些字段属于敏感
启动上下文，当前桌面 Skill 不收集或持久化它们；只有已有的安全设计提供内存
凭据时才允许启动。

## 兼容边界

旧 `--abya-test-role`、存档、关卡和报告参数只用于旧自动化验收，不能与
`--abya-launch-mode` 同时使用。新任务必须使用生产参数。桌面工具仅保留
当前运行时独立解析的 `--abya-test-user-id` 和
`--abya-test-user-name` 作为同机多实例身份覆盖，不传
`--abya-test-role`。

通用参数包括 `-language=<locale>` 和每实例唯一的
`-logFile <path>`。
