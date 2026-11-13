use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::adapters::sqlite::SqliteStore;
use crate::domain::TargetApp;
use crate::services::paste::OwnWrites;

/// The store shared by the capture watcher and the commands (AD-11).
pub type SharedStore = Arc<Mutex<SqliteStore>>;

/// Tauri managed state. If the database could not be opened the app still
/// runs, and the UI shows `Err`'s message.
pub struct AppState {
    pub store: Result<SharedStore, String>,
    /// Magpie's latest own clipboard write, shared with the watcher (AD-7).
    pub own_writes: Arc<OwnWrites>,
    /// The app that was frontmost when the window was last shown (AD-12).
    pub target: Mutex<Option<TargetApp>>,
    /// A file whose existence means the user has answered the Accessibility
    /// explainer once. `None` if there is no data directory.
    pub permission_asked_marker: Option<PathBuf>,
}

impl AppState {
    pub fn new(
        store: Result<SharedStore, String>,
        own_writes: Arc<OwnWrites>,
        permission_asked_marker: Option<PathBuf>,
    ) -> Self {
        Self {
            store,
            own_writes,
            target: Mutex::new(None),
            permission_asked_marker,
        }
    }
}
