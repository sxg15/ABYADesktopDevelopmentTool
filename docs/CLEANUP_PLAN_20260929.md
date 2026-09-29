# Abya 旧包与开发工具清理清单

盘点日期：2026-09-29。状态：用户已批准，A 类 10 项全部删除，B/C 类保留。
实际净增可用空间 91.095 GiB，另保留 2,656 个差异/记录文件（3.232 GiB）。
结果与额外健康检查情况见 [执行记录](CLEANUP_RESULT_20260929.md)。下文保留清理前盘点供追溯。
容量为不跟随目录链接的文件逻辑大小（GiB），实际释放空间可能因文件系统而不同。
本清单不包含主工程 D:/Unity项目/AbyaPB 的源码、Unity 安装、用户游戏存档或任务工作区。

## 当前使用情况

- 正在运行：D:/Unity项目/ABYADesktopDevelopmentTool/Publish/ABYA Desktop Development Tool.exe，PID 53272。
- 盘点时未发现从 D:/AbyaCliValidation 启动的游戏或 Unity 构建进程。
- 最新可用游戏：D:/AbyaCliValidation/Bot-Player-20260929/ABYA_PB.exe，必须保留整个配套目录。
- 默认配置库的“目标游戏 EXE”为空，不能认定所有任务已切换；清理旧包前检查任务中的显式旧路径。
- 桌面工程有未提交修改及新验证报告，均属于工作成果，不纳入清理。
- 三份旧工程副本没有 .git/.plastic 标记；名称、用途与历史构建记录相符。
  本次没有逐文件证明其与主工程完全一致，因此整份清理前应保留日志、配置及检查专用脚本。

## A. 建议清理：合计约 95.976 GiB

| 完整路径 | 大小 GiB | 用途与清理条件 |
|---|---:|---|
| D:/AbyaCliValidation/AbyaPB/ | 32.412 | 9 月 24 日早期构建副本。先保留 Logs、BuildConfigs 和独有验证脚本，再删除副本 |
| D:/AbyaCliValidation/Tests/ | 24.544 | 早期 Unity 测试工程副本。先保留测试结果和专用脚本，再删除副本 |
| D:/AbyaCliValidation/CLI-Migration-20260924/ | 20.406 | 旧 CLI 迁移验证工程。先保留构建/测试记录和配置，再删除副本 |
| D:/AbyaCliValidation/CLI-Player-20260924/ | 0.936 | 缺少新 Bot 接口的旧 Player；任务改用 Bot-Player-20260929 后删除 |
| D:/Unity项目/ABYADesktopDevelopmentTool/src-tauri/target/ | 17.585 | Rust 编译输出；当前运行程序来自 Publish。停止构建/调试进程后可重建，下一次编译会变慢 |
| D:/Unity项目/ABYADesktopDevelopmentTool/dist/ | 0.003 | 前端构建输出，可由 npm run build 重新生成 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/codex-env-20260924-192143/ | 0.018 | 旧环境修复备份；放弃对应旧版本回退时清理 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/sandbox-cli-20260924-211209/ | 0.024 | 旧沙盒 CLI 备份；放弃对应旧版本回退时清理 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/terminal-ui-20260924-223408/ | 0.024 | 旧终端 UI 备份；放弃对应旧版本回退时清理 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/terminal-ui-guide-20260924-224532/ | 0.024 | 旧终端指南备份；放弃对应旧版本回退时清理 |

该总量不重复计算 target/debug 与 target/release，也未扣除将保留的小量归档记录。
其中三份旧工程约 77.36 GiB，Rust target 约 17.58 GiB，是主要清理收益。

## B. 可选清理，暂不放入首批

