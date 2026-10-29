# Vao privacy / 数据处理说明

Version 2026-10-24. Applies to Vao 0.2.6 for Mac and Windows.

## 本机保存

录音使用有界队列和仅本用户可读写的临时会话文件；Mac 在打开后移除文件名，Windows 独占打开并在关闭时删除；处理完成、取消或退出后关闭。不保留音频历史，不承诺介质上的安全擦除。文字历史保留最近七天、最多 200 条，可在应用内删除；逻辑删除不保证清除备份或介质残留。词典和明确反馈形成的偏好本机保存，可编辑、暂停、撤销或清空。应用不会将一般键盘输入作为学习材料。

API Key、随机安装标识及体验凭据保存在 macOS 钥匙串或 Windows 凭据管理器，不进入设置导出。词典导入支持 UTF-8 TXT 和 JSON，合并去重；导出文件只包含词条，不包含密钥、历史、偏好或安装标识。导出位置和文件分享由你决定，词条本身也可能包含私人名称。

## Mac 个人纠错本地测试版补充（2026-10-24）

个人纠错词库只在本机匹配，不加入 ASR/LLM 提示词，也不为纠错建议新增网络调用。只有明确保存并勾选记住的词语对应关系才形成记忆；可编辑、停用、删除、撤销。整理完成后命中词库时等待用户选择，再保存纠正版并按原目标保护输入；不自动替换或发送。原始识别、模型整理结果和纠正版分别保存；历史清理也会清理所属纠正版，词库独立保留。

JSON 导入导出只包含正确写法、错误写法与启停状态，不含历史、音频、密钥或安装标识。词语本身可能涉及隐私，请自行保管和分享。此独立预览使用自己的应用身份、数据和钥匙串空间，不接入正式版更新。

The Mac correction preview matches personal phrases locally, without adding them to ASR/LLM prompts or making extra model calls. Only explicitly checked, saved phrase pairs become memory. Suggestions wait for a choice before correction and protected insertion. Original ASR, model output and corrected text are stored separately; History cleanup removes related edits while the library remains. JSON transfers contain phrases and enabled states only. The preview uses separate app, data and Keychain identities and local test-package updates.

## 划词用途判断

划词处理只服务于所选文字的编辑、翻译、回复组织和相关理解。自有文字模型会先收到一次包含选中文字及语音指令的用途判断请求，再在通过后生成结果；这会增加一次模型调用、耗时和可能的费用。默认服务在服务端独立判断。判断失败或请求无关时不生成答案、不替换原文、不复制结果，也不将本次结果记入历史或学习。判断由模型完成，可能误判，不能保证阻止所有提示注入。普通口述和翻译不增加这次判断。

## 免费共享服务

使用默认体验服务前需主动同意。开通向 https://test.vaovao.app 发送随机安装标识及同意版本，不采集硬件序列号。服务保存安装标识的带密钥摘要、凭据摘要、同意记录、撤销状态和请求计数，不保存凭据原文。旧设备表中的期限、额度字段不再用于拒绝请求。为兼容旧客户端，状态接口保留数字型期限和额度占位值；新客户端依据 `unlimited` 标记显示不设期限及每日额度。

服务不设使用期限、每日请求总量、每设备每日请求量或每日开通人数限制。仍需有效凭据，撤销的凭据不能重新开通。截至 2026-10-23，ASR 同时十六路、文字同时十六路、合计最多三十二路，另有最多八个请求短时排队；请求体、单次音频片段、模型上下文和超时等资源边界仍存在；ASR 只在 512 token 的运行预算内选取词典提示，超出的提示不发送给语音模型，不因此拒绝音频；完整词典仍在本机保留并参与文字整理。长录音由客户端分段。服务免费共享，不保证稳定性和响应速度，繁忙时可能排队或暂不可用；建议按需要配置自有云服务或本地模型。

