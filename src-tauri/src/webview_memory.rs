//! Asks WebView2 to trim its memory while the window is minimized and to go
//! back to normal when it's restored. Does nothing on other platforms.

use std::sync::atomic::{AtomicBool, Ordering};

static LOW: AtomicBool = AtomicBool::new(false);

pub fn set_low(window: &tauri::Window, low: bool) {
    // Resize events fire often; only act when the state actually changes.
    if LOW.swap(low, Ordering::Relaxed) == low {
        return;
    }
    apply(window, low);
}

#[cfg(windows)]
fn apply(window: &tauri::Window, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;

    for webview in window.webviews() {
        let _ = webview.with_webview(move |pw| unsafe {
            let Ok(core) = pw.controller().CoreWebView2() else { return };
            // Older WebView2 runtimes don't have this interface; skip quietly.
            if let Ok(core19) = core.cast::<ICoreWebView2_19>() {
                let level = if low {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                } else {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                };
                let _ = core19.SetMemoryUsageTargetLevel(level);
            }
        });
    }
}

#[cfg(not(windows))]
fn apply(_window: &tauri::Window, _low: bool) {}
