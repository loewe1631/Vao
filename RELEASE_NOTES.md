# Vao 0.2.3 · macOS

Mac build20261023.2017 · Apple Silicon · macOS 12+ · source tag `v0.2.3`.
Windows remains at [0.2.2](https://github.com/loewe1631/Vao/releases/tag/v0.2.2); this release updates Mac only.

- 改进口述意图整理、清单排版、口头语清理与数字归一化；智能补全根据输入内容线索分析并补全信息，默认开启，可随时关闭，保留已保存的开关选择。作品名称推断仍可能出错，请核对重要内容。
- 移除整个输入增强区块，包括临时转写、输入框上下文及窗口附近文字参考。保留显式选区、翻译和起始目标/光标回填保护。
- 默认体验服务与自有 Chat 连接使用完整整理流程；共享资源与队列仍有限，效果和速度取决于模型与网络。
- Mac 安装包现采用 Developer ID 签名、Apple 公证并附带票据。首次使用仍需按系统提示授予麦克风、辅助功能等权限。
- “关于 → 软件更新”支持手动检查、下载验签和确认重启安装，保留设置、词典、历史及钥匙串。录音、处理或待回填时不会安装；旧 App 留作回退。
- 从 0.2.2 等没有更新器的版本需先手动安装本版。已装支持更新的本地测试版可通过签名公开清单升级。

## English summary

Improved intent-based dictation and optional clue-based completion, now enabled by default while respecting saved choices. Removed temporary transcription and ambient input/window reference; explicit selections, translation and insertion safeguards remain. The hosted service supports the same complete refinement flow as custom Chat connections. Check inferred names and important facts.

Mac downloads are Developer ID signed, Apple-notarized and stapled. About now offers manual signed updates with verified downloads, an idle-session guard, retained user data and a previous-App backup. Older releases without an updater need a manual first installation. Windows remains unchanged at 0.2.2.

---

# Vao 0.2.2 · macOS & Windows

[统一下载 / Downloads](https://github.com/loewe1631/Vao/releases/tag/v0.2.2)

- **Mac:** build20261022.1302 · Apple Silicon · macOS 12+ · source tag `v0.2.2` · `7d6f9eafe4f1eeac0b652d0c21dcca5b4b8fadae`.
- **Windows x64:** build20261022.3014 · Windows 10 1809+ · source tag `windows-v0.2.2` · `9803072d6ec7cdd69821b0bdd90d76f0cd1037b0`.
- 两个平台均新增可选“停说后自动结束”：默认关闭，可设置 2–30 秒，默认 5 秒；检测到有效语音后才计时，继续说话会清零倒计时。
- 主快捷键仍可手动结束，Escape 仍可取消；停顿检测在本机完成，检测不可用时保留手动结束。
- Windows 保留完成时当前光标输入、可选返回录音起始位置、私有音频临时文件与剪贴板保护；升级正式版保留原身份与设置。详见 [Windows 指南](docs/WINDOWS.md)。
- Mac 包仍为 ad-hoc、未 Apple 公证、非 App Store 包；Windows 安装包尚无代码签名。
- Release 的自动 Source code 对应 Mac 标签；Windows 使用专用 windows-source 归档/清单及其源码标签。Mac 校验文件为 SHA256SUMS，Windows 为 SHA256SUMS-windows。

## English summary

Both platforms add optional silence auto-stop: off by default, 2–30 seconds, default five seconds. Valid speech arms the timer; resumed speech resets it. The main shortcut and Escape remain available. Local VAD adds no audio upload; an unavailable detector keeps manual stopping.

Mac build20261022.1302 and Windows build20261022.3014 share one release page with distinct source tags. Windows retains current-caret insertion, optional return-to-start, private audio files and clipboard guards. Mac downloads remain ad-hoc signed and not Apple-notarized; the Windows installer is not code-signed.

---

# Vao 0.2.1 · macOS & Windows

[统一下载 / Downloads](https://github.com/loewe1631/Vao/releases/tag/v0.2.1)

- **Mac:** build20261021.1202 · tag `v0.2.1` · source `7416ad6aa2193cfc1cb8354545d9d2dd40f17a8c`.
- **Windows x64:** build20261021.3013 · tag `windows-v0.2.1` · source `12b9f4748d0215f40431b8a7e85773db1af89f70`.

## Windows

Windows 10 1809+ x64 现已提供单文件 EXE 安装程序，支持听写、整理、局部翻译、划词、可选临时转写、输入框上下文、词典迁移和排版偏好。普通听写默认在结果完成时输入到当前光标，“返回录音起始位置”为可选项；硬件 Fn 不支持。登录启动默认关闭，开启后在托盘待命。安装包尚无代码签名。早期测试版数据与正式版独立，不自动迁移。

Windows ships as a single EXE installer, with dictation, refinement, translation, selected-text operations, optional previews/context, dictionary transfer and explicit layout preferences. It defaults to current-caret insertion, with optional return to the starting input. Hardware Fn is unsupported. Sign-in startup is optional and hidden in the tray. The installer is not code-signed; early test settings are not automatically migrated. See [Windows guide](docs/WINDOWS.md).

## Mac

Mac direct download · Apple Silicon · macOS 12+. Not Apple-notarized; not an App Store build.

## 修复

- 修复词典导入、导出窗口关闭后，辅助进程未响应并影响其他窗口交互的问题。
- 文件选择器改为单次进程，选取完成或取消后退出并回收；不占用录音和原窗口回填所用的适配进程。
- 保留现有词典与配置，导入仍合并去重。异常退出或无效响应会报告错误，不会被当作用户取消。

## Fixes

- Fixed an unresponsive helper after closing dictionary import/export dialogs, which could disrupt interaction with other windows.
- File dialogs now run in separate, one-shot processes, exit after selection or cancellation, and are reaped by the parent. They no longer occupy the recording/input-target bridge.
- Existing settings and dictionary terms are preserved. Import still merges and deduplicates; process failures and invalid replies are reported rather than treated as cancellation.

All 0.2.0 features remain available. See [testing](docs/TESTING.md), [privacy](PRIVACY.md) and [distribution](docs/DISTRIBUTION.md).

---

# Vao 0.2.0 · build20261021.1200

Mac direct download · Apple Silicon · macOS 12+. Not Apple-notarized; not an App Store build.

## 新增与改进

- 录音期间可选显示临时转写，独立开关，默认关闭；保留最终整理、一次回填。支持兼容的文件识别与显式选择的 OpenAI Realtime 转写服务。
- 可选读取起始输入框前后文及有限窗口参考文字，帮助续写衔接；分别授权开启，默认关闭。参考内容不进入历史或学习。
- 不再统一限制录音为两分钟；长音频按片段识别，保留取消与原窗口回填保护。
- 词典支持 TXT / JSON 导入、合并去重与导出，导出仅含词条。
- 语音和文字连接按当前所选协议测试，显示测试原文、实际返回和耗时。语音测试使用合成短句，不开启麦克风。
- 划词用途聚焦所选文字的编辑、翻译、回复组织及相关理解；无关请求通过小气泡提示，不打开主界面或写入结果。自有文字模型会增加一次用途判断请求。
- 简化模型、增强和词典界面；移除侧栏平台标签，仅显示版本。
- 默认免费共享服务不设使用期限或每日额度；繁忙时仍有资源排队，不保证稳定性和速度。

## Changes

- Optional temporary transcription during recording, bounded original-input/window context, and compatible OpenAI Realtime transcription. Each enhancement is off by default.
- Long recordings use bounded recognition segments instead of a universal two-minute cutoff. Final text retains protected insertion into the original target.
- TXT/JSON dictionary import/export with validation, merge and deduplication; exports contain terms only.
- Protocol-specific speech and text connection tests show the fixed input, actual output and elapsed time. Speech tests do not enable the microphone.
- Selection operations focus on editing, translating, replying and understanding selected text. Custom text connections perform one additional purpose check; unrelated or failed checks stop with a notice.
- Simplified controls and platform-neutral sidebar version. Free shared access has no expiry or daily quota, while queues and physical resource limits remain.

- Updated TLS certificate validation dependencies for RUSTSEC-2026-0382 / RUSTSEC-2026-0381.

## Limits

File-window previews are temporary excerpts, not a complete final transcript, and can add requests/cost. Realtime requires service/model support; file fallback does not prove Realtime compatibility. Model purpose checks can misclassify requests and cannot guarantee prevention of prompt injection. Context is bounded reference data, not permission to read an entire app. Real output quality depends on the selected models and destination support. See [privacy](PRIVACY.md), [testing](docs/TESTING.md) and [distribution](docs/DISTRIBUTION.md).

---

# Vao 0.1.0-beta.5

## Source release · build 20261019.1011

- Added a shared Xcode project and scheme that build the same complete client as the command-line scripts, without maintainer-specific paths or signing identities.
- Documented the shared-source distribution plan and remaining App Store sandbox work. The current Xcode target is a non-sandboxed direct-install build.
- Added Xcode user-state exclusion checks and a Mac CI build of the shared scheme.

- Updated bilingual setup/build/contribution documentation and removed obsolete development-stage descriptions.
- Consolidated UI translations and corrected selection/automatic-insertion guidance.
- Replaced personal diagnostic examples with synthetic questions.
- Added checked source packaging, boundary tests, locked browser-test dependencies and CI checks.
- Bounded trial JSON responses and blocked unfinished/expired results from learning.
- Supported separate Cargo build caches during packaging.

The initial source tag was followed by matching Beta5/build20261019.1011 DMG and ZIP downloads on GitHub Releases and the website. This historical release is superseded by 0.2.0.

## Previous binary release · 0.1.0-beta.4 / build 20261019.1009

- Built-in message, action, plan, technical-record and document presets work before any layout feedback. Content rules and app hints choose the preset automatically.
- Saved per-scene layout preferences remain in effect. Prompt examples match the chosen layout; paragraph preference still separates shared restrictions and diagnostic facts.
- Distinguishes task instructions from observations and generic everyday verbs. Technical records preserve unverified scope, hypotheses and version/error terminology.
- Keeps cancellation information, quoted examples, outside explanations, currency differences and uncertain plans. Single actions keep their attributes together.
- Translation uses an independent prompt; selected-text instructions remain separate from ordinary dictation.
- Narrow output checks repair lost dates or protected quote content once, preserving the source and marking incomplete refinement if repair still fails. Single numbered items are normalized without a manual review step.
- Includes 30 fixed-text replay cases, request-contract tests and an opt-in read-only model replay command. These checks do not replace real speech and cross-app acceptance.

## Historical 0.1.0 behavior and limitations

- Automatic insertion defaults on for new users; explicit existing settings are retained. Selection questions display answers without replacing source text. Edits can replace the selection.
- Recording locks the starting field and can return after app switches when its content/caret snapshot remains valid. Opaque targets retain stricter foreground checks; WeChat needs a click in the input field.
- Nonactivating voice bar and answer overlay support dragging and multiple displays. Empty speech and missing microphones use small notices.
- Custom ASR and primary/backup LLM connections support discovery and text connection tests with latency, input and output. No silent custom-to-trial fallback.
- Optional trial activation requires consent and individual credentials. Shared queues can delay output; use your own service for regular work. Current limits are in PRIVACY.md.
- Fn and Command combinations support native key capture or typed entry. Permission state reflects macOS; manual-add guidance is provided for Input Monitoring.
- No Windows/Intel release, Apple notarization or App Store compatibility is claimed. Synthetic tests do not replace fresh-Mac or arbitrary-app acceptance.
