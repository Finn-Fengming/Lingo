# Lingo

**Select text. Translate. Keep writing.**

Lingo is a small Rust desktop app that translates selected text inside supported macOS text fields with an LLM. Hold **Option** while dragging to select a passage, or select text and press **⌘⇧L**. English is the default target; choose Traditional Chinese, Japanese, or your own language in settings.

[简体中文](README.zh-CN.md) · [Design research](docs/research.md) · [Architecture](docs/architecture.md) · [Test evidence](docs/testing.md)

## What it does

- Translates existing text in place through macOS Accessibility APIs.
- Provides a workbench for pasting, translating, and copying text when an app does not support direct replacement.
- Uses an OpenAI-compatible Chat Completions endpoint. DeepSeek Flash is the default provider and model.
- Lets you configure the provider URL, model, and target language.
- Keeps the application in Rust, including its `egui` / `eframe` interface. No JavaScript runtime or traditional machine-translation service is required.

This is an early macOS release. Text-field compatibility depends on the host application's Accessibility implementation. Rich editors, terminals, protected fields, and custom canvas interfaces may not support selection or replacement. Use the workbench in those cases.

## Run from source

Install [Rust 1.88 or newer](https://www.rust-lang.org/tools/install) and Apple's command-line developer tools. From the repository:

```sh
cargo run --release
```

Configure your provider and API key in the app. For development, an explicit environment file is also supported:

```sh
cargo run --release -- --env-file /absolute/path/to/.env
```

The file should contain a key obtained from your provider:

```dotenv
DEEPSEEK_API_KEY=replace-with-your-own-key
```

Keep this file outside the repository. Never put a real key in an issue, screenshot, commit, or command-line argument. The repository does not contain a usable API key.

## Translate where you write

1. Launch Lingo and configure a key and target language.
2. In **System Settings → Privacy & Security**, enable **Accessibility** and **Input Monitoring** for the running app. After changing permission, restart Lingo if the trigger is still unavailable. When running from a terminal, macOS may list the terminal or development binary rather than the bundled app.
3. In another app, hold **Option**, drag across text in an editable field, and release. Or select text normally and press **⌘⇧L**.
4. Leave the field and selection unchanged while translation runs. Lingo checks the original selection before replacing it.

The drag gesture means **drag to select text**, not moving a text object into another window. Option-drag can have special behavior in some editors; use the keyboard shortcut or workbench there. Lingo never needs microphone access.

If the focused app or selection changes while translation runs, Lingo leaves the field alone and makes the result available in its workbench. Copy it when you are ready. If a field cannot be read or edited through Accessibility, paste its text into the workbench to translate it there. Review translations before sending important messages.

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
```

CI runs format, lint, and test checks on macOS without provider credentials. Live-provider and real-application interaction checks are separate from those automated checks. The bundle script builds a local `.app`; distributing a trusted macOS binary requires signing and notarization with your own Apple developer identity.

See [test evidence and reproduction steps](docs/testing.md) for verified results and remaining work. Cross-application gestures, replacement, and undo have **not yet been verified end to end**; they require macOS permissions on the test machine.

The **Build macOS bundle** GitHub Actions workflow can be run manually and also runs for `v*` tags. It uploads a ZIP for the runner's architecture as a workflow artifact; it does not publish a GitHub Release or notarize the app.

Contributions to application compatibility, provider adapters, accessibility, and other desktop platforms are welcome. Please include the macOS version, affected app, reproduction steps, and redacted diagnostics in bug reports. Do not include private source text or API keys.

## License

[MIT](LICENSE) © 2026 Finn Fengming.
