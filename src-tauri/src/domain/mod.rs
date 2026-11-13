//! Pure domain types and rules. No Tauri, OS or SQLite types (AD-1).

pub mod error;
pub mod item;
pub mod target;

pub use error::CoreError;
pub use item::{ItemKind, ItemSummary, NewCapture, Recorded};
pub use target::TargetApp;
