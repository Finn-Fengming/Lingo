use std::sync::mpsc::{Receiver, channel};

#[derive(Debug)]
pub enum Action {
    Show,
    Undo,
    Pause,
    Cancel,
    Quit,
}

pub struct Tray {
    pub events: Receiver<Action>,
    #[cfg(target_os = "macos")]
    icon: tray_icon::TrayIcon,
    #[cfg(target_os = "macos")]
    status: tray_icon::menu::MenuItem,
}

impl Tray {
    pub fn new(ctx: &eframe::egui::Context) -> anyhow::Result<Self> {
        let (tx, events) = channel();
        #[cfg(target_os = "macos")]
        {
            use tray_icon::{
                TrayIconBuilder,
                menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
            };
            let menu = Menu::new();
            let status = MenuItem::new("Lingo · Ready", false, None);
            let show = MenuItem::new("打开 Lingo / Open", true, None);
            let undo = MenuItem::new("撤销上次替换 / Undo", true, None);
            let pause = MenuItem::new("暂停 / 恢复全局翻译", true, None);
            let cancel = MenuItem::new("取消当前翻译 / Cancel", true, None);
            let quit = MenuItem::new("退出 Lingo / Quit", true, None);
            menu.append_items(&[
                &status,
                &PredefinedMenuItem::separator(),
                &show,
                &undo,
                &pause,
                &cancel,
                &PredefinedMenuItem::separator(),
                &quit,
            ])?;
            let show_id = show.id().clone();
            let undo_id = undo.id().clone();
            let pause_id = pause.id().clone();
            let cancel_id = cancel.id().clone();
            let quit_id = quit.id().clone();
            let wake = ctx.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let action = if event.id == show_id {
                    Action::Show
                } else if event.id == undo_id {
                    Action::Undo
                } else if event.id == pause_id {
                    Action::Pause
                } else if event.id == cancel_id {
                    Action::Cancel
                } else if event.id == quit_id {
                    Action::Quit
                } else {
                    return;
                };
                let _ = tx.send(action);
                wake.request_repaint();
            }));
            let icon = TrayIconBuilder::new()
                .with_title("Lingo")
                .with_tooltip("Lingo · Option-drag or ⌘⇧L")
                .with_menu(Box::new(menu))
                .build()?;
            Ok(Self {
                events,
                icon,
                status,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (tx, events, ctx);
            anyhow::bail!("A native menu bar is currently available only on macOS")
        }
    }

    pub fn set_status(&self, text: &str, busy: bool, failed: bool) {
        #[cfg(target_os = "macos")]
        {
            self.status.set_text(text);
            self.icon.set_title(Some(if busy {
                "Lingo ···"
            } else if failed {
                "Lingo !"
            } else {
                "Lingo"
            }));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (text, busy, failed);
    }
}
