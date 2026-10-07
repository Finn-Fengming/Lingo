mod app;
mod tray;

use anyhow::{Context, Result, bail};
use lingo::{config, platform, provider};

fn main() {
    if let Err(error) = run() {
        eprintln!("Lingo: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut source = None;
    let mut target = None;
    let mut doctor = false;
    let mut import = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--help" | "-h" => {
                println!(
                    "Lingo — select, translate, keep writing.\n\n  lingo                         Open desktop app\n  lingo --translate TEXT        Translate from the terminal\n        --to LANGUAGE           Override target language\n        --env-file PATH         Load an explicit local .env file\n  lingo --import-key            Save configured provider key to macOS Keychain\n  lingo --doctor                Check permissions and configuration\n\nDesktop: Option-drag selects and translates; Cmd+Shift+L translates a selection.\nOnly the selected text is sent to your configured model."
                );
                return Ok(());
            }
            "--env-file" | "--translate" | "--to" => {
                let name = &args[index];
                index += 1;
                let value = args
                    .get(index)
                    .context("Missing command-line option value")?;
                match name.as_str() {
                    "--env-file" => {
                        // Initialization happens before GUI/worker threads exist. Never echo values.
                        dotenvy::from_path(value)
                            .map_err(|_| anyhow::anyhow!("Could not load the environment file"))?;
                    }
                    "--translate" => source = Some(value.clone()),
                    _ => target = Some(value.clone()),
                }
            }
            "--doctor" => doctor = true,
            "--import-key" => import = true,
            _ => bail!("Unknown option. Run lingo --help."),
        }
        index += 1;
    }
    let (mut config, startup_warning) = match config::Config::load() {
        Ok(config) => (config, None),
        Err(error) if source.is_none() && !doctor && !import => (
            config::Config::default(),
            Some(format!(
                "无法读取设置，已临时使用默认值。请检查后保存。{error}"
            )),
        ),
        Err(error) => return Err(error),
    };
    if let Some(language) = target {
        config.target_language = language;
    }
    config.validate()?;
    if doctor {
        println!(
            "Platform: {} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        println!(
            "Accessibility: {}",
            if platform::is_accessibility_trusted() {
                "granted"
            } else {
                "not granted"
            }
        );
        println!(
            "Input Monitoring: {}",
            if platform::is_input_monitoring_allowed() {
                "granted"
            } else {
                "not granted"
            }
        );
        println!("Provider configuration: valid");
        println!(
            "API credential: {}",
            if config::load_api_key(&config.provider).is_ok() {
                "available"
            } else {
                "missing"
            }
        );
        return Ok(());
    }
    if import {
        let key = config::load_api_key(&config.provider)?;
        config::save_api_key(&config.provider, &key)?;
        println!("API key saved to the configured provider's Keychain entry.");
        return Ok(());
    }
    if let Some(text) = source {
        let key = config::load_api_key(&config.provider)?;
        println!("{}", provider::translate(&config, &key, &text)?);
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([780.0, 710.0])
            .with_min_inner_size([660.0, 600.0])
            .with_title("Lingo")
            .with_app_id("io.github.finn-fengming.lingo"),
        ..Default::default()
    };
    eframe::run_native(
        "Lingo",
        options,
        Box::new(move |cc| Ok(Box::new(app::LingoApp::new(cc, config, startup_warning)))),
    )
    .map_err(|_| anyhow::anyhow!("Could not open the desktop window"))
}
