# Lingo

**Your words, in another language. Right where you write.**

Lingo is a lightweight Rust app for translating the text you already wrote. Translate a phrase or an entire draft with your own LLM provider, choose a default language, and keep writing. It includes a text workbench and macOS integration for translating selections inside supported apps.

[简体中文](README.zh-CN.md) · [Download v0.1.0](https://github.com/Finn-Fengming/Lingo/releases/tag/v0.1.0) · [Changelog](CHANGELOG.md) · [Architecture](docs/architecture.md) · [Test evidence](docs/testing.md)

**v0.1.0 is a public preview.** The workbench and live DeepSeek translation have been tested. Cross-app gestures, replacement, and undo are implemented but **have not been verified end to end**. Compatibility depends on each app's Accessibility support.

## What it does

- **Translate just what you need.** Select a passage or translate the whole draft in the workbench, then copy the result or replace the original text.
- **Choose your language.** English is the default; switch to Traditional Chinese, Japanese, other presets, or a custom target.
- **Bring your own model.** Start with DeepSeek Flash or configure another OpenAI-compatible Chat Completions endpoint.
- **Stay close to your writing.** Option-drag and **⌘⇧L** invoke the native integration; the menu bar offers pause, cancel, and guarded undo.
- **Keep the client small.** The application and its `egui` / `eframe` interface are written in Rust, with no JavaScript runtime or traditional machine-translation service.

Lingo works with existing text and does not need microphone access. The interaction research behind the project is in [design notes](docs/research.md).

## Download and install

The first preview provides an **Apple Silicon (arm64)** macOS app:

- [Lingo-v0.1.0-macos-arm64.zip](https://github.com/Finn-Fengming/Lingo/releases/download/v0.1.0/Lingo-v0.1.0-macos-arm64.zip)
- [SHA256SUMS.txt](https://github.com/Finn-Fengming/Lingo/releases/download/v0.1.0/SHA256SUMS.txt)
- [Release notes and assets](https://github.com/Finn-Fengming/Lingo/releases/tag/v0.1.0)

The bundle declares macOS **12.0 or later**; actual testing was on **macOS 26.4.1, Apple Silicon**. There are no Intel, Windows, or Linux binaries in this release.

1. Download and extract the ZIP, then drag **Lingo.app** into **Applications**.
2. Open Lingo, go to **偏好设置** (Preferences), enter your own API key, and save. DeepSeek Flash is preconfigured.
3. Choose a target language and try a short passage in the workbench.
4. To try the native integration, enable **Accessibility** and **Input Monitoring** for Lingo in **System Settings → Privacy & Security**, then restart the app.

This preview is **ad-hoc signed and is not notarized by Apple**. macOS may block the downloaded app. Building from source is also available below. No credentials are included in the download.

To verify the download, place the ZIP and checksum file in the same folder and run:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

## Run from source

Install [Rust 1.88 or newer](https://www.rust-lang.org/tools/install) and Apple's command-line developer tools:

```sh
git clone https://github.com/Finn-Fengming/Lingo.git
cd Lingo
cargo run --release --locked
```

Configure your provider and API key in the app. For development, an explicit environment file is also supported:

```sh
cargo run --release --locked -- --env-file /absolute/path/to/.env
```

The file should contain a key obtained from your provider:

```dotenv
DEEPSEEK_API_KEY=replace-with-your-own-key
```

Keep this file outside the repository. Do not include real keys in issues, screenshots, commits, or command-line arguments.

## Translate where you write

After setup and permission grants, hold **Option**, drag across text in a supported editable field, and release. Alternatively, select text normally and press **⌘⇧L**. Leave the field and selection unchanged while translation runs; Lingo checks the original target before replacing it.

The gesture means **drag to select text**. Some editors assign their own behavior to Option-drag. Rich editors, terminals, protected fields, and custom canvas interfaces may not expose a usable selection; use the workbench in those cases. When running from a terminal, macOS may assign permissions to the terminal or development binary rather than the bundled app.

The native integration is designed to refuse replacement when the focused control, selection, or original text changes. A completed result remains available in the workbench if replacement cannot finish. If the initial field cannot be read or edited, paste its text into the workbench. These safeguards still need cross-app end-to-end verification in this preview. Review translations before sending important messages.

Closing the window keeps Lingo running in the menu bar. Use its menu to reopen the workbench, pause global translations, cancel a pending replacement, undo the last unchanged replacement, or quit. Cancel discards the eventual result; it cannot recall text already sent to the provider or prevent charges for an in-flight request. Undo requires the original field and unchanged content to remain available.

## Provider configuration

The default configuration is:

| Setting | Default |
| --- | --- |
| Base URL | `https://api.deepseek.com` |
| Model | `deepseek-flash` |
| Target language | English |
| API format | OpenAI-compatible `POST /chat/completions` |

DeepSeek documents `deepseek-flash` as its current Flash model name as of **2026-10-07**. See the [official API guide](https://api-docs.deepseek.com/guides/codex) and [model updates](https://api-docs.deepseek.com/updates/).

For another provider, set its base URL and model in settings and use its API key. Providers must accept standard Chat Completions messages and return text in `choices[0].message.content`. OpenAI compatibility varies; provider-specific APIs and authentication formats may need an adapter. Local compatible endpoints can also be configured.

Use HTTPS for remote providers. Plain HTTP is accepted only for `localhost` and literal loopback addresses. Base URLs may include `/v1` or the complete `/chat/completions` path; redirects are refused. Saved keys are scoped to their configured endpoint. `LINGO_API_KEY` overrides the saved key for any selected provider; `DEEPSEEK_API_KEY` is used only with DeepSeek's official HTTPS origin. Environment keys take precedence over Keychain.

## Command line

```sh
# Inspect local configuration and macOS permissions.
cargo run -- --doctor

# Translate text without interacting with another app.
cargo run -- --translate '你好，很高兴认识你。' --to Japanese

# Load a development key from a separate file.
cargo run -- --env-file /absolute/path/to/.env --translate '明天见。' --to English

# Save the configured provider's environment key to Keychain for app launches.
cargo run -- --env-file /absolute/path/to/.env --import-key
```

## Privacy and security

Translation sends the selected or pasted text, target language, and translation instructions to **the provider you configure**. Lingo is a desktop client; the default model runs remotely. Provider policies and billing apply.

Lingo does not maintain a persistent translation history. Source text and results remain in process memory while the app is running. Preferences are stored locally; saved credentials use macOS Keychain rather than the preferences file. Environment variables and explicitly loaded environment files are supported for development. API keys are not printed by diagnostics.

Accessibility access lets Lingo read and modify the selected editable control when invoked. For replacement checks, it keeps a temporary local snapshot of that field's full text; only the selection is sent to the model. It does not continuously translate ordinary text selection. Controls identified as secure or password fields are excluded. Guarded replacement reduces accidental edits, but app behavior varies; use the workbench for unsupported editors. See [architecture and trust boundaries](docs/architecture.md).

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
scripts/bundle-macos.sh
scripts/package-macos.sh
```

The initial verification includes **19 unit/mock tests**, **4 explicit live DeepSeek tests**, and manual workbench checks. GitHub CI passed formatting, lint, and tests on stable Rust, plus all-target compilation on the minimum supported Rust **1.88**. Live tests use separate credentials and are ignored by default. See [test evidence and reproduction steps](docs/testing.md) for exact coverage and remaining work.

The bundle script builds and ad-hoc signs a local `.app`. The package script builds the app and creates a versioned ZIP plus `SHA256SUMS.txt` in `dist/`. Developer ID signing and Apple notarization are separate distribution steps requiring the maintainer's Apple identity.

The **Build macOS bundle** GitHub Actions workflow can be run manually and also runs for `v*` tags. It uploads a ZIP for the runner's architecture and its checksum file as workflow artifacts; it does not automatically publish a GitHub Release or notarize the app.

Contributions to application compatibility, provider adapters, accessibility, and other desktop platforms are welcome. Please include the macOS version, affected app, reproduction steps, and redacted diagnostics in bug reports. Do not include private source text or API keys.

## License

[MIT](LICENSE) © 2026 Finn Fengming.
