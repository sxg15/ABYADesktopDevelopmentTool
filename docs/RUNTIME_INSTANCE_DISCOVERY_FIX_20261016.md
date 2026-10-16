# Runtime 实例选择修复验证（2026-10-16）

## 行为

内嵌 CLI 与 AbyaPB `Tools/AbyaCli` 同步：完全相同的登记记录去重；存在多个候选时，以只读、带认证的 status 请求核验 PID、工程路径及当前托管桌面实例身份，排除失效端点。核验按四个并发请求分组，单请求限时 1.5 秒。

两个有效端点继续返回 `ambiguous_instance`。发现流程不删除登记、不调用玩法写入、不自动重试写入，也不依赖文件更新时间选取实例。Runtime GUID 相同不直接等于 CLI PID 选择歧义。

## 验证结果

- `npm run check` 通过：模块约束、工作流/模板、前端、CLI 和便携包检查。
- `cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 通过。
- Rust：179 项通过、0 失败；10 项需外部环境的可选测试保持忽略。
- AbyaPB canonical CLI：34/34 通过。涉及本次修改的五个源码/测试文件与桌面快照在统一行尾后内容一致。
- Validation 发布流程成功，包含 APP、桌面 CLI、Node、FFmpeg、两套 provider Skill 和 runtime CLI。
- 399 个文件全量 SHA-256 清单验证通过，合计 317966591 字节。
- 移至 `logs/anomaly-blockers-20261016/中文 便携验收/Publish` 后，全量哈希再次通过。
- 使用迁移包自带 Node 执行 8 项实例发现和四进程回归，全部通过。
- 迁移包 APP 在独立 LOCALAPPDATA / WebView 数据目录与 `ABYA_TEST_MODE=1` 下启动成功；随包 `abya-desktop doctor --json` 返回 `success=true`、`runtimeCliInstalled=true`。
- 隔离数据库完成旧本地客户端选择迁移，GitHub 凭据行数为 0。未启动 provider 会话、未访问用户任务。

四进程测试使用四个真实 Node 子进程模拟 CLI 协议端点，不代表四个 Unity Player 或原玩法验收。本次不改变产品 UI；原生检查验证启动和 CLI 连通，没有把它表述成人工界面验收。

## 发布身份

- 版本：0.4.0，workflow 2.1.0。
- releaseLabel：`runtime-instance-discovery-20261016`。
- APP SHA-256：`5D6AD6C035D38BF92925683D0EF7BD1CB61EADBEED45CCD364A449BCA103D221`。
- CLI SHA-256：`3F32327CBED84989927A17094AB7750F6BC26B8C73821EEB7DE1F8376FE32665`。
- 完整构建日志：`anomaly-publish.log`（本机临时记录，不分发）。

按仓库约定，将已验证完整包提升到 Publish，保留上一包备份；先提交并推送 CLI，再合入 dev 并确认远端包含关系，最后回到 CLI。
