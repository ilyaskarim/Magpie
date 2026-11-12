//! The history window is created once at startup and then only shown and
//! hidden, never destroyed, so it appears instantly on the hotkey.

use tauri::{AppHandle, Manager, Runtime, Window, WindowEvent};

use crate::error::{AppError, AppResult};

pub const MAIN: &str = "main";

pub fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
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
