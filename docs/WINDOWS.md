# Vao 0.2.6 for Windows

适用于 Windows 10（1809）及更新版本，64 位。安装程序为 **Vao-0.2.6-setup.exe**，包含 Vao 图标与中文、英文安装向导。

## 安装与开始使用

1. 从 [Vao 0.2.6 发布页](https://github.com/loewe1631/Vao/releases/tag/v0.2.6)下载 Windows 安装程序并双击运行，也可使用 [Cloudflare 直接下载](https://updates.vaovao.app/downloads/windows/Vao-0.2.6-build20261024.3018-windows-x64-setup.exe)。首次安装需要网络；如果缺少 Microsoft WebView2，安装程序会下载其官方运行环境。
2. 按向导完成安装，打开 Vao。在设置中检查麦克风访问，选择语音与文字服务；可使用免费体验服务，也可配置自有连接。
3. 点击需要输入文字的文本框，按主快捷键开始录音，说完再按一次。默认主快捷键为 Ctrl+Alt+Space；可在设置中修改。
4. 整理期间保持目标输入框获得焦点。结果完成时，文字粘贴到当前光标位置，不会替你发送消息。无法输入时可从历史复制结果。

本安装包尚无 Windows 代码签名。若系统拦截，请核对下载来源与发布页校验值，再根据系统提示决定是否运行。

Windows 0.2.2、0.2.5 及使用同一安装目录的近期候选可以直接覆盖升级，保留设置、词典、历史、凭据和登录启动选择。曾使用独立身份的早期 Windows 预览版需先退出并卸载该预览版，再安装正式版；两者分别保存设置和凭据，不自动导入旧数据。卸载只清理匹配的登录启动项，不自动清除凭据或历史。

## 软件更新

在“关于 → 软件更新”手动检查并下载，校验完成后确认安装。客户端从 `updates.vaovao.app` 获取 Windows 签名清单和安装包；确认后退出 Vao、更新原安装目录，成功后重新打开。录音、处理或待回填期间不能安装，请先完成或取消当前任务。

没有升级器的旧版本需先手动安装本版一次；便携版也需手动安装。网络错误或尚无清单会明确提示，不表示已是最新版。安装取消或失败后可能需要手动重开 Vao。更多说明见 [Windows 软件更新](WINDOWS_UPDATES.md)。

## 个人纠错

在“词典 → 个人纠错”添加正确写法和多个错误写法，相同正确词自动归组。新安装词库为空。普通听写整理后在本机匹配，命中时逐位置选择“替换并输入”“保留原文并输入”或“取消输入”；关闭自动输入时只保存结果。没有倒计时，不会自动替换或发送消息。确认时请保持目标输入框获得焦点；默认使用确认输入时的当前光标，可选返回起始位置仍需目标校验。

结果及历史可编辑纠正版，只有明确勾选的错误→正确关系才加入词库；中文或多处修改请填写完整词语。翻译、划词和受保护的 URL/邮箱/路径/代码不自动纠错。词库不发送给模型，不增加模型请求，与原词典提示分别管理。支持 JSON 导入预览/冲突选择和全部/所选导出；清历史保留词库，重装不会自动清除。文件可能包含私人词语，分享前请检查。

## 日常功能

- **停说后自动结束**：在设置中开启，默认关闭；等待时间为 2–30 秒，默认 5 秒，下一次录音生效。只有本机 VAD 检测到有效语音后才计时，最后约 2 秒在浮条倒计时；继续说话会清零。到时沿用原有识别、整理和输入流程。未说话时保持等待；检测失败则按原快捷键手动结束。主快捷键手动结束与 Esc 取消始终保留。
- **听写与文字整理**：支持长录音、词典、文字排版和明确反馈形成的个人偏好。
- **局部翻译**：录音中按翻译快捷键切换需要翻译的片段，主快捷键结束整段录音。默认翻译快捷键为 Ctrl+Alt+T。
- **划词处理**：选中文字后按主快捷键，说出与原文相关的问题或修改要求；提问保留原文，修改尝试替换已校验的原选区。
- **随 Windows 登录启动**：在设置中开启后，登录时在托盘待命，主窗口不弹出。单击托盘或再次打开 Vao 可显示主窗口。默认关闭。
- **意图整理与智能补全**：区分听写正文与编辑要求，处理消息、清单及后续修正；智能补全默认开启，尊重已有关闭选择，下一次录音生效。补全可能出错，请核对结果。普通问句不会自动转成问答。

本版已移除临时转写、输入框前后文和扩展窗口上下文增强；保留本机目标和选区保护。旧增强选项不再采集或发送上下文，旧实时转写连接迁为同一服务和模型的文件识别连接。

停顿检测只在本机处理已采集的音频，不增加上传接口或保存音频；结束后的远程识别与整理仍按原服务设置执行。噪声可能被误判为语音、轻声可能未被识别；需要时使用快捷键结束或 Esc 取消。没有检测到语音不代表录音未保存于本次临时缓冲，取消时沿用原清理流程。

普通听写从 A 开始、结果完成时光标停在 B，会输入到 B。请保持光标停在目标位置直到文字出现。如果开启“返回录音起始位置”，应用会尝试返回起始输入框，但部分应用无法恢复原光标。“返回起始位置时使用兼容粘贴”仅在该模式下生效。划词修改始终校验原选区。

自动输入不会按 Enter、提交表单或发送消息，也不会因缺少接收确认而自动重复粘贴。结果保存在历史中；请核对目标内容后再手动复制，避免重复。密码、只读或已失效的输入位置会拒绝写入。

## 设置与卸载

快捷键、词典、偏好和历史保存在本机；密钥保存在 Windows 凭据管理器。关闭主窗口后仍可通过托盘使用；选择“退出”才结束程序。登录启动关闭后，下次登录不再启动。Windows 如果禁用启动项，可从 Vao 设置打开系统启动应用页面调整。

在 Windows“设置 → 应用”中卸载 Vao。卸载会清理本安装路径匹配的登录启动项，但不会自动删除本机历史、设置或凭据。需要删除历史时，请先在应用内操作；凭据可在 Windows 凭据管理器中处理。

开发构建与验证见 [Windows build](WINDOWS_BUILD.md)；许可证和第三方声明随安装包提供。

## English

Download **Vao-0.2.6-setup.exe** from the [Vao release](https://github.com/loewe1631/Vao/releases/tag/v0.2.6) or [Cloudflare](https://updates.vaovao.app/downloads/windows/Vao-0.2.6-build20261024.3018-windows-x64-setup.exe). Requires Windows 10 (1809) or later, 64-bit. The installer downloads the official WebView2 runtime if missing. This build is not Windows code-signed; verify its source and release checksum before deciding whether to run it if Windows blocks it.

About → Software update manually checks and downloads a signed Windows update from `updates.vaovao.app`. Confirm installation after verification; Vao exits, updates the registered installation and reopens after success. Recording, processing and pending insertion block installation. Older versions without an updater and portable copies need a manual install. A cancelled or failed installation may require manually reopening Vao; a failed check never means the app is up to date.

Open Vao, check microphone access and choose hosted or custom speech/text connections. Click a destination text field, press Ctrl+Alt+Space to start, then press it again to stop. Keep the field focused until processing finishes: ordinary dictation pastes at the current caret without sending a message. Ctrl+Alt+T marks translation segments; both shortcuts are configurable. Selection tools answer questions about the selected text or apply a requested edit to the verified selection.

Optional pause stopping is off by default. Enable it in Settings and choose 2–30 seconds (default 5); changes apply to the next recording. Local VAD arms only after speech, shows the final two seconds, and resets the timer when speech resumes. Initial quiet never submits. Detector failure keeps manual stopping available. The main shortcut still stops; Escape cancels. VAD itself does not upload audio; the existing recognition/refinement services receive audio/text after finishing as before.

Sign-in startup is optional and off by default. When enabled, Vao stays hidden in the system tray after login. Open it from the tray or its shortcut to show the window. Windows may separately disable startup in system settings. Uninstalling removes only the startup entry matching this installation path; it does not erase local settings, history or Credential Manager entries.

Windows 0.2.2, 0.2.5 and recent candidates sharing the installation can update in place, retaining data, credentials and the startup choice. Only early preview editions using a separate identity require removal and fresh configuration; they have no automatic data migration. Removing that edition removes its matching startup entry.

Automatic insertion never presses Enter, submits or automatically retries. Unsupported inputs keep results in History for copying. Review the destination before copying again to avoid duplicates. Returning to the starting input is optional and cannot restore the original caret in every app.

Intent-aware refinement handles dictated content, editing instructions, messages, lists and later corrections. Smart completion defaults to on, respects saved choices and applies to the next recording; review inferred content. Ordinary dictated questions are not answered automatically. Temporary transcripts and input/title context enhancement have been removed while local target and selection protection remain. Legacy realtime transcript connections migrate to file recognition with the same service and model.

Personal corrections are managed in Dictionary → Personal corrections. Ordinary dictation is matched locally after refinement. Choose occurrences to replace, keep the text, or cancel; there is no countdown or automatic replacement. With automatic insertion off, choices only save/display the result. Keep the destination focused when confirming: the default uses the current caret, with optional original-target validation unchanged. Only explicitly checked relationships are remembered; editing history never changes another app automatically. The correction library is not sent to models. JSON import previews conflicts; export supports all or selected groups. Clearing history keeps this library.
