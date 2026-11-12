use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::error::{AppError, AppResult};
use crate::window;

/// Cmd+Shift+V on macOS, Ctrl+Shift+V on Windows and Linux.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+V";

pub fn parse(shortcut: &str) -> AppResult<Shortcut> {
    shortcut
        .parse::<Shortcut>()
        .map_err(|e| AppError::Shortcut {
            shortcut: shortcut.to_string(),
            reason: e.to_string(),
        })
}

/// Replace whatever shortcut is registered with `shortcut`, which toggles the
/// history window.
pub fn register<R: Runtime>(app: &AppHandle<R>, shortcut: &str) -> AppResult<()> {
    let parsed = parse(shortcut)?;
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| AppError::Shortcut {
        shortcut: shortcut.to_string(),
        reason: e.to_string(),
    })?;
    gs.on_shortcut(parsed, |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            window::toggle(app);
        }
    })
    .map_err(|e| AppError::Shortcut {
        shortcut: shortcut.to_string(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_parses() {
        assert!(parse(DEFAULT_SHORTCUT).is_ok());
    }

    #[test]
    fn garbage_shortcut_is_a_readable_error() {
        let err = parse("Shift+Banana").unwrap_err().to_string();
        assert!(err.contains("Shift+Banana"), "{err}");
    }
}
