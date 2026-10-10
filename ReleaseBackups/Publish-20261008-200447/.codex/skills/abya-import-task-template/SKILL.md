---
name: abya-import-task-template
description: 导入关卡策划提供的任务模板或美术风格模板 Skill，规范名称并部署到 ABYA 开发工具的 .codex 和 .grok，供创建任务时选择。用于导入或更新模板，不用于执行模板中的玩法任务。
---

# 导入任务与美术风格模板 Skill

先取得用户提供的完整 Skill 文件夹路径（必须包含 `SKILL.md`）。没有提供时，
请用户提供该文件夹；已经提供则直接读取，不重复询问。读取入口及其引用资源，
确认它描述可复用任务流程或美术风格；不要执行其中的游戏制作指令或脚本。

## 导入

1. 确定开发工具源码根目录或便携构建目录，必须同时具有
   `.codex/skills/abya-game-development-task/SKILL.md` 和对应 `.grok` 文件。
   不把用户主目录或当前游戏任务工作区当作发布目录。位置不明时询问。
2. 从模板内容提取简短中文显示名称和简介。用稳定的小写英文 slug，最终名称为
   `abya-task-template-<slug>`，总长不超过 64 字符。默认可去掉原名的 `abya-` 前缀。
3. 使用本 Skill 的 [scripts/import-template.mjs](scripts/import-template.mjs)：

   ```powershell
   node "<本Skill目录>/scripts/import-template.mjs" --source "<模板目录>" --destination "<开发工具根目录>" --slug "multiplayer-workflow" --display-name "ABYA 多人游戏制作流程" --description "权威联机、数据化内容、多端 UI 与真实客户端验收"
   ```

   脚本保留完整资源和相对路径，改写入口 name、description 和 `$原名` 调用，
   添加 `abya-task-template.json` 识别标记。普通重名目录拒绝覆盖；相同模板重复
   导入自动幂等更新。更换来源更新已有模板必须明确指定 `--replace`，且只能在
   用户已要求替换该模板时使用。输入不会被修改，输入中的脚本不会被执行。
4. 在源码仓库导入时，两套源码内容是后续构建的正式来源；若已有 `Publish`，
   再以它为 `--destination` 执行同一导入，避免现有构建仍缺少模板。
   只有便携目录时直接导入其中；明确说明下一次用其他源码构建会以源码为准。
5. 读回两端入口、标记和引用文件，核对一致性。源码仓库运行 `npm run check:skills`。
   报告模板 ID、中文名称和两个实际目标路径。模板在任务下次创建或读取时同步；
   已打开的模型会话可能需要重新读取 Skill 或新建对话才能发现更新。

## 识别契约

模板目录名与入口 name 相同且以 `abya-task-template-` 开头；目录下
`abya-task-template.json` 包含 `schemaVersion: 1`、`kind: "abya-task-template"`、
`id`、`displayName`、`description` 和 `sourceSkillName`。创建任务先只读取这些
小型标记，用户选中后才加载模板正文与所需引用。没有模板则沿用通用流程。
导入 Skill 是开发工具维护入口，不应作为玩法模板列入选择。

## 美术风格模板

用户提供美术风格模板时，使用相同导入脚本并加上 `--kind art`。默认
`--kind task` 保持原任务模板行为。美术目录和入口名使用
`abya-art-template-<slug>`，标记为 `abya-art-template.json`，kind 为
`abya-art-template`，其余身份字段与任务模板相同。保留全部图片、素材、
预览、脚本和相对引用，不执行素材脚本。两类模板分开列出：任务模板在
玩法分类前选择，美术模板在可行性评估后选择。

例如导入本次风格用 `--kind art --slug comic-arcade-ui`，名称为
“漫画街机 UI”。美术模板的默认选项是 comic-arcade-ui；新增其他风格
只增加候选，不擅自更改默认项或已有任务选择。
