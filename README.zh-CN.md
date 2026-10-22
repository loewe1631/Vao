<p align="center"><img src="ui/assets/logo.svg" width="80" height="80" alt="Vao 标志"></p>

<h1 align="center">Vao</h1>

<p align="center"><strong>让表达，越来越像你。</strong></p>

<p align="center">macOS &amp; Windows · AGPL-3.0-only</p>

<p align="center">人类发起 · 多 AI Agent 协作 · 全栈制作</p>

[English](README.md) · [官网与下载](https://vaovao.app/) · [隐私说明](PRIVACY.md) · [架构](docs/ARCHITECTURE.md) · [参与贡献](CONTRIBUTING.md) · [AI 协作说明](AI_DEVELOPMENT.md)

Vao 是一款面向 macOS 与 Windows 的开源语音输入工具：把口述整理成可用文字，根据内容与目标应用自动选择表达方式，并记住你明确选择的排版偏好。

**Mac 0.2.2 · Windows 0.2.2。** [统一 0.2.2 发布页](https://github.com/loewe1631/Vao/releases/tag/v0.2.2)提供两个平台的安装包，均新增可选的停说后自动结束。

| 平台 | 安装包 | 要求 |
| --- | --- | --- |
| macOS | [DMG](https://github.com/loewe1631/Vao/releases/download/v0.2.2/Vao-0.2.2-build20261022.1302-mac-arm64.dmg) · [ZIP](https://github.com/loewe1631/Vao/releases/download/v0.2.2/Vao-0.2.2-build20261022.1302-mac-arm64.zip) | Apple Silicon · macOS 12+ |
| Windows | [EXE 安装程序](https://github.com/loewe1631/Vao/releases/download/v0.2.2/Vao-0.2.2-setup.exe) | Windows 10 1809+ · x64；缺少 WebView2 时自动下载 |

Mac 尚未经过 Apple 公证，Windows 安装包尚无代码签名；首次打开提示见[安装指南](https://vaovao.app/install/)。Intel Mac 暂未提供。版本及源码对应关系见[版本说明](RELEASE_NOTES.md)。

## 主要功能

- **说完即输入**：整理口述并填入目标输入框，不替你发送消息。
- **自动适配场景**：内置日常消息、任务、日程、技术记录和文档模板。内容优先，应用提示辅助判断，无需手动切换模式。
- **记住明确偏好**：同场景选择自然段或编号并记住，下次应用，随时暂停、禁用、撤销或清空。
- **划词追问与修改**：选中文字后按主键，直接说问题或要求。答案显示在声栏上方，修改可替换原选区。
- **整段或局部翻译**：直接开始翻译录音，或仅翻译普通录音中的某一段。
- **录音中切换应用**：Mac 通过校验后回填起始位置；Windows 默认输入到结果就绪时的当前光标，可选返回起始输入框。

- **可选输入增强**：临时转写独立开关、有限输入框上下文及可选窗口参考（Mac 附近文字、Windows 仅标题）；实时协议需服务明确支持。
- **词典迁移**：TXT/JSON 导入合并去重，只导出词条。

划词只处理所选文字的编辑、翻译、回复组织及相关理解。自有文字服务会先做一次用途判断，增加一次请求、等待及可能费用；无关请求或判断失败会停止。普通口述中的问题仍作为输入，不由 Vao 回答。

当前学习范围是日常消息、任务、技术写作三种场景中的明确排版选择，不会监视其他编辑，也不会把模型生成内容当作学习证据。长期适配质量仍需验证。详见[场景模板](docs/WRITING_PRESETS.md)。

## 这个项目是怎样做出来的？

> **人类发起，多 AI Agent 协作，全栈制作。**
>
> 从真实使用中的需求出发，多个 AI Agent 参与研究、设计、开发、工程检查、网站制作与宣传运营。人类持续试用、反馈问题，并决定产品方向和版本发布。

我们选择公开 Vao 的制作方式，也欢迎你查看代码、提出问题，参与它的下一次改进。

| 人类负责 | AI Agent 参与 |
| --- | --- |
| 需求、方向、实测、发布决策 | 方案研究、设计开发、工程检查、站点日常维护与宣传制作 |

参与工具、模型与具体分工见 [AI_DEVELOPMENT.md](AI_DEVELOPMENT.md)。

## 安装与使用

1. 从[0.2.2 发布页](https://github.com/loewe1631/Vao/releases/tag/v0.2.2)选择平台下载：Mac 将 **Vao.app** 拖入“应用程序”；Windows 运行 **Vao-0.2.2-setup.exe** 并按安装向导操作。
2. 打开 Vao。首次系统安全提示及权限设置见[安装与授权指南](https://vaovao.app/install/)；Windows 的详细操作见 [Windows 指南](docs/WINDOWS.md)。
3. 按开始页完成：主动开通体验或配置自有模型、按需授权、检查快捷键。
4. 点击目标输入框，按主键开始，说完再按一次结束。

| 默认快捷键 | 用途 |
| --- | --- |
| Mac：Control + Option + Space；Windows：Ctrl + Alt + Space | 开始、结束录音 |
| Mac：Control + Option + T；Windows：Ctrl + Alt + T | 空闲时开始翻译；录音中开启、关闭片段翻译 |
| Escape | 取消当前会话 |

Mac 可录入按键或手动填写，包含 **Fn**、**Fn+Space**。Fn 依赖辅助功能，可能与系统地球键冲突。输入监控列表没有 Vao 时，用系统 **＋** 添加“应用程序”中的 Vao，开启后重启。点击授权不等于已经允许，权限页显示实际状态。

自动输入默认开启，关闭后先预览、复制。选中文字后按主键立即录制要求，再按一次结束：提问显示并复制答案、保留原文；修改在自动输入开启时尝试替换原选区。

Mac 跨应用回填依赖辅助功能支持。原窗口关闭、内容或光标冲突时保留结果，不填入其他窗口。不提供可靠输入快照的应用仍受前台交互校验限制；微信目前需先点消息输入框。发出粘贴事件不等于验证目标收到。

Windows 使用常规组合快捷键，不支持硬件 Fn。普通听写默认在结果就绪时粘贴到当前光标，处理期间请保持目标输入框焦点；“返回录音起始位置”为可选项，部分应用无法恢复原光标。登录启动默认关闭。

## 模型与可选体验服务

ASR、文字服务分别配置。填写地址与可选密钥，获取模型、选择并保存；高级选项可手填。获取列表只代表发现模型。语音和文字连接测试按当前所选协议验证，显示测试原文、实际返回与延迟。语音测试使用内置英语短句，不启用麦克风，会使用服务用量；实时测试必须取得 WebSocket 最终转写，不回退文件识别。之后可用短录音检查麦克风与实际输出质量。

| 阶段 | 支持协议 |
| --- | --- |
| 语音识别 | OpenAI 兼容 multipart `/v1/audio/transcriptions`，返回 JSON `text` |
| 实时识别（显式选择） | OpenAI Realtime 转写 WebSocket 会话；须服务及所选模型支持，不能仅凭文件识别成功确认 |
| 整理、翻译、选区指令 | `/v1/chat/completions`，返回非流式最终消息 |
| 受限整理 | Vao `/v1/refine`，仅普通口述 |

本地 ASR 可评估 **Qwen3-ASR-1.7B**，资源有限时评估 **0.6B**。NVIDIA、Apple Silicon MLX 推理服务均须提供所需协议，模型名相同不代表接口兼容。程序不含权重或推理服务器。远程要求 HTTPS，本机回环服务支持 HTTP。

默认体验用于配置自有服务之前测试功能，需主动同意隐私说明，使用独立凭据，不内置共享 API Key。共享容量有限，排队可能变慢，无法保证速度与质量；日常建议自有云服务或本地模型。[免费服务与数据处理](PRIVACY.md)。

自有 ASR、LLM 按阶段优先，失败不会静默改用体验服务。仅在已明确配置备用 LLM 时，传输错误、HTTP 429 或服务端错误才尝试备用。

## 隐私

录音使用临时会话缓冲并发送到所选 ASR，不保留音频历史；本机文字历史保留最近七天、最多 200 条。密钥保存在 macOS 钥匙串或 Windows 凭据管理器。词典与相关偏好随模型请求发送；选区用于处理所选内容。权限、剪贴板、保留与删除方式见[隐私说明](PRIVACY.md)。

## 源码与编译

当前 `main` 保留 Mac 客户端和 Xcode 构建入口；Windows 发行源码位于 [`windows-v0.2.2`](https://github.com/loewe1631/Vao/tree/windows-v0.2.2)，后续开发在 [`codex/windows`](https://github.com/loewe1631/Vao/tree/codex/windows)。两个标签未合并，统一 Release 中的自动 “Source code” 对应 Mac 标签；Windows 请下载带 `windows-source` 的归档或检出其标签。Windows 构建步骤见 [Windows build](docs/WINDOWS_BUILD.md)。

### Mac 编译与检查

需要 Apple Silicon、Xcode Command Line Tools、Python 3.11+、Node.js 22+、Rust stable 及 Clippy。

安装完整 Xcode 后，也可以打开 **[xcode/Vao.xcodeproj](xcode/Vao.xcodeproj)**，选择 **Vao Direct**，点击 **Product → Build**。它使用本地 ad-hoc 签名，构建同一套完整客户端，详见 [Xcode 构建说明](docs/XCODE.md)。当前目标用于直装；[App Store 适配](docs/APP_STORE.md)在同一仓库中单独推进。

```sh
cargo fetch --locked --target aarch64-apple-darwin
cargo test --offline --workspace --locked
cargo clippy --offline --workspace --all-targets --locked -- -D warnings
VAO_ALLOW_ADHOC=1 bash scripts/build-macos.sh
npm ci
npx playwright install chromium
npm run test:ui
npm run test:source
npm run check:source
python3 scripts/package-source.py
```

上面显式选择本地 ad-hoc 签名，不代表 Apple 公证；更换签名可能需要重新授权。持久签名用 `VAO_SIGNING_IDENTITY` 指定，详见[分发说明](docs/DISTRIBUTION.md)。安装包输出到 `dist/`，构建中间产物位于 `.build/` 和 Cargo 目标目录；支持 `CARGO_TARGET_DIR`，不会替换或安装应用。

浏览器检查使用合成 IPC，不能代替真实录音、模型质量或目标接收验收。原生测试及影响见[测试说明](docs/TESTING.md)。源码包带 SHA256 清单，排除缓存与用户数据。

## 贡献与许可

欢迎参与跨平台应用兼容、场景模板、模型协议、合成场景用例和辅助功能改进。人工编写或 AI 辅助的贡献，都以具体改动与验证结果交流。请先读[贡献指南](CONTRIBUTING.md)、[路线图](docs/ROADMAP.md)与[安全报告](SECURITY.md)。

AGPL-3.0-only。产品方向源于多种开源项目的使用经验，当前客户端为 Vao 自有实现。[来源说明](PROVENANCE.md)、[素材清单](docs/SOURCES.md)与第三方许可须保留，不能因界面或架构变化取消许可义务。
