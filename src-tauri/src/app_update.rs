//! Keeps EzAddon itself up to date from the project's GitHub releases.
//!
//! Every CI build publishes a signed installer plus `latest.json` to the
//! rolling "latest" release. The updater only installs a build whose
//! signature matches the public key in `tauri.conf.json`.

use crate::error::{AppError, AppResult};
use crate::AppState;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

const FIRST_CHECK_AFTER: Duration = Duration::from_secs(5);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}

/// Progress of an app update download, sent as the `app-update-progress` event.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    received: u64,
    total: Option<u64>,
}

fn err(e: tauri_plugin_updater::Error) -> AppError {
    AppError::Other(format!("App update failed: {e}"))
}

/// Whether updates make sense for this build. Local development builds are
/// skipped so they don't replace themselves with a release.
fn enabled() -> bool {
    !cfg!(debug_assertions)
}

pub async fn check(app: &AppHandle) -> AppResult<Option<AppUpdate>> {
    if !enabled() {
        return Ok(None);
    }
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    Ok(update.map(|u| AppUpdate { version: u.version.clone(), current_version: u.current_version.clone(), notes: u.body.clone() }))
}

/// Downloads and installs the latest update. On Windows the installer runs
/// in passive mode, closes EzAddon and starts the new version.
pub async fn install(app: &AppHandle) -> AppResult<bool> {
    if !enabled() {
        return Ok(false);
    }
    let Some(update) = app.updater().map_err(err)?.check().await.map_err(err)? else {
        return Ok(false);
    };
    let _ = app.emit("app-update", serde_json::json!({ "state": "installing", "version": update.version }));
    let mut received = 0u64;
    let handle = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                received += chunk as u64;
                let _ = handle.emit("app-update-progress", UpdateProgress { received, total });
            },
            || {},
        )
        .await
        .map_err(err)?;
    // Reached on platforms where the installer doesn't restart the app.
    app.restart();
}

/// Checks shortly after launch and then every few hours. With automatic
/// updates on, an update found at launch is installed right away; later
/// ones are announced and installed when the user chooses.
pub fn start_background_checks(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        let mut first = true;
        loop {
            if let Ok(Some(update)) = check(&app).await {
                let auto = app.state::<AppState>().settings().auto_update_app;
                if first && auto {
                    if let Err(e) = install(&app).await {
                        let _ = app.emit("app-update", serde_json::json!({ "state": "error", "message": e.to_string() }));
                    }
                } else {
                    let _ = app.emit("app-update", serde_json::json!({ "state": "available", "version": update.version }));
                    let window_open = app.get_webview_window("main").is_some();
                    if !window_open {
                        let _ = app
                            .notification()
                            .builder()
                            .title("EzAddon update available")
                            .body(format!("EzAddon {} is ready to install. Open EzAddon to update.", update.version))
                            .show();
                    }
                }
            }
            first = false;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> AppResult<Option<AppUpdate>> {
    check(&app).await
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> AppResult<bool> {
    install(&app).await
}
