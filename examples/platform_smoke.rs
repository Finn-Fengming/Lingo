//! Manual macOS integration test against a disposable editor document.
//!
//! Create a fresh TextEdit document containing `LINGO_SMOKE_你好，世界！` and
//! select that exact text. Run this example, then focus the document before the
//! initial delay expires. Never run it against a real document.
//!
//! cargo run --example platform_smoke -- --mode replace-undo --delay 5
//! cargo run --example platform_smoke -- --mode changed --delay 5
//!
//! In `changed` mode, change the selection during the second delay. Only fixed
//! fixture text is ever written; editor content is never printed.

use lingo::platform::{self, ErrorKind};
use std::{process::ExitCode, thread, time::Duration};

const FIXTURE: &str = "LINGO_SMOKE_你好，世界！";
const TRANSLATED: &str = "LINGO_SMOKE_Hello, world!";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut mode = "replace-undo".to_owned();
    let mut delay = 5_u64;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--mode" => mode = args.next().ok_or("Missing --mode value")?,
            "--delay" => {
                delay = args
                    .next()
                    .ok_or("Missing --delay value")?
                    .parse()
                    .map_err(|_| "--delay must be an integer between 1 and 120")?;
                if !(1..=120).contains(&delay) {
                    return Err("--delay must be between 1 and 120".into());
                }
            }
            "--help" | "-h" => {
                println!(
                    "Manual disposable TextEdit test: --mode replace-undo|changed --delay SECONDS. Select the exact fixture documented in examples/platform_smoke.rs, then focus the editor during the first delay. In changed mode, move the selection during the second delay."
                );
                return Ok(());
            }
            _ => return Err("Unknown argument; use --help".into()),
        }
    }
    if !matches!(mode.as_str(), "replace-undo" | "changed") {
        return Err("--mode must be replace-undo or changed".into());
    }
    if !platform::is_accessibility_trusted() {
        return Err(
            "Accessibility permission is required for this executable or its launching app.".into(),
        );
    }
    println!("Focus the disposable editor fixture within {delay} seconds.");
    thread::sleep(Duration::from_secs(delay));
    let snapshot = platform::capture_selection().map_err(|error| error.to_string())?;
    if snapshot.selected_text != FIXTURE {
        return Err(
            "Selected text does not match the fixed smoke-test fixture; no write was attempted."
                .into(),
        );
    }
    println!("PASS: captured the expected fixture selection.");

    if mode == "changed" {
        println!("Move the selection or edit the fixture within {delay} seconds.");
        thread::sleep(Duration::from_secs(delay));
        match platform::replace_selection(&snapshot, TRANSLATED) {
            Err(error) if error.kind == ErrorKind::Changed => {
                println!("PASS: changed input rejected before replacement.");
                return Ok(());
            }
            Err(error) => return Err(format!("Unexpected rejection: {error}")),
            Ok(replacement) => {
                platform::undo_replacement(&replacement).map_err(|error| {
                    format!("No change was detected, and restoring the fixture failed: {error}")
                })?;
                return Err("No input change was detected; replacement was restored. Repeat and change the selection during the second delay.".into());
            }
        }
    }

    let replacement =
        platform::replace_selection(&snapshot, TRANSLATED).map_err(|error| error.to_string())?;
    println!("PASS: in-place replacement verified against the entire local value.");
    thread::sleep(Duration::from_millis(250));
    platform::undo_replacement(&replacement).map_err(|error| error.to_string())?;
    println!("PASS: undo restored the original local value.");
    Ok(())
}