模型请求会发送录音、待整理文字、词典及适用偏好；选区操作还包含所选文字。本Mac候选不采集或发送输入框前后文、窗口附近文字、标题或域名；Windows行为见文末专节。服务在请求期间处理，不保存音频或文字正文，不用于训练。运维日志只含请求编号、路径、状态和耗时；网络设施仍会处理来源地址。设备记录用于鉴权和撤销，使用计数保留约 35 天。新开通不再记录网络摘要；既有按日变化的网络摘要随后续开通清理超过约 35 天的记录。

当前没有服务端自助删除入口，可通过项目 issue 询问处理方式。**不要在 issue 中提交密钥、录音或私人正文**；先请求私密渠道，见 SECURITY.md。重新安装不自动清除钥匙串、Windows 凭据管理器或服务端记录。

## 本机停顿检测

“停说后自动结束”默认关闭。开启后使用本机 WebRTC VAD 判断本次录音的语音与停顿，不为检测新增音频上传或音频历史；既有识别、整理及可选增强仍按所选服务处理数据。

Optional silence auto-stop uses local WebRTC VAD and adds no audio upload or audio history. Existing recognition, refinement and optional enhancements still use the selected services.

## 自有模型

自有连接直接访问你配置的服务，不经 Vao 体验网关，遵循其隐私政策。远程要求 HTTPS，回环服务允许 HTTP。连接失败不静默切到默认服务；只有明确配置的备用文字连接会在有限错误条件下被调用。

点击语音连接测试时，仅向当前所选服务发送内置合成英语短句，不启用麦克风，不发送历史、词典或输入框上下文；文字连接测试仅发送固定改口例句。两者均使用所选协议、可能产生服务用量，不改变已保存连接，测试报告仅在当前应用会话显示。

## Mac录音处理和剪贴板

麦克风权限用于录音；在 Mac 上，辅助功能用于快捷键、选区读取、原输入位置校验与回填。Mac 目标保护会临时读取本次捕获输入框的内容，计算仅存于适配进程内存的指纹，不发送该指纹。Mac 录音中切应用后仍尝试返回起始位置；内容或光标冲突、原窗口失效时保留结果并浮层提示。应用只输入，不替你发送消息。安装新版或更换签名后可能需重新授权。

Mac 0.2.3 移除了临时转写、输入框前后文和窗口附近文字参考。录音期间不发起临时识别请求；停止后发送完整录音或长录音的最终分段进行识别，再整理文字。旧增强开关不再生效，下一次保存设置时清理；旧实时协议连接保留地址和模型，迁移为文件识别。目标字段和光标快照仍只用于本地回填保护，显式选区操作仍发送所选文字。取消可阻止后续请求和使用晚到结果，不能收回已经发送的数据。


Mac 选区读取和回填可能临时使用剪贴板，尽量保留多格式内容，并仅在剪贴板未被其他程序更新时恢复。回填内容带临时、隐藏标记，但不能保证所有第三方剪贴板管理器都遵守。

## English

Audio uses a bounded queue and an owner-only session file, unlinked immediately on Mac or exclusively opened with deletion on close on Windows. Files close after processing, cancellation or exit. No audio history is retained. Text history lasts seven days, up to 200 results. Deletion is logical, without guarantees about backups or media erasure. Vocabulary and explicit layout feedback stay locally. Keys and installation credentials stay in macOS Keychain or Windows Credential Manager. Dictionary TXT/JSON exports contain terms only, without keys, history, preferences or installation identifiers; terms may themselves be private.

Optional hosted access requires consent and sends a random installation secret and notice version, not hardware identifiers. The service stores keyed installation hashes, credential hashes, consent, revocation and request counters. No expiry, daily use quota or daily enrollment quota is enforced, including for existing credentials; revocations remain effective. Numeric expiry/quota placeholders remain solely for legacy-client compatibility, with an `unlimited` flag for new clients. Resource bounds remain: as of 2026-10-23, sixteen ASR slots, sixteen text slots and thirty-two total active requests, up to eight queued requests, bounded request bodies/audio slices, model context, an ASR hint budget of 512 tokens (longer hints are reduced without rejecting audio; saved vocabulary still accompanies text refinement), and timeouts. Free shared service does not guarantee stability or response times. Consider your own cloud/local models for regular use.

