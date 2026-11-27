//! Background operation: system tray, close-to-tray, start minimized, launch
//! at login, and native notifications for new messages while MelloGram is
//! hidden or unfocused. Tray and autostart exist on desktop only; native
//! notifications also work on Android through the notification plugin.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Timelike as _;
use mellogram_core::background::notification_for;
use mellogram_core::IncomingMessage;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt as _;

use crate::state::AppState;

/// Chat titles learned from the chat list, for notification titles.
#[derive(Default)]
pub struct BackgroundState {
    pub chat_titles: Mutex<HashMap<i64, String>>,
    /// Notifications shown since the window was last focused.
    pub unseen: AtomicU32,
}

fn main_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("main")
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = main_window(app) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn window_focused(app: &AppHandle) -> bool {
    main_window(app).is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
}

/// Raises a native notification for `msg` if the policy allows it.
pub fn notify_message(state: &AppState, msg: &IncomingMessage) {
    let settings = state.store.lock().unwrap().settings.notifications.clone();
    let title = state.background.chat_titles.lock().unwrap().get(&msg.chat_id).cloned();
    let hour = chrono::Local::now().hour() as u8;
    let Some(content) = notification_for(&settings, msg, title.as_deref(), window_focused(&state.app), hour) else {
        return;
    };
    if let Err(e) = state.app.notification().builder().title(content.title).body(content.body).show() {
        tracing::debug!("notification failed: {e}");
        return;
    }
    let n = state.background.unseen.fetch_add(1, Ordering::Relaxed) + 1;
    update_tray_tooltip(&state.app, n);
}

/// Clears the unseen counter when the user comes back.
pub fn window_focused_again(app: &AppHandle) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        state.background.unseen.store(0, Ordering::Relaxed);
    }
    update_tray_tooltip(app, 0);
}

#[cfg(desktop)]
fn update_tray_tooltip(app: &AppHandle, unseen: u32) {
    if let Some(tray) = app.tray_by_id("main") {
        let text = if unseen == 0 { "MelloGram".to_owned() } else { format!("MelloGram — {unseen} new") };
        let _ = tray.set_tooltip(Some(text));
    }
}

#[cfg(not(desktop))]
fn update_tray_tooltip(_: &AppHandle, _: u32) {}

/// Applies launch-at-login to the OS to match settings.
#[cfg(desktop)]
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt as _;
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    if let Err(e) = result {
        tracing::warn!("could not update launch-at-login: {e}");
    }
}

#[cfg(not(desktop))]
pub fn apply_autostart(_: &AppHandle, _: bool) {}

/// Tray icon, autostart sync and initial window visibility. Call from setup.
pub fn setup(app: &AppHandle, state: &Arc<AppState>) -> tauri::Result<()> {
    let general = state.store.lock().unwrap().settings.general.clone();
    let started_hidden = std::env::args().any(|a| a == "--minimized");

    #[cfg(desktop)]
    {
        build_tray(app, state)?;
        apply_autostart(app, general.launch_at_login);
    }

    // The window is created hidden (tauri.conf.json) so start-minimized does
    // not flash; show it unless the user wants to start in the tray.
    let hide = cfg!(desktop) && (general.start_minimized || started_hidden);
    if !hide {
        show_main(app);
    }
    Ok(())
}

#[cfg(desktop)]
fn build_tray(app: &AppHandle, state: &Arc<AppState>) -> tauri::Result<()> {
    use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let dnd_on = state.store.lock().unwrap().settings.notifications.dnd;
    let show = MenuItem::with_id(app, "show", "Show MelloGram", true, None::<&str>)?;
    let dnd = CheckMenuItem::with_id(app, "dnd", "Do Not Disturb", true, dnd_on, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "Lock now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit MelloGram", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &dnd, &lock, &sep, &quit])?;

    let dnd_item = dnd.clone();
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("MelloGram")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "dnd" => {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    let on = {
                        let mut store = state.store.lock().unwrap();
                        store.settings.notifications.dnd = !store.settings.notifications.dnd;
                        store.settings.notifications.dnd
                    };
                    let _ = state.save();
                    let _ = dnd_item.set_checked(on);
                    let _ = app.emit("mg://dnd-changed", on);
                }
            }
            "lock" => {
                let _ = app.emit("mg://lock-now", ());
            }
            "quit" => {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    state.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                match main_window(app) {
                    Some(w) if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) => {
                        let _ = w.hide();
                    }
                    _ => show_main(app),
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Close-to-tray: returns `true` when the close was turned into a hide.
pub fn intercept_close(window: &tauri::Window) -> bool {
    if !cfg!(desktop) {
        return false;
    }
    let Some(state) = window.app_handle().try_state::<Arc<AppState>>() else { return false };
    let to_tray = state.store.lock().unwrap().settings.general.close_to_tray;
    if to_tray {
        let _ = window.hide();
    }
    to_tray
}
