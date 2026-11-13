# 新版 PB 试玩启动检查修复

大小记忆G单窗口试玩报“当前运行包缺少验收能力：runtime_get_game_state”。启动记录使用新版 PB 和 G-button5-v11-pb20261113，约5秒后失败清理正常退出（exitCode=0），并未切回旧包。

CLI 连通早于游戏根初始化。基础预检已有就绪等待，App 的 start_task_preview / start_preview 却立即检查能力目录。修复在共享 check_instance 中先等待游戏根，再执行原有能力、存档、关卡、网络、UI 和 Lua 检查。真实故障及未知结果仍失败并清理本次窗口，不重放写入。

完整检查通过：npm run check（252项前端测试及配套 Node 检查）、Rust fmt、Clippy、398项库测试、2项CLI测试；17项专用环境测试未执行。完整便携包 releaseNumber=7。

原生 WebView 调用与按钮相同的 start_task_preview 接口，seats=1：旧 Publish 在隔离滑冰副本复现相同错误；修复包返回 ready，新 PB 指纹 a71182a80d90c140742a02664e38ae034f5db991f0e0db1875c4b434d6826c67，联网房主持续运行约98秒，CLI可用，无内存保护停止，随后显式正常关闭。未向人工试玩实例发送游戏输入，不代表完整游戏或多人验收。隔离夹具将复制任务设为 review 测试同一服务，真实项目阶段和批准未改。

原始回执：F:/AbyaPVP/Maintenance/preflight-20261113/native-qa 下 old-preview.json、fixed-preview.json、inspect-fixed.json。

APP SHA256：d4b4085ddb919a542c3f396eaa983c895076a3d5003833e21640ed8034e5a2df。
CLI SHA256：053237e9a462ef124d86383d76663945bb49f833ea70c91736db83a8707faac0。
源指纹：5c3cf8403e8d6d98332ac1add1f689bb29095d058cefd3e62208b454eef96ea0。
