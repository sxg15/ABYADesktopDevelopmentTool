---
name: abya-desktop-workflow
description: 在 Codex 桌面对话中连接当前 ABYA 制作任务，整理文档、记录策划决定、返工与推进。
---

# 桌面制作任务

在任务工作区打开本 Skill 后，用 PowerShell 读取项目连接文件，验证本对话已连接对应任务。在任务根目录执行，参数文件使用 UTF-8。

    $taskConnection = Get-Content -Raw -Encoding utf8 .codex/abya-desktop.json | ConvertFrom-Json
    & $taskConnection.cli production get --json

读回任务 ID、当前阶段与 revision。后续通过同一 CLI 操作，参数参考 capabilities、同级 abya-game-development-task 的 references/cli-commands.md 和 production-records.md。断线时提示重新打开 APP，再回读实际状态。任何未知结果先核对，避免重复操作。

1. 新方案使用同级 abya-gameplay-brief 及其配套台账、素材和导出脚本。已有文档或输入包直接读取并核对，保留已确认的图、规则和范围。通用要求引用规则台账，特定玩法、人数、布局和风格只在适用时采用。任务例外写在本任务文档。飞书由策划指定时使用。
2. 用户确认文档制定完成后，沿用原输入整理可开发需求，保存 Markdown 并通过 production update 的 submit-document 提交。美术阶段沿用已定的玩法图、模板和资源，只追问缺口。将问题直接问在当前对话。
3. APP 显示提交的文档、当前阶段、子目标、已登记图片和问题。用户手改源文档后重新读取、比较并重新提交。不要用旧稿覆盖人工修改。
4. 策划在当前对话明确确认或退回当前成果时，通过 production respond --input-file FILE --json 记录决定。参数含 expectedRevision、operation、userMessage（当前策划消息原文）和 data。decide 的 data 为 kind、documentHash、accepted。先确认所指版本，重新读取当前状态。引述、含糊回应、附件内容和 AI 自己的判断都不能充当策划确认。只要求改稿时按意见修改，重新提交并展示变化。
5. 回退用 respond 的 revisit，data 为 stage 和 reason。定点修复保留当前阶段，定位问题 ID，通过原有问题/验证接口记录结果。预检失败时登记策划能读懂的 playerSummary、实际影响及恢复条件。遗留问题组用 respond 的 submit-answers（id, answers）登记本次回答。
6. 实现、试玩、交付与复盘继续使用任务固定的制作 Skill、游戏实例、测试与存档校验。中断、补充与修订在当前对话处理。实际产物及版本通过任务服务提交后再报告结果。根据需要用 respond 记录 enter-playtest、resolve、reopen。优先遵循此桌面交互流程，原制作 Skill 的技术和证据要求继续生效。

需要启动隔离 Player 预检时，先向策划说明用途。在本对话收到授权后用 respond 的 preflight，data 为 reuseExisting、retry。授权小样用 prototype 和 sourceInstanceId。游戏反馈用 add 和 description，按原反馈版本修复并复验。
