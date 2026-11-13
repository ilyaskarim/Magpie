//! The Tauri shell: wiring, commands, tray, window and hotkey. Nothing below
//! depends on this module.

pub mod commands;
pub mod error;
pub mod hotkey;
pub mod state;
pub mod tray;
pub mod window;

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::adapters::SystemFrontmost;
use crate::adapters::clipboard::{SystemClipboard, SystemWatcher};
use crate::adapters::sqlite::SqliteStore;
use crate::services;
use crate::services::paste::OwnWrites;
use error::AppError;
use state::{AppState, SharedStore};

pub const EVENT_HISTORY_CHANGED: &str = "history-changed";
pub const EVENT_WINDOW_SHOWN: &str = "window-shown";

/// Marker file in the data root: the Accessibility explainer was answered.
const PERMISSION_ASKED_MARKER: &str = "accessibility-asked";

/// The user-facing product name, read from `tauri.conf.json` (`productName`).
pub fn app_name<R: Runtime>(app: &AppHandle<R>) -> String {
    app.config()
        .product_name
        .clone()
        .unwrap_or_else(|| app.package_info().name.clone())
}

/// Opens and migrates `<app_local_data_dir>/magpie.db` (AD-16).
pub fn open_store<R: Runtime>(app: &AppHandle<R>) -> Result<SharedStore, AppError> {
    let root = data_root(app)?;
    std::fs::create_dir_all(&root)
        .map_err(|e| AppError::Startup(format!("could not create {}: {e}", root.display())))?;
    // `CoreError` already reads "History database error: …"; pass it through
    // rather than adding a second prefix.
    let store = SqliteStore::open(&root.join("magpie.db")).map_err(AppError::Core)?;
    Ok(Arc::new(Mutex::new(store)))
}

/// True for a debug build started with `MAGPIE_DATA_DIR`: a test run with its
/// own history. Always false in release builds.
pub fn is_isolated_dev_run() -> bool {
    cfg!(debug_assertions) && std::env::var_os("MAGPIE_DATA_DIR").is_some()
}

/// The storage root (AD-16): `app_local_data_dir()`. Debug builds honour
/// `MAGPIE_DATA_DIR` so development and test runs never touch real history;
/// release builds ignore it.
fn data_root<R: Runtime>(app: &AppHandle<R>) -> Result<std::path::PathBuf, AppError> {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("MAGPIE_DATA_DIR") {
        return Ok(std::path::PathBuf::from(dir));
    }
    app.path()
        .app_local_data_dir()
        .map_err(|e| AppError::Startup(format!("no local data directory: {e}")))
}

/// Starts the supervised capture watcher on its own thread (AD-11, AD-18).
pub fn start_watcher<R: Runtime>(app: &AppHandle<R>, store: SharedStore, own: Arc<OwnWrites>) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("capture-watcher".into())
        .spawn(move || {
            let notify = || {
                if let Err(e) = app.emit(EVENT_HISTORY_CHANGED, ()) {
                    eprintln!("could not emit {EVENT_HISTORY_CHANGED}: {e}");
                }
            };
            services::capture::supervise(|| {
                let reader = SystemClipboard::new()?;
                services::capture::watch(
                    &mut SystemWatcher,
                    &reader,
                    &SystemFrontmost,
                    &*store,
                    &own,
                    &services::now_ms,
                    &notify,
                )
            })
        });
    if let Err(e) = spawned {
        eprintln!("could not start the capture watcher: {e}");
    }
}

/// Registers the managed state, given the outcome of opening the store.
pub fn manage_state<R: Runtime>(
    app: &AppHandle<R>,
    store: Result<SharedStore, AppError>,
    own: Arc<OwnWrites>,
) {
    let store = store.map_err(|e| {
        eprintln!("{e}");
        e.to_string()
    });
    let marker = data_root(app)
        .ok()
        .map(|root| root.join(PERMISSION_ASKED_MARKER));
    app.manage(AppState::new(store, own, marker));
}
