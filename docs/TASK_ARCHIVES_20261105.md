# 任务存档入口、运行包与导出修复验证

日期：2026-11-05。独立候选：`Candidates/TaskArchives-20261105`。
正式 Publish 与正在运行的用户 APP 未替换；退出旧 APP 后可打开候选使用。

## 实现

- APP 当前 gameExecutablePath 是所有新托管进程唯一来源，CLI 请求、旧任务版本及 LAN Host 均不能覆盖。
- 初始化/首次开发启动记录完整 Player 指纹。差异通过 APP 显式确认，保留切换历史与旧版本证据，不自动继承通过结果。
- 启动窗口读取本任务私有目录及登记编辑源，用来源+规范路径区分同 GUID 副本；无候选版本的普通任务也支持。
- 桌面启动经过任务桥接，传入经过归属检查的数据根。候选试玩沿用独立、受验证的快照入口。
- 冻结版本可单独完整导出到全局目录：暂存、指纹校验、同卷重命名、读回与持久回执；相同内容幂等，不覆盖同名/同 GUID 冲突。

## 真实 Player 对照

使用本机 APP 设置的 `D:/AbyaBuild/CatTrapClockFix-20261009/ABYA_PB.exe`，未改 PB 源码或换包。
Player 完整指纹：`28cfca7a22dd9cfb5e9c2e1d848b848bc1bcacb8a1634843eede3a5ebdd7f892`。
使用已有“堵猫猫”存档的隔离副本，仅验证入口，不把它算作新游戏制作成果。
任务：`0494ab80-546f-439d-b635-00dda66b3073`。
存档 GUID：`a618b069-6652-4b89-b04a-0117324981be`；关卡：`40cab2b5239d46c2acb198bfbf78f48a`。

1. 首次原生测试发现：Rust 规范路径带 Windows 扩展前缀，指定 Player 返回 `Requested archive path was not found.`，尚未加载。
2. 修复启动参数：去除盘符路径扩展前缀；UNC 转回普通网络路径，内部规范路径校验保留。
3. 同一指定 Player 重测：Editor 与 Offline 均 `phase=ready, success=true, worldReady=true`；存档与关卡 GUID 一致。
4. 原生枚举同时包含同 GUID 全局原件与任务私有副本；运行日志确认加载 `.abya-authoring/<taskId>/Archives/...`。
5. 指定另一个真实 EXE 的启动请求被 `app_player_mismatch` 拒绝。
6. 测试实例和隔离 APP 已关闭，用户原存档及原 APP 保留。

原始本机证据：`logs/task-archive-qa-20261105/evidence/`，包含修复前失败报告、最终两种模式报告、路径日志与 result.json。
可重复脚本：`scripts/task-archive-launch-smoke.mjs <隔离pilot根> <已有真实存档>`；Playwright 路径通过 ABYA_TEST_PLAYWRIGHT 指定。
脚本只复制原存档到新测试任务，不改用户全局存档；要求全局原件可发现，以覆盖同 GUID 副本场景。

## 检查与边界

- npm run check（技能、变更日志、前端/CLI 测试、TypeScript 和构建）通过。
- Rust fmt、Clippy all-targets（deny warnings）通过；362 项库测试及 2 项 CLI 测试通过，14 项显式环境测试按原配置忽略。
- 新增界面测试覆盖私有路径准确传递、显式换包确认、切任务时拒绝过期响应。
- 新增后端测试覆盖包选择、原路径文件变化、目录归属、无版本私有存档、完整导出/幂等/冲突不覆盖。
- 完整候选 553 文件、349945016 字节，清单逐文件哈希核验通过。
- 全局导出使用隔离文件系统回归；没有向用户真实全局目录执行安装。
- 本专项确认存档启动入口，不等同于所有游戏玩法、多人联机或完整自主制作流程全部通过。
