# 通用任务空白初始化验证

用户在本轮先要求发布已完成专项验证的版本，随后要求补充对应工具并“做成通用的吧，别的开发任务也可以用”。本次补充只实现任务级空白存档初始化，不绑定任何玩法、试跑编号或旧任务，不修改完整游戏验收结论。

## 能力

`archive create` / `game_archive_create` 使用当前会话的任务，接收稳定 UUID `creationId`、`name`、`author`，以及可选 `executablePath`。省略运行包路径时使用 APP 配置，已固定运行包的任务仍必须匹配原固定版本。

通过原生主菜单接口创建存档、直接打开开始关卡并读回真实身份。普通任务可用；完整制作任务须已批准需求和计划。每个任务有独立数据根与创建回执。同 ID 只返回原结果；参数冲突拒绝；创建后的未知结果保留已有成果和实例，不自动重发。`archive creation-status` 可在实例停止后继续查原结果。

关闭成功实例后，普通 `instance launch` 使用返回的 archive 对象重新打开，继续沿用原任务私有数据根。创建成功不等于游戏实现完成或验收通过。

## 配套 Player

Unity 工程同步补充：托管启动跳过按键等待；三种存档入口允许已初始化主菜单；`editor_load_archive.openStartLevel=true` 直接打开真实开始关卡。普通调用省略参数仍保留人工选关。其他编辑写入不开放到主菜单。

实测 Player：`C:/Users/Indiegamespass/ABYA-Validation/archive-bootstrap-20261102/Player-v2/ABYA_PB.exe`。
这是独立 Mono 专项验证包，未覆盖旧 Player、未替换既有任务的运行包固定记录，不是完整 IL2CPP 引擎发布。
其 `ABYA_PB_Data/Managed/Assembly-CSharp.dll` SHA-256：`d223e3c99554273fbe603ee32593524fbc7033330a24b8aa2b02ecd0c386491a`。
旧 Player 缺少免按键启动/主菜单入口/显式开始关卡契约时，工具在创建前报告兼容性缺口。需要在新任务使用上述兼容包或后续包含同一接口的正式 Player。

## 真实验证

专项环境：`C:/ABYA-Archive-Create-20261102-v2`。使用打包后的桌面 CLI、任务会话认证和原生 Player，没有外部制作存档。

| 检查 | 结果 |
| --- | --- |
| 两个普通开发任务从零创建并加载开始关卡 | 通过；真实存档 GUID 和路径不同 |
| 两个任务使用同一 creationId、同名项目 | 通过；回执、实例及文件相互隔离 |
| 同任务同 ID 重复调用 | 通过；沿用原实例，无新增存档 |
| 同 ID 更换参数、指定其他任务查询 | 均拒绝 |
| editor_get_archive 实际读回 | 两个任务的 GUID、关卡和加载状态均匹配 |
| 停止后通过普通实例启动重新加载 | 通过；同一存档和关卡 |
| npm run check、格式、Clippy | 通过 |
| Rust 测试 | 351 项库测试、2 项 CLI 测试通过；14 项既有可选测试未运行 |
| Unity 上下文/参数测试 | 7/7 通过 |
| 完整便携包清单 | 553 个文件，逐文件哈希验证通过 |

冷启动首次读取时仍处于 SceneTransition，测试保留原始响应，改为有界只读等待真实编辑器上下文后验证；没有重发启动或创建。
脚本：`scripts/archive-bootstrap-smoke.mjs`，设置 ABYA_ARCHIVE_QA_ROOT 和 ABYA_TEST_PLAYWRIGHT 后按 setup、create A、create B、verify、cold、cold-check、cleanup 执行。需预先启动相应隔离 APP。

早期两类失败记录保存在 `C:/ABYA-Archive-Create-20261102`：旧 Player 主菜单能力缺口；首个新 Player 已创建存档但停在人工选关页面导致加载超时。后者保留存档和未知结果回执，没有盲目重发。最终 Player-v2 修正了显式开始关卡路径。

## 桌面交付身份及边界

- APP SHA-256：`6fda0106cb6171de4ab1fd19190ac3eeb28cd5d82b90aab492acebfd59172890`
- inventory SHA-256：`17bc502739b4475b0d773684306bbe6437bf1fc9c9225dcc17503af7ea139a57`
- runtime source fingerprint：`14bcc4e172db1fa82aa5149ef6b5195c62a9a15154c3f992907979417a65508d`

这次专项不代表三类游戏六次原生完整试跑通过。旧批次仍使用原 APP/Player，保留原目标、时间预算、失败和干预记录。本工具补充不更改标准完整试跑发布门槛，也不自动迁移或批准旧任务。
