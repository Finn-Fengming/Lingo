# Changelog / 更新记录

## [v0.1.0](https://github.com/Finn-Fengming/Lingo/releases/tag/v0.1.0) — 2026-10-07

**First public preview / 首个公开预览版**

### English

Lingo's first release brings LLM translation to a lightweight Rust desktop app. Translate selected text or a full draft in the workbench, choose a default target language, and connect your own model provider.

- Added a Rust workbench with whole-text and selection translation, optional replacement, and copyable results.
- Added English, Traditional Chinese, Japanese, other language presets, and custom target languages.
- Added configurable OpenAI-compatible Chat Completions support, with DeepSeek `deepseek-flash` as the default.
- Added macOS Option-drag and ⌘⇧L integration, target checks before replacement, and menu-bar pause, cancel, and undo.
- Added endpoint-scoped macOS Keychain storage, explicit environment-file loading, command-line translation, and diagnostics.
- Added provider/Unicode tests, opt-in live DeepSeek tests, macOS CI, and a local application bundle script.
- Fixed stale workbench selection/caret state after translation and cancellation ordering during result handling.

The release includes an Apple Silicon ZIP and SHA-256 checksums. The app is ad-hoc signed and not Apple-notarized. Its declared minimum is macOS 12; actual testing was on macOS 26.4.1 arm64.

The workbench and live provider have been verified. Cross-app gestures, replacement, undo, and changed-target protection still need end-to-end verification. This release provides no Intel, Windows, or Linux binary. See [release notes](docs/releases/v0.1.0.md) and [test evidence](docs/testing.md).

### 简体中文

Lingo 首个版本提供基于大模型的 Rust 桌面翻译工具：在工作台中翻译选区或完整草稿，配置默认目标语言，并接入自己的模型服务。

- 新增 Rust 翻译工作台，支持全文/选区翻译、可选替换及复制译文。
- 新增英文、繁体中文、日语等预设，以及自定义目标语言。
- 新增可配置的 OpenAI 兼容 Chat Completions 接口，默认使用 DeepSeek `deepseek-flash`。
- 新增 macOS Option 拖选、⌘⇧L、替换前目标检查，以及菜单栏暂停、取消和撤销。
- 新增按端点隔离的 macOS 钥匙串存储、显式环境文件加载、命令行翻译与诊断。
- 新增协议与 Unicode 测试、可选真实 DeepSeek 测试、macOS CI 和本地应用打包脚本。
- 修复翻译后的工作台旧选区/光标状态残留，以及处理返回结果时的取消顺序。

本次提供 Apple Silicon ZIP 和 SHA-256 校验文件。应用采用 ad-hoc 签名，尚未经过 Apple 公证；声明最低支持 macOS 12，实际测试环境为 macOS 26.4.1 arm64。

工作台和真实模型调用已验证。跨应用手势、替换、撤销及目标变化保护仍需端到端验证；本次不提供 Intel、Windows 或 Linux 二进制。详见 [发布说明](docs/releases/v0.1.0.md) 和 [测试记录](docs/testing.md)。
