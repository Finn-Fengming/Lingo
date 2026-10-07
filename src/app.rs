use std::{
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, RichText, Stroke};
use lingo::{
    config::{self, Config},
    platform, provider,
};

use crate::tray::{Action, Tray};

const INK: Color32 = Color32::from_rgb(28, 47, 43);
const MUTED: Color32 = Color32::from_rgb(111, 125, 119);
const TEAL: Color32 = Color32::from_rgb(31, 112, 88);
const BG: Color32 = Color32::from_rgb(247, 248, 244);
const LINE: Color32 = Color32::from_rgb(220, 227, 219);

enum Origin {
    Local {
        original: String,
        range: Option<std::ops::Range<usize>>,
    },
    External(platform::SelectionSnapshot),
    ConnectionTest,
}

struct Pending {
    origin: Origin,
    started: Instant,
    auto_replace: bool,
    cancelled: bool,
}

pub struct LingoApp {
    config: Config,
    draft: Config,
    key_input: String,
    credential_ready: bool,
    settings: bool,
    source: String,
    output: String,
    selected: Option<std::ops::Range<usize>>,
    status: String,
    error: bool,
    paused: bool,
    trusted: bool,
    monitoring: bool,
    last_permission_check: Instant,
    pending: Option<Pending>,
    responses: Receiver<anyhow::Result<String>>,
    response_tx: mpsc::Sender<anyhow::Result<String>>,
    triggers: Receiver<platform::Trigger>,
    trigger_tx: mpsc::Sender<platform::Trigger>,
    listener: Option<platform::ListenerHandle>,
    replacement: Option<platform::Replacement>,
    tray: Option<Tray>,
    quitting: bool,
}

