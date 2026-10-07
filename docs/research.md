# Interaction research

Research date: **2026-10-07**. These notes describe the public official pages inspected on that date, not an exhaustive hands-on comparison. The requested name “TypeList” is interpreted here as **Typeless**, the AI voice-dictation product; that identification is an assumption.

## What the references establish

| Product | Officially documented behavior | Relevant lesson for Lingo |
| --- | --- | --- |
| Doubao Input Method | Its desktop page describes speech recognition, LLM-assisted typing, and organization of spoken text; it offers a macOS download. | Keep the user's attention in the field where they are writing, and make the model invocation a small action. |
| Typeless Dictate | The desktop guide starts with focusing a text field, then pressing a shortcut to begin and again to finish. On macOS the documented default is Fn. | A system-level trigger can connect an existing text field to a model without a separate chat conversation. |
| Typeless Translate | The guide documents configurable target languages and a desktop translation shortcut. Spoken input is translated and placed in the starting field. | Store the preferred target language and make changing it straightforward. |

Sources: [Doubao desktop page](https://shurufa.doubao.com/pc), [Typeless Dictate guide](https://www.typeless.com/help/quickstart/dictate), [Typeless Translate guide](https://www.typeless.com/help/quickstart/translate).

The current Typeless desktop instructions describe a **press-to-start / press-to-stop** interaction. The user's hold-a-key analogy captures the desire for a short invocation, but should not be presented as a verified universal interaction across these products. This research does not validate their latency, translation quality, application-compatibility claims, or privacy promises.

## Lingo's scope

Lingo begins with **text already written in a field**, rather than speech. Its core action is:

1. Hold Option and drag to select a passage, or use ordinary selection followed by ⌘⇧L.
2. Read that selected text and send it to the configured LLM with the target language.
3. Replace the selection only if the original editable control and selection are still valid.
4. Make the result available for manual copying if direct insertion cannot safely finish.

Requiring a modifier on the drag gesture prevents ordinary text selection from making a network request. The keyboard path is useful in editors that interpret Option-drag differently. The workbench is needed for applications whose selection or edit controls are not accessible. These are product decisions derived from the request and macOS constraints, rather than claims that competitors use the same implementation.

English is the initial target. Traditional Chinese, Japanese, and a custom target make the setting useful without forcing a new prompt on every invocation. The interface should keep the default workflow short while exposing connection settings separately.

## Model and protocol

DeepSeek's official guide documents an OpenAI-compatible API with base URL `https://api.deepseek.com`, `POST /chat/completions`, and the model name `deepseek-flash`. Its 2026-09-10 update identifies this name with V4.1 Flash; older Flash identifiers are compatibility aliases at the time of research. Lingo uses the documented current name and keeps it configurable. [API guide](https://api-docs.deepseek.com/guides/codex), [DeepSeek change log](https://api-docs.deepseek.com/updates/).

A standard messages-based request is sufficient for translation. The model receives explicit translation instructions and source text as separate messages, and the client consumes the textual answer. No speech-recognition pipeline or traditional translation API is part of this design. OpenAI-compatible providers can be substituted by changing the base URL, model, and credential; provider-specific options should remain optional and isolated.

## Platform implications

macOS Accessibility exposes UI elements and their attributes to assistive applications. Actual attributes and write support vary by target application, so “works in every app” would be an unsupported claim for this release. Native field integration must fail conservatively when the selection, focus, or document changes. [Apple AXUIElement documentation](https://developer.apple.com/documentation/applicationservices/axuielement).

The initial delivery targets macOS because that is the development and verification environment. A pure Rust core leaves room for other platform adapters, but Windows and Linux system-wide integration are outside the initial implementation.
