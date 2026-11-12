use serde::{Serialize, Serializer};

use crate::domain::CoreError;

/// Errors that cross the Tauri command boundary (AD-14). They serialise to a
/// plain, human-readable string so the UI can show them as-is.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Could not register the shortcut \"{shortcut}\": {reason}")]
    Shortcut { shortcut: String, reason: String },
    #[error("Window error: {0}")]
    Window(String),
    #[error("History is unavailable: {0}")]
    StoreUnavailable(String),
    #[error("Could not open the history database: {0}")]
    Startup(String),
    #[error("Could not save your Accessibility choice: {0}")]
    PermissionChoice(String),
    #[error("{0}")]
    Core(#[from] CoreError),
    #[error("{0}")]
    Tauri(#[from] tauri::Error),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
