# Vao privacy / 数据处理说明

Version 2026-10-22. Applies to Vao 0.2.2 for Mac and 0.2.1 for Windows.

## 本机保存

录音使用有界队列和仅本用户可读写的临时会话文件；Mac 在打开后移除文件名，Windows 独占打开并在关闭时删除；处理完成、取消或退出后关闭。不保留音频历史，不承诺介质上的安全擦除。文字历史保留最近七天、最多 200 条，可在应用内删除；逻辑删除不保证清除备份或介质残留。词典和明确反馈形成的偏好本机保存，可编辑、暂停、撤销或清空。应用不会将一般键盘输入作为学习材料。

API Key、随机安装标识及体验凭据保存在 macOS 钥匙串或 Windows 凭据管理器，不进入设置导出。词典导入支持 UTF-8 TXT 和 JSON，合并去重；导出文件只包含词条，不包含密钥、历史、偏好或安装标识。导出位置和文件分享由你决定，词条本身也可能包含私人名称。

## 划词用途判断

划词处理只服务于所选文字的编辑、翻译、回复组织和相关理解。自有文字模型会先收到一次包含选中文字及语音指令的用途判断请求，再在通过后生成结果；这会增加一次模型调用、耗时和可能的费用。默认服务在服务端独立判断。判断失败或请求无关时不生成答案、不替换原文、不复制结果，也不将本次结果记入历史或学习。判断由模型完成，可能误判，不能保证阻止所有提示注入。普通口述和翻译不增加这次判断。

## 免费共享服务

使用默认体验服务前需主动同意。开通向 https://test.vaovao.app 发送随机安装标识及同意版本，不采集硬件序列号。服务保存安装标识的带密钥摘要、凭据摘要、同意记录、撤销状态和请求计数，不保存凭据原文。旧设备表中的期限、额度字段不再用于拒绝请求。为兼容旧客户端，状态接口保留数字型期限和额度占位值；新客户端依据 `unlimited` 标记显示不设期限及每日额度。

服务不设使用期限、每日请求总量、每设备每日请求量或每日开通人数限制。仍需有效凭据，撤销的凭据不能重新开通。ASR 同时两路、文字同时四路、总处理四路，另有最多八个请求短时排队；请求体、单次音频片段、模型上下文和超时等资源边界仍存在；ASR 只在 512 token 的运行预算内选取词典提示，超出的提示不发送给语音模型，不因此拒绝音频；完整词典仍在本机保留并参与文字整理。长录音由客户端分段。服务免费共享，不保证稳定性和响应速度，繁忙时可能排队或暂不可用；建议按需要配置自有云服务或本地模型。

模型请求会发送录音、待整理文字、词典及适用偏好；选区操作还包含所选文字。开启上下文增强后会发送下文说明范围内的参考文字。服务在请求期间处理，不保存音频或文字正文，不用于训练。运维日志只含请求编号、路径、状态和耗时；网络设施仍会处理来源地址。设备记录用于鉴权和撤销，使用计数保留约 35 天。新开通不再记录网络摘要；既有按日变化的网络摘要随后续开通清理超过约 35 天的记录。

当前没有服务端自助删除入口，可通过项目 issue 询问处理方式。**不要在 issue 中提交密钥、录音或私人正文**；先请求私密渠道，见 SECURITY.md。重新安装不自动清除钥匙串、Windows 凭据管理器或服务端记录。

## 自有模型

自有连接直接访问你配置的服务，不经 Vao 体验网关，遵循其隐私政策。远程要求 HTTPS，回环服务允许 HTTP。连接失败不静默切到默认服务；只有明确配置的备用文字连接会在有限错误条件下被调用。

点击语音连接测试时，仅向当前所选服务发送内置合成英语短句，不启用麦克风，不发送历史、词典或输入框上下文；文字连接测试仅发送固定改口例句。两者均使用所选协议、可能产生服务用量，不改变已保存连接，测试报告仅在当前应用会话显示。

## 输入增强和剪贴板

麦克风权限用于录音；在 Mac 上，辅助功能用于快捷键、选区读取、原输入位置校验与回填。Mac 目标保护会临时读取本次捕获输入框的内容，计算仅存于适配进程内存的指纹，不发送该指纹。Mac 录音中切应用后仍尝试返回起始位置；内容或光标冲突、原窗口失效时保留结果并浮层提示。应用只输入，不替你发送消息。安装新版或更换签名后可能需重新授权。

