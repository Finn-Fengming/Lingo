# 测试记录与复现方法

记录日期：**2026-10-07**。环境：**macOS 26.4.1 / arm64 / Rust 1.97.1**。

目前已验证自动化检查、真实 DeepSeek 调用和打包应用内的翻译操作。**跨应用的全局手势、原位替换、撤销以及焦点/选区变化后的保护，尚未完成端到端验证**：测试机仍需为对应应用授予 macOS 辅助功能和输入监控权限。已完成下表列出的最终应用内回归；本记录不代表跨应用交互已经验收。

## 已取得的证据

| 项目 | 结果 | 范围与限制 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 最近一次运行通过 | Rust 格式检查 |
| `cargo clippy --all-targets -- -D warnings` | 最近一次运行通过 | 所有目标的静态检查，无警告 |
| `cargo test --all-targets` | 19 项通过 | 12 项库测试、1 项 UI Unicode 替换测试、6 项 HTTP 协议测试；无需真实密钥或外部模型服务，HTTP 测试使用本地回环服务器 |
| 真实 DeepSeek 基线测试 | 通过 | `你好，世界！` 返回英文译文；显式启用，默认忽略 |
| 真实 DeepSeek 扩展测试 | 4 项通过 | 英文、繁体中文、日语均检查代码、URL、段落及列表保留；另有一项将源文本中的指令作为文本翻译的测试 |
| 打包应用：中文全文翻译 | 已实际操作并观察成功 | GUI 显示约 **0.7 秒**；单次观测，不能当作稳定延迟或性能承诺 |
| 打包应用：局部选区翻译 | 已实际操作并观察成功 | 在 `KEEP \| 你好，世界！ \| KEEP` 中只选中中文，GUI 显示约 **0.6 秒**，左右 `KEEP` 保持不变 |
| 本地选区状态回归 | 最终 GUI 回归通过 | 局部替换后光标折叠到译文末尾，不再沿用旧选区；前后文字保持不变 |
| 取消翻译 | 最终 GUI 回归通过 | Escape 取消后显示原文保持不变；没有应用返回结果 |
| 模型设置校验 | 最终 GUI 回归通过 | 远程明文 HTTP 地址被拒绝；恢复 DeepSeek 默认配置后，连接测试成功 |
| 系统钥匙串 | 导入和读取通过 | 本地显式环境文件中的密钥可保存到钥匙串；不携带环境文件参数的诊断能发现凭据 |
| 本地 `.app` 打包 | 已构建并启动 | 使用 ad-hoc 签名；未进行 Developer ID 签名或 Apple 公证 |

本次 arm64 release 构建包含 Accessibility/AccessKit，应用包约 **7.3 MB**（`du -sh`）；`Info.plist` 与 ad-hoc 签名验证通过。

真实模型测试使用 `deepseek-flash` 和合成测试文本。指令注入样例通过，只能证明该样例当时的表现，不能证明模型不会遵循其他恶意源文本，也不能代替完整的翻译质量评估。Lingo 不向模型提供可执行工具。

## 复现自动化检查

在仓库根目录执行：

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

普通测试不会自动读取 `.env`，真实模型用例默认标记为 `ignored`。单独运行四项真实模型测试：

```sh
LINGO_TEST_ENV_FILE=/absolute/path/to/.env \
  cargo test --locked --test live_deepseek -- --ignored
```

环境文件必须由你显式指定，且包含自己的 `DEEPSEEK_API_KEY`。该命令会发送固定测试文本并产生服务商费用。测试不会打印密钥或原始服务商错误响应。不要将环境文件提交到仓库。

若已在当前终端安全设置 `DEEPSEEK_API_KEY`，可单独复现最初的基线检查：

```sh
cargo test --locked --test provider_http live_deepseek_flash_translation -- --ignored --exact
```

## 应用内手动检查

```sh
cargo run -- --doctor
scripts/bundle-macos.sh
codesign --verify --deep --strict dist/Lingo.app
open dist/Lingo.app
```

应用启动前，可在设置中保存密钥；也可先将显式环境文件中的密钥导入钥匙串，供从 Finder 启动的应用读取：

```sh
cargo run -- --env-file /absolute/path/to/.env --import-key
```

工作台复现步骤：

1. 输入 `你好，世界！`，选择 English，翻译全文，检查原输入框和译文区。
2. 输入 `KEEP | 你好，世界！ | KEEP`，只选中中文并翻译，检查前后文本没有变化。
3. 将光标移动到译文末尾，继续输入文字；再次全选或局部选中后翻译，检查选区没有沿用上一轮状态。
4. 切换繁体中文、日语、自定义语言，检查保存与再次启动后的设置。
5. 在翻译进行时编辑原文或取消，检查新输入未被替换；取消无法撤回已经发给服务商的请求。
6. 关闭主窗口后，从菜单栏重新打开；检查暂停、恢复和退出。

这些步骤是回归清单；除上表明确记录的项目之外，不应理解为已经全部通过。

## 跨应用验证：尚未完成

先在 **系统设置 → 隐私与安全** 中授予 Accessibility / 辅助功能与 Input Monitoring / 输入监控权限。命令行示例和打包应用的权限身份可能不同，必要时分别授权并重启。

使用 TextEdit 新建一个**可丢弃的纯文本测试文档**，输入并精确选中：

```text
LINGO_SMOKE_你好，世界！
```

测试原位替换及撤销：

```sh
cargo run --example platform_smoke -- --mode replace-undo --delay 5
```

命令启动后五秒内返回 TextEdit，使上述测试文字保持选中。示例只接受完全匹配的固定文本，会验证写入，再验证恢复原文；不调用模型、不输出文档正文。它直接调用平台接口，**不能代替全局快捷键/手势的验证**。

测试选区变化后的保护：

```sh
cargo run --example platform_smoke -- --mode changed --delay 5
```

在第一个五秒内回到 TextEdit，保持固定文本选中；捕获完成后的第二个五秒内移动选区或修改测试文本，检查替换被拒绝。

最后，使用已配置密钥的打包应用，在同一可丢弃文档分别测试 Option 拖选、⌘⇧L、菜单栏撤销，以及翻译期间切换应用、移动选区和编辑正文。记录目标应用与版本、权限状态和实际结果。尚未取得这些端到端结果前，不宣称“所有应用都可用”或“跨应用交互已完整验证”。
