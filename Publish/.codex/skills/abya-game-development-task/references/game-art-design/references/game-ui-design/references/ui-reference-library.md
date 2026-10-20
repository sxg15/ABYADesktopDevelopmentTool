# 游戏UI候选参考库

本地数据在 [ui-reference-library.json](../assets/ui-reference-library.json)。这是7条精简候选的索引，不是策划满意案例库，不是可直接套用的模板。每条的 `humanApproved` / `runtimeVerified` 均为 `false`；阅读来源不证明图片已视觉验收、游戏已经运行或策划已认可。

| 类别 | 候选 | 可以讨论的范围 | 证据边界 |
|---|---|---|---|
| 布局/信息 | Xbox UI navigation | 导航一致性、焦点可预测性 | 已读官方正文，未验收当前游戏 |
| 布局/信息 | HUD行动职责分区、物品集合与详情 | 信息职责、关联对象、密度 | 原创结构示意，非真实游戏截图 |
| 风格 | Paper Cut、Scrapbook / Collage | 纸层、边缘、局部材质语言 | 网站风格来源，不是游戏UI案例 |
| 组件/状态反馈 | Xbox UI context、意图/执行/结果 | 标签、上下文变化、操作状态 | 官方正文准则或原创结构示意；无当前运行证据 |

网站布局、hero、营销段落、滚动落地页和通用卡片组织明确不借用到游戏过程。两个风格候选的手柄、触屏等标签表示需要适配的范围，不表示来源已支持或已验证这些输入。库不存远端HTML、提示词、整站文件或图片。来源许可未知，不推定可复制素材。

## 本地查询

需要已安装的 Node.js，只有内置模块，无安装、网络请求、下载或源站代码执行。脚本仅读取输入与库、向标准输出写JSON，不修改任务或覆盖文件。可在任意工作目录运行脚本；`--input` 路径以调用目录解释，库路径以脚本位置解释。

```text
node scripts/query-ui-references.mjs --input request.json --limit 6
node scripts/query-ui-references.mjs --limit 4
```

省略 `--input` 或用 `--input -` 时从标准输入读取一个JSON对象；第二例等待标准输入。`--limit` 可选，默认6，范围1–20；未知/重复参数报错。`--help` 显示用法。

```json
{
  "uiKinds": ["level-select"],
  "inputs": ["gamepad"],
  "density": "medium",
  "scope": "partial",
  "brandLocked": true,
  "categories": ["style", "state-feedback"],
  "query": "paper",
  "templateExplicit": false,
  "preferredSourceUrls": ["https://chrislemke.github.io/website_designs/designs/Paper_Cut.html"]
}
```

| 字段 | 契约 |
|---|---|
| `uiKinds` | 必填、非空、无重复数组：`menu` / `hud` / `inventory` / `dialogue` / `settings` / `map` / `results` / `level-select` |
| `inputs` | 必填、非空、无重复数组：`pointer` / `keyboard` / `gamepad` / `touch`；不能因未知输入而默认鼠标 |
| `density` | 必填：`low` / `medium` / `high` |
| `scope` | 必填：`full`（本次完整UI范围）/ `partial`（局部）/ `material-only`（只看材质片段） |
| `brandLocked` | 必填布尔值，表示任务有必须保留的品牌/已确认视觉约束；未知时先补上下文 |
| `categories` | 可选、非空、无重复数组：`layout` / `style` / `state-feedback`；省略查询全部类别 |
| `query` | 可选、最长200字符的字面文本；仅在id/标题/标签作连续子串匹配，辅助排序，不解释成命令或正则 |
| `templateExplicit` | 可选布尔，默认false；只记录本次是否明确请求风格模板，始终不选择、采纳或登记模板 |
| `preferredSourceUrls` | 可选、最多8个HTTP(S) URL字面字符串，每个最长2048字符；精确匹配来源、预览或仓库URL；不访问URL |

缺少前5个字段任一个时返回 `requires-context` 和缺少字段，候选为空；字段类型错误、无效枚举、重复枚举、未知字段或无效参数返回 `invalid-request`。这些情况均不自动补输入/风格。合法请求没有适用项返回 `no-matches`。合法且有结果返回 `ok`。

