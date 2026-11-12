use serde::{Serialize, Serializer};

/// Errors that cross the Tauri command boundary. They serialise to a plain,
/// human-readable string so the UI can show them as-is.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Could not register the shortcut \"{shortcut}\": {reason}")]
    Shortcut { shortcut: String, reason: String },
    #[error("Window error: {0}")]
    Window(String),
    #[error("{0}")]
    Tauri(#[from] tauri::Error),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
