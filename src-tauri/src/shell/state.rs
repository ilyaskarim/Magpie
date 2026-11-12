use std::sync::{Arc, Mutex};

use crate::adapters::sqlite::SqliteStore;

/// The store shared by the capture watcher and the commands (AD-11).
pub type SharedStore = Arc<Mutex<SqliteStore>>;

/// Tauri managed state. If the database could not be opened the app still
/// runs, and the UI shows `Err`'s message.
pub struct AppState {
    pub store: Result<SharedStore, String>,
}