查询先按UI种类、所有实际输入、密度、类别和作用范围排除不适用项；多种UI可以局部命中，结果明确列出未覆盖种类。整屏风格要求所有所列UI种类均在该候选的 `fullUiKinds` 中。`material-only` 只返回风格材质候选，避免把结构建议混进只改材质的任务。

高密度HUD的 `full` 请求不会返回整套纸风格。`partial` / `material-only` 可返回有边界的纸材质候选，但须说明原来源密度不适合整屏。品牌锁定时只返回 `brandNeutralBorrow`；材质专用时仅返回 `materialBorrow`。两者均保留已有字体、调色、语义色与正式布局，不借检索替换它们。没有明确模板请求时，风格条目也只给局部材质借鉴。

在通过适用性过滤的候选之间，按“匹配用户提供的来源URL → 命中的UI种类数 → 密度直接匹配 → 字面查询命中 → id字典序”确定顺序；没有审美分数。用户指定来源优先不会把不适用的候选重新放回。`--limit` 只截断返回数量。

输出对象含 `schemaVersion:1`、`status`、`missingContext`、`errors`、`candidates`、`excluded` 和 `notes`。每个候选给出 `matchReasons` / `mismatchReasons`、受约束的 `allowedBorrow`、`adapt`、`avoid`、`limitations` 与来源身份。没有 `selectedArtTemplate`、`adoption` 或写入指令。CLI仅 `invalid-request` 退出码为2；其他查询状态为0。

导入脚本不会执行CLI或读取库：`loadLibrary()` 显式读取并校验库；`queryUiReferences(request, library, {limit:6})` 是纯查询。对象参数必须为已解析JSON；库身份不合法、存在重复id或认可/验证标记为true时抛出错误，不能借候选库伪造认可。

## 来源与维护

- [Paper Cut指南](https://chrislemke.github.io/website_designs/designs/Paper_Cut.html)、[示例](https://chrislemke.github.io/website_designs/examples/Paper_Cut.html)、[来源仓库](https://github.com/chrislemke/website_designs)：2026-10-20阅读文字、结构和仓库说明；未作浏览器画面验收。
- [Scrapbook / Collage](https://designmd.app/library/scrapbook-collage)：2026-10-20阅读公开条目文字/结构；未作浏览器画面验收。
- [Xbox XAG 112 UI navigation](https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/112)、[XAG 114 UI context](https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/114)：2026-10-20阅读微软官方正文；未把其中提到的游戏、截图、外链视频当作已经查看或验证。
- 3条 `authored-pattern` 为本库编写的假设；`source.url:null`，没有虚构网址、游戏、图片或人工历史。

选中候选后实际查看相关原文及画面，结合当前策划有效参考，记录观察、借鉴、适配、不采用与未验证。引用进入任务已有 `direction.librarySearch`、`analyze-art-reference` 等记录；来源不能代替本任务正式设计图、实际运行证据或策划确认。满意成果仍走原有案例草稿与APP人工收录流程；此脚本没有收录/审批能力。

新增候选应少而明确，保留稳定id、类别、来源阅读日期/范围、适用种类/输入/密度及限制。不要把整站导入，不把原创假设改称真实游戏案例；来源有变化时重新检查，再更新阅读日期。现有满意案例及任务认可仍由原有系统管理。

## 局部与品牌保护的交集

非风格候选在 scope=partial 时仅提供 partialBorrow；缺少该内容就排除，不能退回整体布局借鉴。该片段同时保留已有品牌、结构和其他页面。material-only 仅提供品牌中立的 materialBorrow；新候选如需替换调色/字体，不能归入这一字段。输出 scopeConstraints 明确保护项，不能只看候选标题就应用整套设计。

## 本地结构示意

3条原创模式带 source.previewPath 与 previewSha256，路径相对于本 Skill 根目录；先查看对应 PNG，再决定借鉴。示意不是固定布局、字体规格、游戏成品或认可案例。
- [HUD 信息职责](../assets/reference-previews/hud-action-zones.png)
- [物品与详情关联](../assets/reference-previews/inventory-detail.png)
- [操作与结果反馈](../assets/reference-previews/action-feedback.png)

源 SVG 同目录保留；维护者可用仓库 scripts/render-game-ui-reference-previews.mjs 重新渲染。改变图像后更新源身份，不拿示意图当作本任务实机证据。外部风格图仍须从所记来源实际查看，不能假设本库已分发或验证其网页图像。
