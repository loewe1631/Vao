# Vao privacy / 数据处理说明

Version 2026-10-19. Applies to Vao 0.1.0-beta.4.

## 本机数据

录音仅在当前会话内存中处理，不写入历史。文字结果在本机 SQLite 保存最近 7 天、最多 200 条；可在“历史”中删除。历史删除属于应用层逻辑删除，不保证抹除操作系统备份或存储介质上的残留。词典与明确反馈产生的排版偏好保存在本机，可暂停、撤销或清空。应用不会扫描聊天历史，不会把任意键盘输入作为学习材料。快捷键监视只匹配配置键位及用户明确开启的短时按键录入。

API Key、随机安装标识与体验设备凭据分别保存在 macOS 钥匙串，应用设置和源码包不包含密钥。卸载应用不自动删除钥匙串；因此重新安装可能沿用同一安装的体验到期日和额度，不重新发放体验。

## 默认体验服务

首次开通必须由用户主动勾选同意。开通向 https://test.vaovao.app 发送随机安装标识与说明版本，不采集硬件序列号。服务保存标识的带密钥摘要、设备凭据 SHA256 摘要、同意版本、创建/到期时间及使用额度；不保存安装标识或凭据原文。开通来源地址仅保存按日变化的 HMAC 摘要用于限制滥用，计数保留最多约 35 天。

使用默认体验时，通过 HTTPS 发送录音、当前待整理文字、当前词典及本次需要的排版偏好；选区修改还会发送用户选中文字和口述修改指令。语音识别和文字整理由 Vao 的体验服务完成。服务端在请求运行期间处理内容，不保存音频或文字正文，不用于训练。运维记录只含随机请求编号、接口、状态和耗时；网络基础设施可能接触来源地址。不承诺“绝对零日志”或服务永远可用。

公众体验为 30 天、每设备每日 200 个模型请求；ASR 与文字整理各计一次，普通一次输入通常为 2 次；局部翻译等可能更多。额度按 UTC 自然日恢复。当前服务全体共享每日 2000 个模型请求，ASR 同时 2、文字同时 4、同时处理 4 个请求，另有最多 8 个请求短时排队。每日最多 100 个新安装、同一来源每日最多 10 个，另有请求限速。重装或重复开通不会重置同一随机安装的配额和到期日。此为小规模测试服务，不保证所有人随时可开通或使用；共享服务不保证响应速度；繁忙时可能排队，输出会变慢。建议日常使用配置自有服务，以获得更稳定的响应与效果。

设备记录持续用于到期、停用及防止同一安装重复领用；日用量保留约 35 天。本测试版暂无服务端自助删除入口，可通过项目 issue 询问处理方式，**不要在 issue 贴密钥、录音、私人正文**。使用项目仓库的 issue 请求处理方式；涉及隐私详情时先请求私密沟通渠道，参见 SECURITY.md。

## 自有模型

自有连接直接访问用户填写的服务，不经 Vao 体验网关。对应语音/文字、词典和本次偏好由该服务处理，遵循其隐私政策。本地服务可用回环 HTTP，远端应使用 HTTPS。自有连接失败不会静默改用默认体验；主用 LLM 只在已配置备用且满足有限错误条件时使用备用。

## 跨应用输入

麦克风权限用于录音。辅助功能用于 Fn 快捷键、当前选区读取和回填目标校验。读取选区可能短暂使用复制操作，结束后尝试恢复剪贴板；输入结果也可能短暂使用剪贴板。为恢复本次明确捕获的输入位置，应用会临时读取该输入框的值，计算仅存于适配进程内存中的指纹，并保留光标/选区位置；指纹和未选中的输入内容不写入存储、不发送给模型。录音期间切换应用后，应用尝试返回原窗口和输入框；原内容、光标发生冲突或原窗口失效时，保留结果并以浮层提示。无法验证输入框的应用仍保留前台交互校验。应用只输入，不替你发送消息。安装新版或更换签名后可能需要重新授权。

## English

Audio is held in session memory. Local text history is limited to seven days and 200 results. Deletion is logical application deletion, not guaranteed erasure from OS backups or storage media. Vocabulary and explicit layout feedback remain local until relevant information is needed in a user-initiated model request. No chat-history scraping, general keystroke collection, background enrollment, or training on trial content.

API keys, a random installation secret and individual trial credentials stay in macOS Keychain. Enrollment requires explicit consent, and sends the random secret and notice version over HTTPS, not hardware identifiers. The service retains keyed installation hashes, credential hashes, consent, expiry and usage. Enrollment rate limits use a daily-changing source-network hash, kept for about 35 days. Reinstalling does not reset an existing installation grant.

Trial requests send audio, current text, vocabulary, applicable preferences and (for selection edits) selected text to Vao servers. Speech recognition and text refinement run on Vao's trial service. Audio/text bodies are processed temporarily and not retained or used for training. Content-free operational logs include request IDs, paths, status and timing. Infrastructure processes network addresses.

The public trial lasts 30 days with 200 model POST requests daily per installation, shared service capacity of 2000 daily, two ASR/four text slots and four processing requests and up to eight queued requests. Recognition and refinement each count as one; some features use more. UTC daily reset. Enrollment is capped at 100 new installations daily, ten per source, plus rate limits. Shared response times are not guaranteed; queues may delay output. Connect your own service for more consistent performance and results. Device records remain for expiry/revocation and repeat-grant prevention; daily usage is retained for about 35 days. There is no in-app server-record deletion in this beta. Never post credentials or private content in issues.

Custom model requests go directly to your provider under its policy. Recording requires Microphone access; Fn, selection and insertion need Accessibility. Selection/insertion may temporarily use the clipboard. For the explicitly captured input only, its value is read temporarily to calculate an in-memory fingerprint, together with the caret/selection position. Neither that fingerprint nor unselected input contents are stored or sent to models. This lets Vao return to the original field after you switch apps; conflicts preserve the result and show an overlay. Inputs without a verifiable snapshot keep stricter foreground checks. Messages are never automatically sent.

## Windows 开发预览 / Windows development preview

Windows 候选使用独立的 `app.vaovao.windows.preview` 数据和凭据标识。密钥、随机安装标识和体验凭据保存在 Windows 凭据管理器，不写入明文设置，也不从 Mac 迁移。卸载候选不会自动移除凭据或本机历史；可在 Windows 凭据管理器和应用数据目录中删除。麦克风访问检查只短暂打开设备，在内存中丢弃样本，不发起模型请求。此候选不读取选区，不执行跨应用输入；仅在明确点击复制时更新剪贴板。其他模型、历史与学习的数据边界同上。

The Windows candidate uses a separate `app.vaovao.windows.preview` data and credential identity. Keys and installation/trial credentials stay in Windows Credential Manager, outside plaintext settings. Uninstalling does not automatically delete credentials or local history; remove these through Credential Manager and the application data directory. A requested microphone access check briefly opens the device, discards memory samples and makes no model request. This candidate does not read selections or insert into other apps; clipboard writes require an explicit copy action. The model, history and learning boundaries above also apply.
