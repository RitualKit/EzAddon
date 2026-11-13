//! System tray: reopening the window, the tray menu, and background update
//! checks with a Windows notification.
//!
//! When the window is closed in tray mode it is destroyed, not hidden, so the
//! WebView2 processes exit and only this small Rust process stays running.
//! The window is rebuilt from the config when it's reopened.

use crate::{compute_updates, AppState, UpdateInfo};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::NotificationExt;

const TRAY_ID: &str = "main";
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

static AVAILABLE: AtomicBool = AtomicBool::new(false);

/// True once the tray icon exists, so closing the window can't strand the app.
pub fn is_available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}

struct TrayState {
    updates_item: MenuItem<Wry>,
    /// (mod id, file id) pairs already announced, so each update notifies once.
    notified: Mutex<HashSet<(u32, u32)>>,
}

pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open EzAddon", true, None::<&str>)?;
    let updates = MenuItem::with_id(app, "updates", "Check for updates", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit EzAddon", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &updates, &PredefinedMenuItem::separator(app)?, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("EzAddon")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "updates" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { run_check(&app, true).await });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    AVAILABLE.store(true, Ordering::Relaxed);

    app.manage(TrayState { updates_item: updates, notified: Mutex::new(HashSet::new()) });

    // Background checks. A sleeping timer costs nothing between runs.
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            if handle.state::<AppState>().settings().notify_updates {
                run_check(&handle, false).await;
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
    Ok(())
}

/// Shows the main window, recreating it if it was closed to the tray.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    if let Some(cfg) = app.config().app.windows.first().cloned() {
        if let Ok(w) = tauri::WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
            let _ = w.set_focus();
        }
    }
}

/// Puts the number of available updates in the tray tooltip and menu.
pub fn show_update_count(app: &AppHandle, count: usize) {
    let (tip, item) = match count {
        0 => ("EzAddon".to_string(), "Check for updates".to_string()),
        1 => ("EzAddon: 1 addon update available".to_string(), "1 update available, check again".to_string()),
        n => (format!("EzAddon: {n} addon updates available"), format!("{n} updates available, check again")),
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tip));
    }
    if let Some(state) = app.try_state::<TrayState>() {
        let _ = state.updates_item.set_text(item);
    }
}

/// Checks for updates, then refreshes the tray, tells an open window, and
/// shows a notification for new updates (or always, when `manual`).
async fn run_check(app: &AppHandle, manual: bool) {
    let state = app.state::<AppState>();
    let list: Vec<UpdateInfo> = match compute_updates(&state).await {
        Ok(l) => l,
        Err(e) => {
            if manual {
                notify(app, &format!("Couldn't check for updates: {e}"));
            }
            return;
        }
    };
    show_update_count(app, list.len());
    let _ = app.emit("updates-available", &list);

    let window_visible = app
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
        .unwrap_or(false);
    let fresh = {
        let tray = app.state::<TrayState>();
        let mut seen = tray.notified.lock().unwrap();
        list.iter().filter(|u| seen.insert((u.mod_id, u.latest_file_id))).count()
    };
    if manual {
        notify(app, &match list.len() {
            0 => "All your addons are up to date.".to_string(),
            1 => "1 addon update is available.".to_string(),
            n => format!("{n} addon updates are available."),
        });
    } else if fresh > 0 && !window_visible {
        notify(app, &match list.len() {
            1 => "1 addon update is available. Open EzAddon to install it.".to_string(),
            n => format!("{n} addon updates are available. Open EzAddon to install them."),
        });
    }
}

fn notify(app: &AppHandle, body: &str) {
    let _ = app.notification().builder().title("EzAddon").body(body).show();
}
