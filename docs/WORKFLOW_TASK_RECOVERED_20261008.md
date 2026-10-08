# 堵猫咪测试任务恢复记录

日期：2026-10-08。

- 旧 APP 已正常退出，新版 `Publish-Staging-Recovery` 已通过普通 Windows 进程启动。
- 已核对真实数据库中的原任务，沿用任务 ID `094d0123-0808-4e83-a9f9-0f9cfa093397`。
- APP 对话 `1a1dc00b-483c-477a-ba02-f04654a6d04a` 已重新关联原始调研会话 `01a119bc-568b-7c31-81ca-42dece11ef30`。
- 原错误关联对应的另一条原生历史保留；旧活动投影已备份，没有合并或改写原生历史。
- 使用现有 TaskService 升级方法将固定流程从 1.1.0 升至 1.2.0，6 个 Skill 固定记录已更新，两套 provider 资料已核对。
- 人数模式为 multiplayer，记录版本为 3；当前 requirements，没有文档批准或游戏开发自动执行。
- 新 APP 的真实 CLI doctor 和 production get 均成功，读回版本、任务 ID、人数模式和阶段与预期一致。

## 备份

完整数据库备份：
`C:\Users\Indiegamespass\AppData\Local\ABYA Desktop Development Tool\Data\RecoveryBackups\workflow-20261008-1791441947329\app.db`

旧 Skill：任务工作区下
`artifacts/game-development/skill-backups/d0987a47-b3dc-4a4e-be54-0e0d77a8bf39`。

恢复结果：任务工作区下 `artifacts/game-development/recovery-applied-20261008.json`。
关联备份与有来源的“多人”纠正位于原 APP 对话目录。

## 数据目录核对

当前 Codex 打包环境读到的默认 AppData 数据库副本仅含旧“测试”任务；普通 Windows 进程读取同名路径，实际包含“人际测试”和“堵猫咪—工作流测试01”。已通过独立只读查询验证这一差异。
本次操作在普通 Windows 环境调用现有恢复、升级和人数设置服务完成，没有用工作区导出 JSON 重建数据库，也没有修改旧副本。

后续从资源管理器启动新版，进入原任务继续需求梳理即可。需求问题使用 APP 弹窗，三个文档确认节点保持未批准状态。
