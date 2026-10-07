# Lingo

**选中文字，原地翻译，继续表达。**

Lingo 是一个用 Rust 编写的轻量桌面应用，通过大模型翻译 macOS 输入框中选中的文本。按住 **Option** 拖动选择文字，或者先选中文字，再按 **⌘⇧L**。默认翻译为英文，也可配置为繁体中文、日语或自定义语言。

[English](README.md) · [产品调研](docs/research.md) · [架构说明](docs/architecture.md) · [测试记录](docs/testing.md)

## 功能

- 通过 macOS 辅助功能接口读取选区，并在支持的输入框中原地替换。
- 提供翻译工作台：遇到不支持原地操作的应用时，可粘贴、翻译、复制。
- 默认使用 DeepSeek Flash，支持配置 OpenAI 兼容的 Chat Completions 服务地址和模型。
- 使用 Rust 和 `egui` / `eframe` 构建界面，无需 JavaScript 运行时，不使用传统机器翻译服务。

当前为 macOS 早期版本。原地翻译是否可用，取决于目标应用对辅助功能接口的支持；富文本编辑器、终端、受保护输入框和自绘界面可能无法读取选区或写回，请使用工作台。

## 本地运行

安装 [Rust 1.88 或更新版本](https://www.rust-lang.org/tools/install) 和 Apple 命令行开发工具，然后在仓库目录执行：

```sh
cargo run --release
```

在应用中配置服务商和 API Key。开发时也可以显式加载仓库之外的环境文件：

```sh
cargo run --release -- --env-file /absolute/path/to/.env
```

文件内容示例：

```dotenv
DEEPSEEK_API_KEY=replace-with-your-own-key
```

请使用自己的密钥，并将环境文件保存在仓库之外。不要把真实密钥放进 Issue、截图、Git 提交或命令行参数。仓库不包含可用密钥。

## 在输入框中翻译

1. 启动 Lingo，配置密钥及目标语言。
2. 在 **系统设置 → 隐私与安全** 中，为正在运行的应用开启 **辅助功能** 和 **输入监控**。授权后若触发仍不可用，请重启 Lingo。从终端运行时，macOS 可能将权限归于终端或开发二进制，而非打包后的应用。
3. 在其他应用的可编辑输入框中，按住 **Option** 拖动选择文字，松开鼠标；也可普通选中后按 **⌘⇧L**。
4. 翻译期间保持焦点和选区不变。写回之前，Lingo 会检查原始选区。

这里的“拖动”指拖动选择文字，无需把文本拖入另一个窗口。有些编辑器为 Option 拖动定义了特殊操作，这时请使用快捷键或工作台。Lingo 不需要麦克风权限。

如果翻译期间焦点或选区变化，Lingo 会保留输入框原文，将译文放在工作台，供你手动复制。如果目标输入框本身不支持辅助功能读取或写入，请先将原文粘贴到工作台，再进行翻译。重要内容请在发送前核对译文。

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
```

CI 在 macOS 上执行格式、静态检查和测试，不使用服务商密钥。真实模型调用和跨应用交互需要单独验证。打包脚本生成本地 `.app`；正式分发受 macOS 信任的应用，还需要使用你自己的 Apple 开发者身份签名和公证。

已验证结果及复现方法见 [测试记录](docs/testing.md)。跨应用手势、替换和撤销**尚未完成端到端验证**，需要先在测试机授予相应 macOS 权限。

GitHub Actions 中的 **Build macOS bundle** 工作流支持手动执行，也会在推送 `v*` 标签时运行。它将当前运行器架构的应用 ZIP 保存为工作流产物，不会自动发布 GitHub Release 或进行应用公证。

欢迎提交应用兼容性、模型适配、无障碍体验和其他桌面平台的贡献。反馈问题时请提供 macOS 版本、目标应用、复现步骤和脱敏诊断信息，不要附带私密原文或 API Key。

## 开源协议

[MIT](LICENSE) © 2026 Finn Fengming。