以下开关默认关闭，下一次录音生效；Windows 的窗口参考范围及默认输入方式另见下文。预览需要兼容语音连接；上下文仅用于普通听写。选区操作和翻译的最终处理保持独立：

- **录音时显示临时转写**：文件接口发送最近最多十二秒的窗口，每次请求结束后间隔约四秒，再对完整录音进行最终识别。实时接口持续发送 PCM 音频、分段提交，只有完整确认且覆盖录音的结果才复用；中断或不完整时尝试同一连接的文件识别。默认共享服务当前使用文件接口。预览文字不单独进入历史或学习；复用并完成整理的转写会作为最终结果的原文进入正常历史。
- **参考输入框前后文**：校验起始非安全输入框后，读取光标前后各最多 500 字符、合计最多 8 KiB，发给本次文字连接及必要时已配置的备用连接。
- **Mac 参考起始窗口的附近文字**：依赖前后文开关，额外参考原窗口最多 1200 字符可见静态文字、120 字符标题及网页域名，不包含完整网址、路径或查询。跳过已识别的敏感应用、密码控件及不可验证输入框；其他应用的可见文字仍可能敏感，请按场景决定是否开启。附近文字可能包括消息内容。采集有时间和节点数量边界，不读取切换后的新应用，也不遍历其他窗口。无法读取的部分留空。

上下文仅作为不可信参考，不作为指令，不写入历史或学习，处理完成或取消后释放；受限 Vao 整理协议不发送上下文。开启预览会增加发送量、请求量和自有服务费用。取消只能阻止后续请求与本机使用晚到结果，不能收回已发送数据或保证远端立即停止。

Mac 选区读取和回填可能临时使用剪贴板，尽量保留多格式内容，并仅在剪贴板未被其他程序更新时恢复。回填内容带临时、隐藏标记，但不能保证所有第三方剪贴板管理器都遵守。

## English

Audio uses a bounded queue and an owner-only session file, unlinked immediately on Mac or exclusively opened with deletion on close on Windows. Files close after processing, cancellation or exit. No audio history is retained. Text history lasts seven days, up to 200 results. Deletion is logical, without guarantees about backups or media erasure. Vocabulary and explicit layout feedback stay locally. Keys and installation credentials stay in macOS Keychain or Windows Credential Manager. Dictionary TXT/JSON exports contain terms only, without keys, history, preferences or installation identifiers; terms may themselves be private.

Optional hosted access requires consent and sends a random installation secret and notice version, not hardware identifiers. The service stores keyed installation hashes, credential hashes, consent, revocation and request counters. No expiry, daily use quota or daily enrollment quota is enforced, including for existing credentials; revocations remain effective. Numeric expiry/quota placeholders remain solely for legacy-client compatibility, with an `unlimited` flag for new clients. Resource bounds remain: two ASR slots, four text slots, four total active requests, up to eight queued requests, bounded request bodies/audio slices, model context, an ASR hint budget of 512 tokens (longer hints are reduced without rejecting audio; saved vocabulary still accompanies text refinement), and timeouts. Free shared service does not guarantee stability or response times. Consider your own cloud/local models for regular use.

Model requests send audio, current text, vocabulary, relevant preferences, selected text when applicable and enabled context. Audio/text bodies are processed temporarily, not retained or used for training. Operational logs exclude content; infrastructure handles network addresses. Usage counters last about 35 days. New enrollments no longer store source-network digests; old digests older than about 35 days are cleaned during subsequent enrollments. Device records support authentication and revocation. Reinstallation does not erase Keychain, Credential Manager or server records. For record-handling questions, request a private channel through the project; never post credentials or private content in issues.

Custom connections go directly to your provider under its policy, without silent fallback to hosted access. A configured backup text connection handles only limited failures. On Mac, Microphone and Accessibility support recording, shortcuts, selection and original-target insertion; messages are never automatically sent. Input fingerprints remain native memory and are not sent to models.

