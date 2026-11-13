# 可复用美术、资产与表现配方

用现有 art list/get 按玩法、动作和用途检索；builtin.* 是随包原创配方，包含 Web 预览与源码，未声明原生通过。
inspect-art-case 将媒体与资源复制到任务 artifacts/art-library/<id>/<version>/；resources.json 记录任务内文件位置。
查阅不会自动采用或批准。借鉴后用 use-art-case 记录 borrow/adapt/avoid，在游戏副本中修改并验证。

save-art-case-draft 可增加 resources 对象，schema=abya.art-resources/v1。
kind 为 asset/effect/interaction/native-guide；license、compatibility、lifecycle、integration、verification 为非空说明。
parameters 为参数定义对象；requiredCapabilities 和 tests 为数组，列出接入能力及生命周期用例。
files 每项包含 name（包内相对路径）、path（当前任务实际文件）、sha256、role，以及 dependencies（清单内 name 数组）。
文件必须真实存在且指纹匹配，名称不能越界或大小写冲突。源码和素材只作为文件保存，不在入库时执行。
案例草稿仍须当前正式媒体和设计上下文；收录由策划记录认可范围。资源与配方随同案例导入、导出和版本冻结。

原生接入沿用当前 Abya 的 CustomUI/Lua/ParticleEffect 契约。缺少引擎 CLI 能力时登记具体缺口，继续可独立实现的部分。
一次性、持续、取消、重复触发、销毁、输入恢复、保存重载与适用的逐端表现须实际复验。
Web 运行测试、原生运行验证、策划认可分别记录；源码、演示和文件存在不能代替任何一项。
