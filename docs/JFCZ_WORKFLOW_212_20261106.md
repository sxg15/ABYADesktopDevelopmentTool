# 正式发布工作流 2.12 与见缝插针迁移

用户明确授权先替换正式版、迁移见缝插针并清理旧版本。本次使用上一轮已验证的完整候选，不重新构建或虚构整款游戏通过。

## 正式包

- 入口仍为仓库根的 `ABYA 开发工具.cmd` 与原桌面快捷方式。
- Publish：564 文件、353328949 字节，全清单 SHA-256 校验通过。
- APP：`3fc4fb5f4c6e546b1e2151d4d9738b5ccddc833638c6496382fee9628e196d52`。
- inventory：`8ae02cc003dbde98264df8dd47bc9030a490eb4147f1ae6839691f98d371f1f0`。
- 发布标签：`workflow-unlimited-20261106`；流程版本：2.12.0。

## 项目迁移

- taskId：`4dba22b4-c92b-4b66-8561-116677cafd75`，沿用原任务及会话，没有创建替代任务。
- 原生 `upgrade_task_production` 完成迁移，revision 464 → 465，executionTimePolicy=unlimited。
- 阶段仍为 review，当前作品仍为 `input-362f-v1`；模型保持暂停。
- 4条批准、28条证据、文档、设计记录、反馈、当前版本详情和原有问题状态保留。
- 所有目标 ID 保留，目标变化仅为相关记录版本和更新时间；原 nativeSessionId 保留。
- 编辑存档56文件、冻结候选45文件，迁移前后逐文件哈希一致。
- 实际 Player 沿用 `D:/ABYA验证/Players/archive-bootstrap-20261102/Player-v2/ABYA_PB.exe`。
- 已在正式工作台核对“本任务流程 2.12.0”与原作品、阶段和问题。

## 清理与备份

- 已移除已发布的 `Candidates/Workflow-2.12-20261106` 重复目录；相同完整运行文件已进入正式 Publish。
- 已移入回收站：Publish-Staging-Workflow、旧 logs/publish-backups 下安装包、2026-11-05迁移的 previous-Publish、2026-10-20旧安装备份 ZIP。
- 保留本次回滚包，以及项目工作区、编辑存档和迁移前后原生记录，位于 `D:/ABYA维护/发布与迁移-20261106`。
- 工作区备份包含13264文件、约1.55 GiB；未清理用户游戏数据、历史证据或数据库恢复材料。

## 数据视图核对

常规路径读取最初命中无关旧库。实际项目状态来自通过正式桌面启动链打开的 App 原生接口；未以旧数据库覆盖用户库。
前轮 CatTrapClockFix Player 的初始化失败仅属于旧视图中的运行包，不作为当前见缝插针项目的能力结论。