impl LingoApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        startup_warning: Option<String>,
    ) -> Self {
        configure_style(&cc.egui_ctx);
        let (response_tx, responses) = mpsc::channel();
        let (trigger_tx, native_events) = mpsc::channel();
        let (forward_tx, triggers) = mpsc::channel();
        let wake = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(event) = native_events.recv() {
                if forward_tx.send(event).is_err() {
                    break;
                }
                wake.request_repaint();
            }
        });
        let credential_ready = config::load_api_key(&config.provider).is_ok();
        let tray = Tray::new(&cc.egui_ctx).ok();
        let mut app = Self {
            draft: config.clone(),
            config,
            key_input: String::new(),
            credential_ready,
            settings: false,
            source: String::new(),
            output: String::new(),
            selected: None,
            status: "准备就绪。写下想说的话，或在其他应用中选中文字。".into(),
            error: false,
            paused: false,
            trusted: platform::is_accessibility_trusted(),
            monitoring: platform::is_input_monitoring_allowed(),
            last_permission_check: Instant::now(),
            pending: None,
            responses,
            response_tx,
            triggers,
            trigger_tx,
            listener: None,
            replacement: None,
            tray,
            quitting: false,
        };
        app.connect_listener();
        if let Some(warning) = startup_warning {
            app.settings = true;
            app.set_status(warning, true);
        }
        app
    }

    fn connect_listener(&mut self) {
        if self.trusted && self.monitoring && self.listener.is_none() {
            match platform::start_global_listener(self.trigger_tx.clone()) {
                Ok(listener) => {
                    listener.set_enabled(!self.paused);
                    listener.set_option_drag(self.config.option_drag);
                    self.listener = Some(listener);
                }
                Err(error) => self.set_status(error.to_string(), true),
            }
        }
    }

    fn set_status(&mut self, status: impl Into<String>, error: bool) {
        self.status = status.into();
        self.error = error;
        if let Some(tray) = &self.tray {
            tray.set_status(&self.status, self.pending.is_some(), error);
        }
    }

    fn start(&mut self, ctx: &egui::Context, text: String, origin: Origin) {
        if self.pending.is_some() {
            return;
        }
        let key = match config::load_api_key(&self.config.provider) {
            Ok(key) => key,
            Err(error) => {
                self.set_status(error.to_string(), true);
                return;
            }
        };
        let config = self.config.clone();
        let tx = self.response_tx.clone();
        let wake = ctx.clone();
        self.pending = Some(Pending {
            origin,
            started: Instant::now(),
            auto_replace: self.config.auto_replace,
            cancelled: false,
        });
        self.set_status(
            format!("正在翻译成 {}…", self.config.target_language),
            false,
        );
        std::thread::spawn(move || {
            let result = provider::translate(&config, &key, &text);
            let _ = tx.send(result);
            wake.request_repaint();
        });
    }

    fn translate_local(&mut self, ctx: &egui::Context) {
        let original = self.source.clone();
        let range = self
            .selected
            .clone()
            .filter(|r| r.start < r.end && r.end <= original.chars().count());
        let text = match &range {
            Some(range) => original
                .chars()
                .skip(range.start)
                .take(range.len())
                .collect(),
            None => original.clone(),
        };
        self.start(ctx, text, Origin::Local { original, range });
    }

    fn cancel(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.cancelled = true;
            self.set_status("已取消替换，正在等待当前请求结束。", false);
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if self.last_permission_check.elapsed() > Duration::from_secs(2) {
            self.trusted = platform::is_accessibility_trusted();
            self.monitoring = platform::is_input_monitoring_allowed();
            self.last_permission_check = Instant::now();
            self.connect_listener();
        }
        loop {
            let action = self
                .tray
                .as_ref()
                .and_then(|tray| tray.events.try_recv().ok());
            let Some(action) = action else {
                break;
            };
            match action {
                Action::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Action::Pause => {
                    self.paused = !self.paused;
                    if let Some(listener) = &self.listener {
                        listener.set_enabled(!self.paused);
                    }
                    if self.paused {
                        self.cancel();
                    }
                    self.set_status(
                        if self.paused {
                            "全局翻译已暂停"
                        } else {
                            "全局翻译已恢复"
                        },
                        false,
                    );
                }
                Action::Cancel => self.cancel(),
                Action::Undo => {
                    if let Some(replacement) = self.replacement.as_ref() {
                        match platform::undo_replacement(replacement) {
                            Ok(()) => {
                                self.replacement = None;
                                self.set_status("已恢复原文", false);
                            }
                            Err(error) => self.set_status(error.to_string(), true),
                        }
                    } else {
                        self.set_status("当前没有可撤销的替换", false);
                    }
                }
                Action::Quit => {
                    self.cancel();
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        while let Ok(trigger) = self.triggers.try_recv() {
            if self.paused || self.pending.is_some() {
                continue;
            }
            let selection = match trigger {
                platform::Trigger::SelectionDrag(result) => {
                    if !self.config.option_drag {
                        continue;
                    }
                    result
                }
                platform::Trigger::Shortcut(result) => result,
            };
            match selection {
                Ok(snapshot) => {
                    let text = snapshot.selected_text.clone();
                    self.start(ctx, text, Origin::External(snapshot));
                }
                Err(error) => self.set_status(error.to_string(), true),
            }
        }
        while let Ok(result) = self.responses.try_recv() {
            let Some(pending) = self.pending.take() else {
                continue;
            };
            if pending.cancelled {
                self.set_status("已取消。原文保持不变。", false);
                continue;
            }
            match result {
                Err(error) => self.set_status(error.to_string(), true),
                Ok(translated) => {
                    let elapsed = pending.started.elapsed().as_secs_f32();
                    self.output.clone_from(&translated);
                    match pending.origin {
                        Origin::ConnectionTest => self.set_status(
                            format!("连接成功 · 模型已返回译文 · {elapsed:.1}s"),
                            false,
                        ),
                        Origin::Local { original, range } => {
                            if pending.auto_replace
                                && self.source == original
                                && self.selected == range
                            {
                                let caret = range.as_ref().map_or(0, |r| r.start)
                                    + translated.chars().count();
                                self.source = replace_chars(&original, range, &translated);
                                self.selected = None;
                                let id = egui::Id::new("source-editor");
                                if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
                                    state.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::one(egui::text::CCursor::new(
                                            caret,
                                        )),
                                    ));
                                    state.store(ctx, id);
                                }
                                self.set_status(format!("已在输入框中替换 · {elapsed:.1}s"), false);
                            } else {
                                self.set_status(
                                    format!("译文已就绪 · {elapsed:.1}s · 可复制使用"),
                                    false,
                                );
                            }
                        }
                        Origin::External(snapshot) => {
                            if pending.auto_replace {
                                match platform::replace_selection(&snapshot, &translated) {
                                    Ok(replacement) => {
                                        self.replacement = Some(replacement);
                                        self.set_status(
                                            format!("已原位替换 · {elapsed:.1}s · 菜单栏可撤销"),
                                            false,
                                        );
                                    }
                                    Err(error) => self.set_status(
                                        format!("{error} 译文已保留，打开 Lingo 可复制。"),
                                        true,
                                    ),
                                }
                            } else {
                                self.set_status(
                                    format!("译文已就绪 · {elapsed:.1}s · 打开 Lingo 可复制"),
                                    false,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn workbench(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading(RichText::new("选中。翻译。继续。").size(30.0).color(INK));
        ui.add_space(8.0);
        ui.label(RichText::new("在原来的输入框里，换一种语言表达。 ").color(MUTED));
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("自动识别语言").color(MUTED));
            ui.label(RichText::new("  →  ").color(TEAL));
            let before = self.config.target_language.clone();
            language_picker(ui, &mut self.config.target_language, "target");
            if before != self.config.target_language {
                match self.config.save() {
                    Ok(()) => self
                        .draft
                        .target_language
                        .clone_from(&self.config.target_language),
                    Err(error) => self.set_status(error.to_string(), true),
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Cmd + Enter 翻译").small().color(MUTED));
            });
        });
        ui.add_space(14.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("试写区").strong().color(INK));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} 字", self.source.chars().count()))
                            .small()
                            .color(MUTED),
                    );
                });
            });
            ui.add_space(8.0);
            let editor = egui::TextEdit::multiline(&mut self.source)
                .id(egui::Id::new("source-editor"))
                .hint_text("写下或粘贴文字，也可以拖选其中一段。\n例如：你好，很高兴和你一起做这个开源项目。")
                .desired_width(f32::INFINITY).desired_rows(5).frame(false);
            let response = editor.show(ui);
            if let Some(range) = response.cursor_range {
                let sorted = range.as_sorted_char_range();
                self.selected = if sorted.is_empty() {
                    None
                } else {
                    Some(sorted)
                };
            }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let label = if self.pending.is_some() {
                "正在翻译…"
            } else if self.selected.is_some() {
                "翻译选中文字  ↗"
            } else {
                "翻译全文  ↗"
            };
            if ui
                .add_enabled(
                    self.pending.is_none() && !self.source.trim().is_empty(),
                    primary(label),
                )
                .clicked()
            {
                self.translate_local(ui.ctx());
            }
            if self.pending.is_some() {
                ui.spinner();
                if ui.button("取消").clicked() {
                    self.cancel();
                }
            } else {
                ui.label(
                    RichText::new(if self.config.auto_replace {
                        "完成后自动替换原文"
                    } else {
                        "仅预览，不替换原文"
                    })
                    .small()
                    .color(MUTED),
                );
            }
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("译文").strong().color(INK));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!self.output.is_empty(), egui::Button::new("复制译文"))
                        .clicked()
                    {
                        ui.ctx().copy_text(self.output.clone());
                        self.set_status("已复制译文", false);
                    }
                });
            });
            ui.add_space(6.0);
            if self.output.is_empty() {
                ui.label(RichText::new("译文会出现在这里。不会保存翻译历史。").color(MUTED));
                ui.add_space(20.0);
            } else {
                egui::ScrollArea::vertical()
                    .max_height(110.0)
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(&self.output).selectable(true).wrap());
                    });
            }
        });
        ui.add_space(18.0);
        if !self.trusted || !self.monitoring || !self.credential_ready {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("让 Lingo 随处可用").strong());
                        ui.label(
                            RichText::new(if !self.credential_ready {
                                "先在偏好设置中连接你的 AI 模型。"
                            } else {
                                "授权辅助功能和输入监控，即可原位翻译。"
                            })
                            .small()
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(if !self.credential_ready {
                                "连接模型"
                            } else {
                                "开启权限"
                            })
                            .clicked()
                        {
                            if !self.credential_ready {
                                self.settings = true;
                            } else {
                                self.open_missing_permission();
                            }
                        }
                    });
                });
            });
        } else {
            ui.label(
                RichText::new("全局快捷操作   Option + 拖选文字    /    Cmd + Shift + L 翻译选中")
                    .color(TEAL),
            );
            ui.label(
                RichText::new("关闭窗口后仍在菜单栏运行；翻译期间请保持原选区。 ")
                    .small()
                    .color(MUTED),
            );
        }
    }

    fn preferences(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading(RichText::new("按你的习惯来。").size(28.0).color(INK));
        ui.add_space(8.0);
        ui.label(RichText::new("自带密钥，直连模型。没有中转服务。").color(MUTED));
        ui.add_space(18.0);
        card(ui, |ui| {
            ui.label(RichText::new("翻译偏好").strong());
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("默认目标语言");
                language_picker(ui, &mut self.draft.target_language, "settings-target");
            });
            ui.horizontal(|ui| {
                ui.label("自定义语言");
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft.target_language)
                        .desired_width(240.0)
                        .hint_text("例如：Korean / 简体中文"),
                );
            });
            ui.add_space(6.0);
            ui.checkbox(&mut self.draft.option_drag, "按住 Option 拖选文字后翻译");
            ui.checkbox(
                &mut self.draft.auto_replace,
                "完成后原位替换（关闭则只在 Lingo 中预览）",
            );
            ui.label(
                RichText::new("Cmd + Shift + L 始终可用。选区或内容发生变化时，Lingo 会停止替换。")
                    .small()
                    .color(MUTED),
            );
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("AI 服务").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("恢复 DeepSeek 默认值").clicked() {
                        self.draft.provider = Config::default().provider;
                    }
                });
            });
            ui.add_space(10.0);
            egui::Grid::new("provider-grid")
                .num_columns(2)
                .spacing([14.0, 10.0])
                .show(ui, |ui| {
                    ui.label("API 地址");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.draft.provider.base_url)
                            .desired_width(420.0),
                    );
                    ui.end_row();
                    ui.label("模型名称");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.draft.provider.model)
                            .desired_width(420.0),
                    );
                    ui.end_row();
                    ui.label("超时（秒）");
                    ui.add(
                        egui::DragValue::new(&mut self.draft.provider.timeout_secs).range(5..=120),
                    );
                    ui.end_row();
                    ui.label("API Key");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.key_input)
                            .password(true)
                            .desired_width(420.0)
                            .hint_text("填入新密钥；留空保留现有密钥"),
                    );
                    ui.end_row();
                });
            ui.add_space(10.0);
            ui.label(
                RichText::new("支持 OpenAI 兼容的 Chat Completions。密钥保存在 macOS 钥匙串。")
                    .small()
                    .color(MUTED),
            );
            ui.label(
                RichText::new("只发送你选中的文本。服务商按其自身政策处理请求。")
                    .small()
                    .color(MUTED),
            );
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.pending.is_none(), primary("保存设置"))
                .clicked()
            {
                self.save_settings();
            }
            if ui
                .add_enabled(
                    self.pending.is_none(),
                    egui::Button::new("测试已保存的连接").min_size(egui::vec2(140.0, 36.0)),
                )
                .clicked()
            {
                self.start(
                    ui.ctx(),
                    "你好，很高兴认识你。".into(),
                    Origin::ConnectionTest,
                );
            }
            if ui.button("检查系统权限").clicked() {
                self.open_missing_permission();
            }
        });
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!(
                "辅助功能：{}    输入监控：{}",
                if self.trusted {
                    "已开启"
                } else {
                    "未开启"
                },
                if self.monitoring {
                    "已开启"
                } else {
                    "未开启"
                }
            ))
            .small()
            .color(MUTED),
        );
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.small_button("移除已保存的密钥").clicked() {
                match config::delete_api_key(&self.config.provider) {
                    Ok(()) => {
                        self.credential_ready = config::load_api_key(&self.config.provider).is_ok();
                        self.set_status("已移除钥匙串密钥。环境变量中的密钥不受影响。", false);
                    }
                    Err(error) => self.set_status(error.to_string(), true),
                }
            }
            ui.label(RichText::new("环境变量优先于钥匙串。").small().color(MUTED));
        });
    }

    fn open_missing_permission(&mut self) {
        let result = if !platform::is_accessibility_trusted() {
            platform::request_accessibility_permission();
            platform::open_accessibility_settings()
        } else {
            platform::request_input_monitoring_permission();
            platform::open_input_monitoring_settings()
        };
        if let Err(error) = result {
            self.set_status(error.to_string(), true);
        }
    }

    fn save_settings(&mut self) {
        let result = (|| -> anyhow::Result<()> {
            self.draft.validate()?;
            if !self.key_input.trim().is_empty() {
                config::save_api_key(&self.draft.provider, self.key_input.trim())?;
                self.key_input.clear();
            }
            self.draft.save()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.config = self.draft.clone();
                if let Some(listener) = &self.listener {
                    listener.set_option_drag(self.config.option_drag);
                }
                self.credential_ready = config::load_api_key(&self.config.provider).is_ok();
                self.set_status("设置已保存", false);
            }
            Err(error) => self.set_status(error.to_string(), true),
        }
    }
}