| 完整路径 | 大小 GiB | 建议 |
|---|---:|---|
| D:/AbyaCliValidation/Bot-Player-Project-20260929/Library/ | 7.144 | 当前新包独立构建副本的缓存。需要更多空间时可删除，未来导入/编译会变慢 |
| D:/AbyaCliValidation/Bot-Player-Project-20260929/ | 20.123 | 新包通过 Demo 测试后，可保存配置/报告再删除整份副本；与上一行二选一，不叠加 |
| D:/AbyaCliValidation/Builds/ABYA-PB Standard IL2CPP-Release R0.8.4 20260924/ | 5.739 | 旧 IL2CPP Release 包，与新 Mono Diagnostic 用途不同；仅确认不需要发布对比或回退后清理 |
| D:/Unity项目/ABYADesktopDevelopmentTool/node_modules/ | 0.128 | 可通过 npm ci 恢复，但收益小，继续开发期间建议保留 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish-Staging-Bot/ | 0.115 | 未运行的 9 月 28 日验证包；它与当前 Publish 二进制哈希不同，先确定保留哪个完整工具版本再处理 |

最新构建副本若整份清理，至少先保留：
- Logs/bot-build-result.json、Logs/bot-preflight-result.json、Logs/bot-editmode.xml。
- Logs/bot-editmode.log、Logs/bot-player-fingerprint.json。
- Logs/AbyaBuildCli 下两次运行的 request/result/unity-result 和必要日志。
- BuildConfigs/bot-windows-diagnostic.json。
建议将这些小文件复制到 D:/AbyaCliValidation/Bot-Player-Readiness-20260929/build-evidence/ 后核对，再删副本。

## C. 必须保留

- D:/AbyaCliValidation/Bot-Player-20260929/：最新游戏包，约 0.940 GiB；不能只保留 EXE。
- D:/Unity项目/ABYADesktopDevelopmentTool/Publish/：当前运行的桌面工具。保留 EXE、CLI、runtime、tools、.codex、.grok 及 manifest。
- D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/bot-pipeline-20260928/：仅约 67 KiB，保留人机流程回退记录。
- D:/AbyaCliValidation/Bot-Player-Readiness-20260929/：最新接口、双端启动、截图与验收记录，仅约 606 KiB。
- D:/AbyaCliValidation/CLI-Smoke-Archive/：约 61.6 MiB，是可复用的测试存档，不是缓存。
- D:/AbyaCliValidation/Bot-Pipeline-20260928/、CLI-Smoke-Evidence/ 和根目录结果 JSON/XML：历史证据很小，建议保留。
- 桌面工程的 .git、src、src-tauri（不含 target）、scripts、skills、.codex、.grok、tools、docs 及配置/锁文件。
- C:/Users/Indiegamespass/ABYA Desktop Development ToolWorkspaces/ 和 D:/Unity项目/ABYA Desktop Development ToolWorkspaces/：用户任务、对话、生成内容。
- C:/Users/Indiegamespass/AppData/Local/ABYA Desktop Development Tool/Data/：设置、任务数据库、连接状态及日志索引。
  不手动删除 app.db、app.db-wal、app.db-shm 或 cli-connection.json；数据库当前由运行中的工具使用。
- C:/Users/Indiegamespass/AppData/LocalLow/ABYA ProductionTeam/ABYA_PB/Data/Archives/：用户游戏存档。

## 执行顺序

1. 核对游戏任务已采用新 Player；记录当前程序/Unity/编译进程，排除仍在使用的路径。
2. 集中保存旧验证副本中的结果、配置和专用脚本；发现未回收的源码修改则先保留该副本。
3. 按 A 清理旧副本、旧 Player 和可重建缓存；备份文件按是否保留回退决定。
4. B 类暂留，待新 Demo 验收或明确不再需要对应构建后处理。
5. 删除时逐项校验真实绝对路径位于本清单目录，使用单一 PowerShell 文件操作，不跟随目录链接。
6. 记录实际删除项和磁盘回收量；不关闭当前任务、不删除用户会话或自动清理其他磁盘目录。

原始容量明细：D:/Unity项目/ABYADesktopDevelopmentTool/logs/cleanup-inventory-20260929.json。
