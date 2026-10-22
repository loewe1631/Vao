# Vao 0.2.2 for Windows — release notes

## 中文

新增“停说后自动结束”，默认关闭。在设置中开启后，可设置 2–30 秒停顿等待时间，默认 5 秒，下一次录音生效。检测到有效语音后才计时；最后约 2 秒显示倒计时，继续说话会重新计时。到时自动结束录音，继续既有识别、整理和输入流程。

主快捷键手动结束、Esc 取消、默认向完成时当前光标输入，以及可选返回录音起始位置保持。Windows 私有临时音频、设备格式兼容和剪贴板保护保持。旧设置缺少新字段时会使用默认值。

语音活动检测使用本机 WebRTC VAD，不为停顿检测上传音频。原有语音识别、文字整理与可选增强仍按服务设置处理数据。未检测到有效语音时不自动结束；检测失败则恢复手动结束。噪声、轻声和麦克风差异可能影响判断，主快捷键和 Esc 可随时使用。

安装包：`Vao-0.2.2-setup.exe`，Windows 10（1809）及更新版本，x64。沿用正式版身份与本地设置。安装前退出 Vao，再按向导覆盖安装；卸载不自动清除设置、历史或凭据。缺少 WebView2 时安装器下载微软官方组件。

此版本尚无 Windows 代码签名。请从正式发布页下载并核对 SHA256；Windows 的 SmartScreen 提示和签名信任不由校验和替代。

本说明随候选源码提供；是否已公开发行及对应验收范围，以正式 GitHub Release 为准。构建检查、安装包完整性检查和 Windows 桌面验收分别记录，不互相替代。

## English

Adds **Stop after a pause**, off by default. Enable it in Settings and choose 2–30 seconds (default 5), effective for the next recording. Local voice detection arms after speech, shows the final two seconds, and resets when speech resumes. The normal transcription, refinement and insertion pipeline handles automatic stopping.

Manual stopping, Escape cancellation, insertion at the completion-time caret and optional return to the starting input are retained. Existing Windows audio formats, private temporary files and clipboard protection remain. Legacy settings receive safe defaults.

Pause detection uses local WebRTC VAD and adds no audio upload. Existing recognition, refinement and optional context follow the selected services. Initial quiet does not submit; detector errors retain manual stopping. Noise, soft speech and device differences can affect detection.

Installer: `Vao-0.2.2-setup.exe`, Windows 10 (1809) or later, x64. Quit Vao before upgrading; the stable application identity and local settings are retained. Uninstalling does not automatically erase settings, history or credentials. The installer downloads Microsoft's WebView2 runtime if needed.

The installer is **not Windows code-signed**. Verify the official source and SHA256. Checksums do not replace signature trust or SmartScreen decisions. This document accompanies candidate source; publication and acceptance status are defined by the actual GitHub Release. CI, package verification and real desktop acceptance are separate evidence.