Model requests send audio, current text, vocabulary, relevant preferences, selected text when applicable and Windows context when enabled. Audio/text bodies are processed temporarily, not retained or used for training. Operational logs exclude content; infrastructure handles network addresses. Usage counters last about 35 days. New enrollments no longer store source-network digests; old digests older than about 35 days are cleaned during subsequent enrollments. Device records support authentication and revocation. Reinstallation does not erase Keychain, Credential Manager or server records. For record-handling questions, request a private channel through the project; never post credentials or private content in issues.

Custom connections go directly to your provider under its policy, without silent fallback to hosted access. A configured backup text connection handles only limited failures. On Mac, Microphone and Accessibility support recording, shortcuts, selection and original-target insertion; messages are never automatically sent. Input fingerprints remain native memory and are not sent to models.



Mac 0.2.3 removes temporary transcription and ambient input/window reference. No preview recognition runs while recording. Stopping sends the complete recording, or final chunks of a long recording, for file transcription and refinement. Old opt-ins are inert and removed on the next settings save; old Realtime configurations keep their URL/model and migrate to file transcription. Input/caret snapshots remain local insertion safeguards. Explicit selection operations still send the selected text. Windows behavior is documented separately below.

On Mac, selection and insertion may temporarily use the clipboard. Multiple formats are preserved where possible and restored only if another app has not updated it. Transient/concealed markers are advisory; third-party clipboard tools may ignore them.

Connection diagnostics send only fixed synthetic speech or a fixed correction sentence to the selected provider/protocol, using its service allowance. They do not enable the microphone, include history/dictionary/context, or change saved connections. Reports remain in the current app session only.

## Windows 输入与本机存储 / Windows input and local storage

正式 Windows 版本与早期测试版分别保存设置和凭据，不自动迁移数据。密钥、随机安装标识和体验凭据保存在 Windows 凭据管理器；卸载不会自动清除凭据或本机历史。麦克风访问检查只短暂打开设备并丢弃样本，不请求模型。长录音使用仅所有者可访问、独占打开且关闭自动删除的临时文件。

普通听写默认在结果完成时，使用剪贴板和一次 Ctrl+V 输入到当前光标位置。处理期间请保持目标输入框获得焦点；不会返回录音起始应用。不会按 Enter、提交或自动重发。无法输入时保留结果；未取得完整接收确认时保留粘贴文字供核对。剪贴板仅备份有界、可安全复制的内存格式；无法安全处理的格式会在写入前拒绝。只有取得严格文本/光标确认且剪贴板序号、事务标记均未变化时才尝试恢复旧内容；用户之后的复制优先。

“返回录音起始位置”默认关闭。开启后校验起始进程、字段、内容和光标；其兼容粘贴选项默认开启，仅在该模式下生效。部分应用无法恢复原光标，请在结果完成前回到原输入位置。划词修改始终校验原选区。完整字段快照仅留有界内存，不落日志或历史。显式复制结果也会更新剪贴板。

Windows 0.2.5 起已移除临时转写及输入框/窗口参考增强，不再通过这些开关采集或发送上下文。原有目标、光标与选区快照仅用于本机输入保护；显式选区处理仍发送所选文字。

The Windows release stores settings and credentials separately from early test editions, without automatic migration. Keys and installation credentials stay in Windows Credential Manager. Uninstalling does not automatically erase them or local history. Microphone access checks discard samples and contact no model. Long recordings use owner-restricted, exclusively opened temporary files deleted on close.