All input-enhancement options are off by default. File preview sends overlapping recent audio windows; final recognition runs separately. Realtime preview requires the public transcription WebSocket contract. Only complete confirmed coverage can be reused for ordinary final dictation, otherwise the same connection's file API is attempted. The hosted service currently uses files. Temporary text is excluded from history/learning, except when confirmed text becomes the original of a normal final result. Cancellation cannot recall transmitted audio or guarantee remote cancellation.

On Mac, surrounding-input context sends up to 500 characters per side, 8 KiB total. Its separate expanded option adds up to 1200 visible-text characters, 120 title characters and a website domain from the originally captured window, without full URL/path/query. Known sensitive applications, password controls and unverifiable inputs are skipped; other visible material, including messages, can still be private. Data goes to the chosen text service and configured backup when needed. Raw context is session-only, excluded from history/learning, never an instruction, and omitted for restricted Vao refine, translation and selection workflows.

On Mac, selection and insertion may temporarily use the clipboard. Multiple formats are preserved where possible and restored only if another app has not updated it. Transient/concealed markers are advisory; third-party clipboard tools may ignore them.

Connection diagnostics send only fixed synthetic speech or a fixed correction sentence to the selected provider/protocol, using its service allowance. They do not enable the microphone, include history/dictionary/context, or change saved connections. Reports remain in the current app session only.

## Windows 输入与本机存储 / Windows input and local storage

正式 Windows 版本与早期测试版分别保存设置和凭据，不自动迁移数据。密钥、随机安装标识和体验凭据保存在 Windows 凭据管理器；卸载不会自动清除凭据或本机历史。麦克风访问检查只短暂打开设备并丢弃样本，不请求模型。长录音使用仅所有者可访问、独占打开且关闭自动删除的临时文件。

普通听写默认在结果完成时，使用剪贴板和一次 Ctrl+V 输入到当前光标位置。处理期间请保持目标输入框获得焦点；不会返回录音起始应用。不会按 Enter、提交或自动重发。无法输入时保留结果；未取得完整接收确认时保留粘贴文字供核对。剪贴板仅备份有界、可安全复制的内存格式；无法安全处理的格式会在写入前拒绝。只有取得严格文本/光标确认且剪贴板序号、事务标记均未变化时才尝试恢复旧内容；用户之后的复制优先。

“返回录音起始位置”默认关闭。开启后校验起始进程、字段、内容和光标；其兼容粘贴选项默认开启，仅在该模式下生效。部分应用无法恢复原光标，请在结果完成前回到原输入位置。划词修改始终校验原选区。完整字段快照仅留有界内存，不落日志或历史。显式复制结果也会更新剪贴板。

输入增强默认关闭；启用前后文时，仅在普通听写中向所选文字服务发送起始可验证字段光标前后各最多 500 字符。Windows 的窗口参考最多增加起始窗口标题的 120 个字符，不扫描页面、文档或采集域名。无法读取、密码、只读或已变化的目标会跳过上下文。原始上下文不参与历史或学习。

The Windows release stores settings and credentials separately from early test editions, without automatic migration. Keys and installation credentials stay in Windows Credential Manager. Uninstalling does not automatically erase them or local history. Microphone access checks discard samples and contact no model. Long recordings use owner-restricted, exclusively opened temporary files deleted on close.

Ordinary dictation defaults to the clipboard plus one Ctrl+V at the current caret when the result is ready. Keep the destination focused during processing; this mode does not return to the recording's starting app. It never presses Enter, submits or automatically retries. Unavailable inputs keep the result; unconfirmed receipt keeps paste text for inspection. Only bounded safely copied memory-backed clipboard formats are supported. Old contents are restored only after strict text/caret receipt and unchanged clipboard sequence/transaction marker. Later user copies win.

Returning to the starting input is off by default. If enabled, it verifies the original process, field, contents and caret. Its compatibility paste option defaults to on and applies only in this mode. Some apps cannot restore the original caret; return there before processing finishes. Selection edits always verify their original selection. Full-field snapshots stay in bounded memory, outside logs and history. Explicit copying also updates the clipboard.

Input enhancements are off by default. Ordinary-dictation context contains up to 500 characters per side of the original verified caret. Windows window context adds only up to 120 characters of the starting title; no page/document scan or domain collection. Unreadable, changed, password and readonly inputs skip context. Raw reference data stays out of history and learning.
