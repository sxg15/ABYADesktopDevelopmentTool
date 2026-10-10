# 人工验收与反馈迭代（流程 2.0）

当前流程没有固定 8＋4 或最低轮数。完成 Lua 实现及必要自测后，尽早交给策划试玩。
已固定旧流程的任务仍以 production get 返回的 policy 为准；升级前不伪造或跳过旧门槛。

## 首次可试玩

完成确认需求、实际素材与核心循环；必要检查有真实证据，存档已经保存并冷加载。
使用 production version 登记实际存档和 Player，必要时 configure-acceptance 指定席位与必需工具。
production start-acceptance 打开同版本实例，production acceptance-status 查询进展。
网络连接、玩法初始化、人工验收是不同事实，不将启动成功写成已验收。
交给策划后停止自动输入，不在用户操作的窗口里继续脚本测试。

## 处理反馈

先读 record.feedback 的未关闭项、version、candidateVersion、artifactId、attachmentIds、instanceIds。
关联图像须实际查看；复现用户行为，明确预期、实际结果与影响范围。
通过 update-feedback 标记 in-progress。完成修改、实际复验并登记当前版本证据后，
标记 awaiting-recheck，填写 fix、recheck、evidenceIds。只能由用户在 APP 关闭或重开反馈。
新增规则或扩大范围修订需求/计划；错字、字号、局部反馈不清空整个阶段。

## 必要自测与证据复用

save-self-test 按实际记录当前版本的 architecture、lua、core-loop、input、visual、
lifecycle、save-reload、authority。单人 authority 可说明不适用，其余缺环境就写 blocked。
每项写具体观察与 evidenceIds；这些记录仅校验材料一致性，不代替运行与视觉检查。
新版本对受影响行为及依赖复验。对确认未受影响的旧证据，用 reuse-evidence
提供 sourceId、新 id、reason、unaffectedScope，再由当前版本自测引用。
复用保留原文件与摘要，并明确“不是本版本重新执行”，不能把旧失败或旧周期冒充通过。
基础时钟、共享状态、存档结构等修改应扩大回归；不为小改动反复跑整个游戏。
旧失败已有修复证据后更新当前结论，历史仍保留。

## 取证、交付与停止条件

围绕具体问题采集状态、日志、截图或动作视频；取消 R0/R8/R12 固定整轮录制。
仍须以真实输入证明关键链路，语义事件不能冒充硬件鼠标点击，状态注入不能冒充真实操作。
只在表现、时序或用户要求需要时录制，遵守 [录像契约](recording.md)，实际看过才登记审阅。
达到确认范围、必要检查有效且策划反馈关闭后提交交付候选，等待接受当前版本。
策划未接受前不自行完成任务；已授权的普通修复连续进行，不逐项询问是否继续。
