# Abya 首批清理执行结果

日期：2026-09-29。清单 A 的 10 项均已删除，并确认路径不再存在。

- 删除文件逻辑大小：95.976 GiB。
- 保留差异/记录：2656 个文件，共 3.232 GiB；逐文件 SHA-256 校验通过。
- D 盘可用空间净增加：91.095 GiB（已计入归档占用；文件逻辑大小不等于实际占用）。
- 最新游戏 EXE、当前桌面 EXE 和 CLI 的 SHA-256 与删除前一致，CLI --version 正常返回。
- 9 个关键保留路径均存在，未删除清单 B 的可选项、主工程、任务工作区、用户存档或应用数据库。

## 实际删除项

| 路径 | 逻辑大小 GiB | 状态 |
|---|---:|---|
| D:/AbyaCliValidation/AbyaPB | 32.412 | 已删除 |
| D:/AbyaCliValidation/Tests | 24.544 | 已删除 |
| D:/AbyaCliValidation/CLI-Migration-20260924 | 20.406 | 已删除 |
| D:/AbyaCliValidation/CLI-Player-20260924 | 0.936 | 已删除 |
| D:/Unity项目/ABYADesktopDevelopmentTool/src-tauri/target | 17.585 | 已删除 |
| D:/Unity项目/ABYADesktopDevelopmentTool/dist | 0.003 | 已删除 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/codex-env-20260924-192143 | 0.018 | 已删除 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/sandbox-cli-20260924-211209 | 0.024 | 已删除 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/terminal-ui-20260924-223408 | 0.024 | 已删除 |
| D:/Unity项目/ABYADesktopDevelopmentTool/Publish/.backups/terminal-ui-guide-20260924-224532 | 0.024 | 已删除 |

## 归档与说明

归档目录：D:\AbyaCliValidation\Cleanup-Archive-20260929。
manifest.json 记录保留文件原位置、归档位置、大小、哈希及每个删除目标的状态。
README.txt 说明恢复方式；归档是差异与记录集合，不是完整可运行 Unity 工程。

Rust target 和 dist 已删除，下次源码构建需要重新生成；当前运行程序仍来自 Publish。
旧游戏 EXE 路径已失效，测试任务应使用 D:/AbyaCliValidation/Bot-Player-20260929/ABYA_PB.exe。

## 额外健康检查

独立 CLI doctor 返回 unauthorized，因此未确认其认证联通性；此调用未携带托管会话环境。
桌面进程 PID 53272 仍在运行，相关程序哈希保持不变。没有重置凭据、重启服务或中断当前会话。
本次没有删除或写入 AppData 的设置、数据库或连接描述文件，无法据此断定认证问题产生时间。
