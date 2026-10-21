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

## Current behavior and limitations

- Automatic insertion defaults on for new users; explicit existing settings are retained. Selection questions display answers without replacing source text. Edits can replace the selection.
- Recording locks the starting field and can return after app switches when its content/caret snapshot remains valid. Opaque targets retain stricter foreground checks; WeChat needs a click in the input field.
- Nonactivating voice bar and answer overlay support dragging and multiple displays. Empty speech and missing microphones use small notices.
- Custom ASR and primary/backup LLM connections support discovery and text connection tests with latency, input and output. No silent custom-to-trial fallback.
- Optional trial activation requires consent and individual credentials. Shared queues can delay output; use your own service for regular work. Current limits are in PRIVACY.md.
- Fn and Command combinations support native key capture or typed entry. Permission state reflects macOS; manual-add guidance is provided for Input Monitoring.
- No Windows/Intel release, Apple notarization or App Store compatibility is claimed. Synthetic tests do not replace fresh-Mac or arbitrary-app acceptance.
