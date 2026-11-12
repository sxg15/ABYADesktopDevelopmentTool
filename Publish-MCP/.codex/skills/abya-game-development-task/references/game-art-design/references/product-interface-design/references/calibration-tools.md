# 排版测量辅助工具

仅在需要精确还原、对照字体/几何偏差时使用。Node 20+，无额外依赖；不启动产品、不操作存档，不读取账号。工具不是美术验收器。

## 输入来自哪里

复制 [模板](../assets/calibration.template.json) 到任务的设计资料目录。空值故意保留，未填写不能通过。删去确实不适用的测量对象，新增本次关键对象；不要只保留容易通过的项。

- `reference` 是已选、可执行设计基准，`actual` 是对应实际版本。图像分别填写原文件相对路径、SHA-256 与原始像素宽高。hash 可由宿主工具或 `Get-FileHash -Algorithm SHA256` 获取；不猜写。
- `context` 的 designVersion/scenario/content/layout 两侧必须相同；scenario 说明镜头、交互状态与取样时刻，content 标识相同文本/数据，layout 指同一布局模式。actual.implementationVersion 填真实运行版本。多种分辨率或模式分别建样本，不用修改标签掩盖内容不同。
- `frame` 是原图中配准后的同一内容区域，像素坐标左上角为原点。仅支持裁切原点与等比例尺度转换；比例不一致应修正配准或采用相应布局基准。界面逻辑重排不使用简单比例换算。
- `fonts` 按文字角色记录实际可用 resourceId、数值字重（如 700）、style、相同 sample 与实测来源 source。resourceId 使用字体文件摘要或可核对的平台资源身份；两侧字段同名不能代替实际字形审阅。
- `measurements` 使用稳定 id、kind、axis、实测像素 value 与来源 source。edge 是原图上的边界坐标，gap 是净间隙，size 是宽或高，glyph-height 是相同样字的可见高度并关联 fontId。不要把字号参数直接填写为 glyph-height。
- `tolerances` 分别填基准像素误差和字面高度百分比。初拟值保持 proposed；只有已在适用设计/验收资料中明确的标准才能写 specified，并填 source 定位。该字段不代表工具核验过人工批准。

## 运行

将下列路径替换为实际 Skill 脚本与任务文件的路径；输出用新版本文件名，已有文件不会被覆盖。

```text
node <此Skill目录>/scripts/compare-calibration.mjs --input <任务目录>/calibration-v1.json --output <任务目录>/comparison-v1.json
```

输入 JSON 最多 2 MiB，每侧测量/字体各最多 500 项；图像文件逐块校验哈希，单个最多 64 MiB，不把大图整体读入内存。文件身份检查只核对字节，不自动识别图像尺寸、边界、文字或真实运行来源；尺寸和观察来源仍须从实际截图/运行检查取得。

输出包含逐项偏差、字体元数据差别、配准/版本/阈值缺口及文件身份检查：

- 退出码 0 / within-tolerance：仅已填且可比的测量在明确阈值内。
- 退出码 1 / differences：已填测量存在超差或字体元数据不同。
- 退出码 2 / unverified：缺证据、身份不符、不同状态/内容、未配准、阈值待定、输入无效或输出文件已存在。

字体不同或样字不同不能仅靠高度接近证明还原；glyph-height 会标为 unverified。整个结果始终是 `visualAcceptance: not-evaluated`。即使退出码 0，也需查看字形、笔画、色彩、材质、交互和动态反馈；不要用它自动填写通过、matched 或用户已认可。
程序比对的是输入测量，不是对截图执行 OCR 或像素识别。未经配准或内容不同的样本不产生可解释的误差值，更不输出全画面还原率。
