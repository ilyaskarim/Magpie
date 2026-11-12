//! The history window is created once at startup and then only shown and
//! hidden, never destroyed, so it appears instantly on the hotkey.

use tauri::{AppHandle, Emitter, Manager, Runtime, Window, WindowEvent};

use super::EVENT_WINDOW_SHOWN;
use super::error::{AppError, AppResult};

pub const MAIN: &str = "main";

/// Shows the history window. Every path that opens it (hotkey, tray, second
/// launch) goes through here, so the UI always gets `window-shown` (AD-17).
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
        if let Err(e) = app.emit_to(MAIN, EVENT_WINDOW_SHOWN, ()) {
            eprintln!("could not emit {EVENT_WINDOW_SHOWN}: {e}");
        }
    }
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.hide();
    }
}

pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let visible = app
        .get_webview_window(MAIN)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if visible { hide(app) } else { show(app) }
}

pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != MAIN {
        return;
    }
    match event {
        // Closing just hides; quitting is done from the tray menu.
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        // Behave like a popover: clicking elsewhere dismisses it.
        WindowEvent::Focused(false) => {
            let _ = window.hide();
        }
        _ => {}
    }
}

#[tauri::command]
pub fn hide_window(app: AppHandle) -> AppResult<()> {
    let w = app
        .get_webview_window(MAIN)
        .ok_or_else(|| AppError::Window("history window not found".into()))?;
    w.hide()?;
    Ok(())
}
