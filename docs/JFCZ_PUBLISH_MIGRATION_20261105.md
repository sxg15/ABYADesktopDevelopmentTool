# 见缝插针项目接入最新 Publish

用户授权：将“2.11 隔离实测 · 见缝插针 v2.0 · 1”迁入最新正式 Publish，并清理旧文件。

正式 Publish 已替换为已通过完整检查的 ConcreteIssues-20261105 包，APP SHA-256：
`09048ec6fa753bdc4a180b9e166cd9ff714f1632691a0795859aca340a8d4aa0`。
完整 553 文件、352349485 字节校验通过，保留原桌面快捷方式，已重新打开并查看真实工作台。

实际项目早已位于 D 盘正式工作区，无须移动或重建存档：

- taskId：4dba22b4-c92b-4b66-8561-116677cafd75。
- revision：463；currentVersion：input-362f-v1；阶段：review。
- 原会话 ID 与 nativeSessionId 保留；没有自动继续模型执行。
- 批准、文档、问题、证据与 versionDetails 逐项比对一致。
- 编辑数据根 56 文件、当前冻结版本 45 文件逐文件 SHA-256 与迁移前备份一致。
- APP 当前指定 Player 保持 D:/ABYA验证/Players/archive-bootstrap-20261102/Player-v2/ABYA_PB.exe。

普通路径读取受文件系统视图影响，最初读到的是无关旧库。最终从运行实例实际打开的文件句柄取得数据库快照，quick_check=ok，升级后同方式读回验证；未用旧备份覆盖生产数据库。

备份及验证材料保留于 `D:/ABYA维护/迁移见缝插针-20261105`：迁移前有效数据库 live-zero、项目 task-workspace-before、编辑源 authoring-before、原包 previous-Publish，以及 migration-result.json、file-verification.json、migrated-app.png。

已清理 TaskArchives-20261105、Preflight-20261105、ConcreteIssues-20261105 三份旧候选目录。额外删除 staging 副本及诊断临时数据库的命令被自动审核以 blocked by policy 拦截，未执行；这些剩余文件保留，不宣称全部旧文件已清空。
本次验证项目接续与文件完整性，不改变游戏验收结论。