impl eframe::App for LingoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel();
        }
        self.poll(ctx);
        // Low-frequency wakeups drain native selection events even while the window is hidden.
        ctx.request_repaint_after(Duration::from_secs(2));
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
            && !self.settings
        {
            self.translate_local(ctx);
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(28, 20)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Li")
                            .strong()
                            .size(25.0)
                            .color(Color32::WHITE)
                            .background_color(TEAL),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new("Lingo").strong().size(24.0).color(INK));
                    ui.add_space(10.0);
                    ui.label(RichText::new("语言，随手切换。").size(13.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.selectable_value(&mut self.settings, true, "偏好设置");
                        ui.selectable_value(&mut self.settings, false, "翻译");
                    });
                });
            });
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(28, 15)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("●").color(if self.error {
                        Color32::from_rgb(174, 88, 55)
                    } else {
                        TEAL
                    }));
                    ui.label(RichText::new(&self.status).size(12.0).color(if self.error {
                        Color32::from_rgb(155, 69, 40)
                    } else {
                        MUTED
                    }));
                });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(28, 8)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.settings {
                        self.preferences(ui);
                    } else {
                        self.workbench(ui);
                    }
                });
            });
    }
}

fn configure_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for path in [
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ] {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".into(), FontData::from_owned(data).into());
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .push("cjk".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.selection.bg_fill = Color32::from_rgb(201, 225, 207);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, TEAL);
    style.visuals.widgets.inactive.bg_fill = Color32::WHITE;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.widgets.inactive.corner_radius = 7.into();
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(229, 237, 227);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(207, 227, 211);
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    ctx.set_style(style);
}

fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE))
        .fill(TEAL)
        .min_size(egui::vec2(145.0, 38.0))
        .corner_radius(8)
}

fn card<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(12)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contents(ui)
        })
}

fn language_picker(ui: &mut egui::Ui, target: &mut String, id: &str) {
    let languages = [
        ("English", "English · 英语"),
        ("Traditional Chinese", "繁體中文"),
        ("Japanese", "日本語 · 日语"),
        ("Simplified Chinese", "简体中文"),
        ("Korean", "한국어 · 韩语"),
        ("French", "Français · 法语"),
        ("German", "Deutsch · 德语"),
        ("Spanish", "Español · 西班牙语"),
    ];
    let current = languages
        .iter()
        .find(|(value, _)| *value == target)
        .map(|(_, label)| *label)
        .unwrap_or(target);
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(170.0)
        .show_ui(ui, |ui| {
            for (value, label) in languages {
                ui.selectable_value(target, value.into(), label);
            }
        });
}

fn replace_chars(
    original: &str,
    range: Option<std::ops::Range<usize>>,
    translated: &str,
) -> String {
    match range {
        None => translated.into(),
        Some(range) => {
            original.chars().take(range.start).collect::<String>()
                + translated
                + &original.chars().skip(range.end).collect::<String>()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacing_a_selection_preserves_unicode_surroundings() {
        assert_eq!(
            replace_chars("嗨👋，你好！", Some(3..5), "Hello"),
            "嗨👋，Hello！"
        );
    }
}
