# Vao

让表达，越来越像你。

[English](README.md) · [官网与下载](https://vaovao.app/) · [隐私说明](PRIVACY.md) · [架构](docs/ARCHITECTURE.md) · [参与贡献](CONTRIBUTING.md)

Vao 是一款开源的 Mac 语音输入工具：把口述整理成可用文字，根据内容与目标应用自动选择表达方式，并记住你明确选择的排版偏好。

**Apple Silicon · macOS 12+ · 公开测试阶段。** Windows x64 0.2.1 提供[单一安装程序](https://github.com/loewe1631/Vao/releases/tag/windows-v0.2.1)，见[Windows 使用说明](docs/WINDOWS.md)；Intel Mac 未交付。当前官网下载包尚未经过 Apple 公证。源码与安装包版本见[版本说明](RELEASE_NOTES.md)。

## 主要功能

- **说完即输入**：整理口述，回填原输入位置，不替你发送消息。
- **自动适配场景**：内置日常消息、任务、日程、技术记录和文档模板。内容优先，应用提示辅助判断，无需手动切换模式。
- **记住明确偏好**：同场景选择自然段或编号并记住，下次应用，随时暂停、禁用、撤销或清空。
- **划词追问与修改**：选中文字后按主键，直接说问题或要求。答案显示在声栏上方，修改可替换原选区。
- **整段或局部翻译**：直接开始翻译录音，或仅翻译普通录音中的某一段。
- **录音中切换应用**：查阅其他应用时继续说，结束后通过目标校验，返回起始输入位置。

- **可选输入增强**：临时转写独立开关、有限输入框上下文及可选附近文字；实时协议需服务明确支持。
- **词典迁移**：TXT/JSON 导入合并去重，只导出词条。

划词只处理所选文字的编辑、翻译、回复组织及相关理解。自有文字服务会先做一次用途判断，增加一次请求、等待及可能费用；无关请求或判断失败会停止。普通口述中的问题仍作为输入，不由 Vao 回答。

当前学习范围是日常消息、任务、技术写作三种场景中的明确排版选择，不会监视其他编辑，也不会把模型生成内容当作学习证据。长期适配质量仍需验证。详见[场景模板](docs/WRITING_PRESETS.md)。

## 安装与使用

1. 从[官网](https://vaovao.app/)下载，解压 ZIP 或打开 DMG，将 **Vao.app** 拖入“应用程序”。
2. 打开 Vao。若未公证下载包被拦截，按[安装与授权指南](https://vaovao.app/install/)处理，不要全局关闭 Gatekeeper。
3. 按开始页完成：主动开通体验或配置自有模型、按需授权、检查快捷键。
4. 点击目标输入框，按主键开始，说完再按一次结束。

| 默认快捷键 | 用途 |
| --- | --- |
| Control + Option + Space | 开始、结束录音 |
| Control + Option + T | 空闲时开始翻译；录音中开启、关闭片段翻译 |
| Escape | 取消当前会话 |

可录入按键或手动填写，包含 **Fn**、**Fn+Space**。Fn 依赖辅助功能，可能与系统地球键冲突。输入监控列表没有 Vao 时，用系统 **＋** 添加“应用程序”中的 Vao，开启后重启。点击授权不等于已经允许，权限页显示实际状态。

自动输入默认开启，关闭后先预览、复制。选中文字后按主键立即录制要求，再按一次结束：提问显示并复制答案、保留原文；修改在自动输入开启时尝试替换原选区。

跨应用回填依赖辅助功能支持。原窗口关闭、内容或光标冲突时保留结果，不填入其他窗口。不提供可靠输入快照的应用仍受前台交互校验限制；微信目前需先点消息输入框。发出粘贴事件不等于验证目标收到。

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

录音使用临时会话缓冲并发送到所选 ASR，不保留音频历史；本机文字历史保留最近七天、最多 200 条。密钥在 macOS 钥匙串。词典与相关偏好随模型请求发送；选区用于处理所选内容。权限、剪贴板、保留与删除方式见[隐私说明](PRIVACY.md)。

## 编译与检查

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

欢迎参与 Windows 适配、模型兼容、合成场景用例和辅助功能改进。请先读[贡献指南](CONTRIBUTING.md)、[路线图](docs/ROADMAP.md)与[安全报告](SECURITY.md)。

AGPL-3.0-only。产品方向源于多种开源项目的使用经验，当前客户端为 Vao 自有实现。[来源说明](PROVENANCE.md)、[素材清单](docs/SOURCES.md)与第三方许可须保留，不能因界面或架构变化取消许可义务。
