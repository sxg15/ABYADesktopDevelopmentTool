# 工具边界

使用当前环境已可用的 Node 20+；没有运行时则说明未验证，不为设计任务安装依赖。
[visual-tools.mjs](../../scripts/visual-tools.mjs) 只读取指定 JSON 并输出结果，不扫描界面、不写项目文件。
从该通用 Skill 根目录运行；输出经检查后才写入项目已有变量体系，不替换整个配置。

## 明确色对的对比度

`node scripts/visual-tools.mjs contrast pairs.json`

输入示例：
```json
{"pairs":[{"name":"主要操作文字","fg":"#ffffff","bg":"#16705b","fontSizePx":16,"fontWeight":600}]}
```

字号为真实 CSS px，不是截图像素。普通文字使用 4.5:1；24px 普通字或至少 14pt
（14×96/72 CSS px）的粗体使用 3:1。特殊字体、中文等还需确认等效大小与实际可读性。
不先四舍五入比值判定。fontWeight 按数值提供，粗体条件为至少 700。
支持不透明 #RGB/#RRGGBB；透明、渐变、图片、变量和缺失字号返回 unverified，
不能按通过处理。动态背景需要实际渲染检查；本工具不覆盖非文字控件和完整可访问性。
退出码：0 声明色对全部通过，1 有失败，2 存在未验证或输入错误。均不代表设计验收。

## 已定义变量的导出

`node scripts/visual-tools.mjs tokens tokens.json css`

输入采用 primitive/semantic 两层，可使用 `{color.primary.500}` 引用。叶值支持
字符串、数值或 `{ "hex": "#16705b" }` / `{ "value": "16px" }`，拒绝丢失及循环引用。
CSS 保留变量引用，方便主题切换；可选 v3 输出用 JSON 转义生成配置片段，避免字体
名称引号破坏语法。v3 是片段，需合入已有配置；不安装 Tailwind，不承诺 v4 导出。
色阶依据现有品牌与参考设计，工具不自动生成“高级配色”或擅自增加阴影等级。
