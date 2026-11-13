//! The history window is created once at startup and then only shown and
//! hidden, never destroyed, so it appears instantly on the hotkey.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime, Window, WindowEvent};

use super::EVENT_WINDOW_SHOWN;
use super::error::{AppError, AppResult};
use super::state::AppState;
use crate::adapters::SystemFrontmost;
use crate::domain::TargetApp;
use crate::ports::Frontmost;
use crate::services::capture::lock;

pub const MAIN: &str = "main";

/// Shows the history window. Every path that opens it (hotkey, tray, second
/// launch) goes through here, so the UI always gets `window-shown` (AD-17).
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    remember_frontmost(app);
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
        if let Err(e) = app.emit_to(MAIN, EVENT_WINDOW_SHOWN, ()) {
            eprintln!("could not emit {EVENT_WINDOW_SHOWN}: {e}");
        }
    }
}

/// Saves the paste-back target (AD-12) before Magpie takes focus.
fn remember_frontmost<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<AppState>() {
        remember(&state.target, &SystemFrontmost);
    }
}

/// The rule behind [`remember_frontmost`]: a known frontmost app replaces the
/// target; when Magpie itself is frontmost or the app is unknown (`None`),
/// the target saved earlier in this show is kept. The target is cleared on
/// every hide, so it never outlives the show it was saved for.
pub fn remember(target: &Mutex<Option<TargetApp>>, front: &dyn Frontmost) {
    if let Some(app) = front.app() {
        *lock(target) = Some(app);
    }
}

/// Forgets the paste-back target; called whenever the window hides.
fn forget_target<R: Runtime, M: Manager<R>>(app: &M) {
    if let Some(state) = app.try_state::<AppState>() {
        *lock(&state.target) = None;
    }
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.hide();
    }
    forget_target(app);
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
            forget_target(window);
        }
        // Behave like a popover: clicking elsewhere dismisses it.
        WindowEvent::Focused(false) => {
            let _ = window.hide();
            forget_target(window);
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
    forget_target(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::frontmost::fakes::FakeFrontmost;

    #[test]
    fn known_frontmost_app_is_stored() {
        let target = Mutex::new(None);
        remember(&target, &FakeFrontmost(Some("TextEdit".into())));
        let saved = lock(&target).clone().unwrap();
        assert_eq!(saved.name.as_deref(), Some("TextEdit"));
        assert_eq!(saved.pid, 42);
    }

    #[test]
    fn magpie_or_unknown_frontmost_keeps_the_saved_target() {
        // `FakeFrontmost(None)` stands for "Magpie itself is frontmost" and
        // "unknown": the adapter returns `None` for both.
        let target = Mutex::new(None);
        remember(&target, &FakeFrontmost(Some("TextEdit".into())));
        remember(&target, &FakeFrontmost(None));
        assert_eq!(
            lock(&target).as_ref().and_then(|t| t.name.as_deref()),
            Some("TextEdit")
        );

        let empty = Mutex::new(None);
        remember(&empty, &FakeFrontmost(None));
        assert!(lock(&empty).is_none());
    }
}
