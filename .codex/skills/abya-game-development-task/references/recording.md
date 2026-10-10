# 实际运行录像

只在需要完整循环、巡检或动作录像时读取。先用 recording tools 确认随包组件可用。
录制目标必须是当前任务正在运行的受管实例；使用 PID 对应的游戏窗口，不录整个桌面。

1. 确认已保存、加载当前版本；必要时先 production version。停止无关自动输入。
2. recording start --input-file <JSON> --json：参数 instanceId，可选 maxSeconds（5—7200，默认3600）。
3. 记录返回 id、sourceVersion 和相对 path；运行真实输入完成目标过程。
4. recording get 读取状态和帧数；失败或黑屏须处理，帧数增长不代表画面正确。
5. recording stop 正常结束，只有 completed 且画面经检查的内容才作为完成证据。

当前方式是 Windows 窗口实时录屏、15 fps、H.264 MP4，不含音频。录制期间会临时显示
游戏窗口，不抢焦点，结束后恢复原可见状态。不要遮挡、最小化或调整录制窗口尺寸。
后台离屏窗口的 GDI 录制可能产生黑屏，不能只看文件存在就接受。
开始后检查代表帧，结束后检查开头、结尾、关键动作及连续过程；记录实际审阅范围。
可以用随包 tools/ffmpeg/runtime/bin/ffmpeg.exe 从视频提取查看帧；提取帧不改变原始录像
的来源，单纯采帧另标 frames，不称完整实时录屏。未实际听验的音效另列未覆盖或人工验收。

metadata_path 指向本次 recording.json，记录窗口所属任务/实例、开始结束、版本与状态。
最大时长到达或实例停止会结束录制；异常/中断文件保留，但不当作完整循环通过依据。
APP 重启后的旧录像与元数据仍在任务目录，不能凭原 recording 状态宣称已正常完成。
实际录像的 path 可提交 register-evidence，kind=video，按目的选 captureType，
经审阅后才写 reviewed=true。关联状态断言和截图，录像不能独自证明规则或同步正确。
