# Lingo

**换一种语言，继续表达。**

Lingo 是一个用 Rust 编写的轻量翻译应用，将你已经写好的文字交给大模型翻译。无论是一小段选区，还是整篇草稿，都可以使用自己的模型服务，按默认语言快速转换。应用提供独立翻译工作台，也包含面向 macOS 输入框的原位翻译集成。

[English](README.md) · [下载 v0.1.0](https://github.com/Finn-Fengming/Lingo/releases/tag/v0.1.0) · [更新记录](CHANGELOG.md) · [架构说明](docs/architecture.md) · [测试记录](docs/testing.md)

**v0.1.0 为公开预览版。** 工作台和真实 DeepSeek 翻译已经测试；跨应用手势、原位替换和撤销已经实现，但**尚未完成端到端验证**。实际兼容性取决于目标应用对 macOS 辅助功能的支持。

## 功能

- **只翻译需要的内容。** 在工作台中选择一段文字，或翻译整篇草稿，再复制译文或替换原文。
- **按习惯选择语言。** 默认翻译为英文，可切换繁体中文、日语等预设，也可填写自定义目标语言。
- **使用自己的模型。** 默认接入 DeepSeek Flash，可改用其他 OpenAI 兼容的 Chat Completions 服务。
- **在写作处调用翻译。** 原生集成支持 Option 拖选和 **⌘⇧L**，菜单栏提供暂停、取消和带状态检查的撤销。
- **保持客户端轻量。** 应用和 `egui` / `eframe` 界面均使用 Rust，无需 JavaScript 运行时，不使用传统机器翻译服务。

Lingo 处理已经写好的文本，不需要麦克风权限。交互设计的参考依据见 [产品调研](docs/research.md)。

## 下载与安装

首个预览版提供 **Apple Silicon（arm64）** macOS 应用：

- [Lingo-v0.1.0-macos-arm64.zip](https://github.com/Finn-Fengming/Lingo/releases/download/v0.1.0/Lingo-v0.1.0-macos-arm64.zip)
- [SHA256SUMS.txt](https://github.com/Finn-Fengming/Lingo/releases/download/v0.1.0/SHA256SUMS.txt)
- [完整发布说明与附件](https://github.com/Finn-Fengming/Lingo/releases/tag/v0.1.0)

应用声明的最低系统版本为 **macOS 12.0**，实际测试环境为 **macOS 26.4.1 / Apple Silicon**。本次不提供 Intel、Windows 或 Linux 二进制包。

1. 下载并解压 ZIP，将 **Lingo.app** 拖入 **应用程序**。
2. 打开 Lingo，在 **偏好设置** 中填写自己的 API Key 并保存；默认已经配置 DeepSeek Flash。
3. 选择目标语言，在工作台输入一小段文字试译。
4. 如需试用原生集成，在 **系统设置 → 隐私与安全** 中为 Lingo 开启 **辅助功能** 和 **输入监控**，然后重启应用。

此预览版采用 **ad-hoc 签名，尚未经过 Apple 公证**，macOS 可能阻止打开下载的应用。也可以按下方步骤从源码构建。下载包不包含任何服务商密钥。

将 ZIP 与校验文件放在同一目录，可执行：

```sh
shasum -a 256 -c SHA256SUMS.txt
```

## 本地运行

安装 [Rust 1.88 或更新版本](https://www.rust-lang.org/tools/install) 和 Apple 命令行开发工具，然后执行：

```sh
git clone https://github.com/Finn-Fengming/Lingo.git
cd Lingo
cargo run --release --locked
```

在应用中配置服务商和 API Key。开发时也可以显式加载仓库之外的环境文件：

```sh
cargo run --release --locked -- --env-file /absolute/path/to/.env
```

文件内容示例：

```dotenv
DEEPSEEK_API_KEY=replace-with-your-own-key
```

请使用自己的密钥，并将环境文件保存在仓库之外。不要把真实密钥放进 Issue、截图、Git 提交或命令行参数。

## 在输入框中翻译

完成设置和授权后，在支持的可编辑输入框中按住 **Option** 拖动选择文字并松开鼠标；也可以普通选中后按 **⌘⇧L**。翻译期间保持焦点和选区不变，Lingo 会在替换前检查原目标。

这里的“拖动”指**拖动选择文字**。部分编辑器为 Option 拖动定义了自己的操作；富文本编辑器、终端、受保护输入框和自绘界面也可能无法提供可用选区，请使用工作台。从终端运行时，macOS 可能将权限归于终端或开发二进制，而非打包后的应用。

原生集成会检查焦点控件、选区和原文是否变化，并在不匹配时拒绝替换。已完成的译文在无法写回时会保留在工作台；如果一开始就无法读取或编辑目标输入框，请将原文粘贴到工作台。这些保护逻辑仍需跨应用端到端验证。重要内容请在发送前核对译文。

关闭窗口后，Lingo 仍在菜单栏运行。菜单中可以重新打开工作台、暂停全局翻译、取消当前替换、撤销上次尚未被修改的替换，或退出应用。取消会丢弃后续返回的结果，无法撤回已经发出的文本，也无法阻止服务商对已发出的请求计费。撤销要求原输入框仍可访问且内容未被改动。

## 模型与扩展

| 配置项 | 默认值 |
| --- | --- |
| 服务地址 | `https://api.deepseek.com` |
| 模型 | `deepseek-flash` |
| 目标语言 | English |
| API 格式 | OpenAI 兼容的 `POST /chat/completions` |

截至 **2026-10-07**，DeepSeek 官方文档将 `deepseek-flash` 列为当前 Flash 模型名称。参考 [官方接入文档](https://api-docs.deepseek.com/guides/codex) 和 [模型更新记录](https://api-docs.deepseek.com/updates/)。

切换服务商时，在设置中修改服务地址、模型和对应密钥。服务需要接受标准 Chat Completions 消息，并在 `choices[0].message.content` 中返回文本。不同服务商的兼容程度有所差异，专有 API 和认证格式可能需要额外适配。也可以配置本地兼容端点。

远端服务必须使用 HTTPS；HTTP 仅允许 `localhost` 和字面回环地址。地址可包含 `/v1`，也可填写完整的 `/chat/completions` 路径；客户端不跟随重定向。保存的密钥按端点隔离。`LINGO_API_KEY` 会覆盖当前服务商的保存密钥；`DEEPSEEK_API_KEY` 仅用于 DeepSeek 官方 HTTPS 源站。环境变量优先于钥匙串。

## 命令行

```sh
# 查看本地配置及 macOS 权限状态
cargo run -- --doctor

# 不操作其他应用，直接翻译
cargo run -- --translate '你好，很高兴认识你。' --to Japanese

# 从独立环境文件读取开发密钥
cargo run -- --env-file /absolute/path/to/.env --translate '明天见。' --to English

# 将当前服务商的环境密钥存入钥匙串，供直接启动应用使用
cargo run -- --env-file /absolute/path/to/.env --import-key
```

## 隐私与安全

翻译时，选中或粘贴的文本、目标语言和翻译指令会发送给**你配置的模型服务商**。Lingo 是端侧客户端，默认模型在远端运行；数据处理和计费遵循所选服务商的规则。

Lingo 不保存持久化翻译历史。应用运行期间，原文和译文会保留在进程内存中。偏好设置存放在本机；保存的密钥使用 macOS 钥匙串，不写入偏好设置文件。开发环境支持环境变量和显式加载的环境文件。诊断信息不会输出密钥。

辅助功能权限用于在你触发翻译时读取、修改选中的可编辑控件。为检查写回是否安全，Lingo 会在本机临时保存该输入框的全文快照；只有选中文字会发送给模型。普通划词不会持续触发翻译。被系统标记为安全或密码类型的输入框会被排除。写回检查能降低误修改风险，但各应用实现存在差异，不支持的编辑器请使用工作台。详细边界见 [架构说明](docs/architecture.md)。

## 开发与打包

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
scripts/bundle-macos.sh
scripts/package-macos.sh
```

首轮验证包括 **19 项单元/模拟服务测试**、**4 项显式启用的真实 DeepSeek 测试**和工作台手动检查。GitHub CI 已通过 stable Rust 的格式、静态检查和测试，以及最低支持版本 **Rust 1.88** 的全部目标编译检查。真实模型用例使用单独提供的密钥，默认忽略。具体范围及待验证事项见 [测试记录](docs/testing.md)。

bundle 脚本生成并以 ad-hoc 方式签名本地 `.app`；package 脚本构建应用，并在 `dist/` 中生成带版本号的 ZIP 和 `SHA256SUMS.txt`。Developer ID 签名和 Apple 公证属于独立分发步骤，需要维护者的 Apple 开发者身份。

GitHub Actions 中的 **Build macOS bundle** 工作流支持手动执行，也会在推送 `v*` 标签时运行。它将当前运行器架构的应用 ZIP 和校验文件保存为工作流产物，不会自动发布 GitHub Release 或进行应用公证。

欢迎提交应用兼容性、模型适配、无障碍体验和其他桌面平台的贡献。反馈问题时请提供 macOS 版本、目标应用、复现步骤和脱敏诊断信息，不要附带私密原文或 API Key。

## 开源协议

[MIT](LICENSE) © 2026 Finn Fengming。