Ordinary dictation defaults to the clipboard plus one Ctrl+V at the current caret when the result is ready. Keep the destination focused during processing; this mode does not return to the recording's starting app. It never presses Enter, submits or automatically retries. Unavailable inputs keep the result; unconfirmed receipt keeps paste text for inspection. Only bounded safely copied memory-backed clipboard formats are supported. Old contents are restored only after strict text/caret receipt and unchanged clipboard sequence/transaction marker. Later user copies win.

Returning to the starting input is off by default. If enabled, it verifies the original process, field, contents and caret. Its compatibility paste option defaults to on and applies only in this mode. Some apps cannot restore the original caret; return there before processing finishes. Selection edits always verify their original selection. Full-field snapshots stay in bounded memory, outside logs and history. Explicit copying also updates the clipboard.

Windows 0.2.5 and later remove temporary transcription and ambient input/window enhancement. Target/caret/selection snapshots remain local insertion safeguards; explicit selection processing still sends selected text.

## Mac 软件更新 / Mac software updates

在“关于”中主动点击检查更新时，Vao 会连接 updates.vaovao.app 的 Mac 平台公开发布清单；点击下载后再从同域请求对应更新包。请求不包含录音、转写、词典、历史、模型密钥或体验凭据；网络服务仍会处理来源 IP、请求路径和通用更新客户端标识。不在后台自动检查或下载，未提供有效清单时明确显示无法检查。更新前校验签名、包完整性与应用身份；应用数据和钥匙串不随 App 替换，旧 App 留在安装目录旁的隐藏备份目录。

When you choose Check for updates in About, Vao requests the public Mac release manifest from updates.vaovao.app. Downloading requests the selected archive on the same domain. These requests contain no audio, transcripts, vocabulary, history, model keys or trial credentials. Network services still process the source IP, request path and a generic updater user agent. There are no background checks or downloads. Updates verify the signed release and archive before replacing the App; app data and Keychain remain separate, and the previous App is retained in a hidden directory beside the installation.

## Windows 0.2.6 个人纠错与更新 / Windows corrections and updates

个人纠错只在普通听写整理完成后本机匹配，不向模型发送词库，也不增加请求；只有明确选择记忆的关系进入本机词库。历史纠正版与原结果分开保存，清历史保留词库。JSON 导出只含纠错组和别名及格式元数据，不含密钥、录音或历史；词语可能私人，由你决定分享。

Windows“关于 → 软件更新”主动访问 updates.vaovao.app 的 Windows 清单，下载经同源限定的安装器并验签、校验大小及 SHA256；不后台检查/下载，不发送录音、正文、词典、纠错库、模型密钥或体验凭据。网络设施处理来源 IP、路径及通用客户端标识。确认后使用原注册目录更新并在成功后重开，保留数据与 Windows 凭据。取消或失败可能需手动重开，不承诺 Mac 式自动回滚；更新清单签名不等于安装器已有 Authenticode 签名。

Windows personal corrections match locally after ordinary dictation. The library is not sent to models; only explicitly selected relationships are remembered. Corrected history is separate from the original, and clearing history keeps the library. JSON exports contain correction groups/aliases and format metadata, without keys, audio or history; the terms themselves may be private. Windows updates are manually requested from updates.vaovao.app without private content or credentials. Verified installation uses the registered directory and reopens on success; a cancelled/failed installation may need manual reopening. Signed metadata does not make the installer Authenticode-signed.

## Mac window compatibility

When automatic insertion is enabled and Vao is idle, Mac app activation notifications may prepare the foreground application's accessibility interface. This step checks application identity and interface availability; it does not read input text, selected text, window titles or clipboard contents. Recording, processing and pending input sessions keep their existing destination safeguards.

Mac 自动输入开启且 Vao 空闲时，会在前台应用激活后准备其辅助功能接口。该步骤只检查应用身份和接口可用性，不读取输入正文、选中文字、窗口标题或剪贴板内容。录音、处理及待输入期间沿用原有目标保护。
