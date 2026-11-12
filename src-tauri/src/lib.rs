//! Magpie: a local-first clipboard manager.
//!
//! Hexagonal layout (see the architecture spine): `domain` (pure), `ports`
//! (traits), `adapters` (OS, clipboard, SQLite), `services` (use cases) and
//! `shell` (Tauri). This file only wires them together.
//!
//! The product name lives in exactly one place: `productName` in
//! `tauri.conf.json`. Rust reads it through [`shell::app_name`] and the
//! frontend through `getName()` from `@tauri-apps/api/app`.

mod adapters;
mod domain;
mod ports;
mod services;
mod shell;

use tauri::Manager;

use shell::{commands, hotkey, tray, window};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    // A second launch (e.g. from Finder or the Start menu) just opens the
    // existing history window instead of starting a second listener. Debug
    // runs with an isolated MAGPIE_DATA_DIR skip this so they can run next to
    // an installed copy without handing off to it.
    if !shell::is_isolated_dev_run() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show(app);
        }));
    }
    let builder = builder
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            window::hide_window,
            commands::list_items,
            commands::startup_error,
        ])
        .on_window_event(window::on_window_event)
        .setup(|app| {
            // Menu-bar-only app on macOS: no Dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();

            tray::create(handle)?;

            // AD-18 order: open DB → migrate (inside open) → register hotkey
            // → start watcher. A database failure is not fatal: the window
            // still opens and shows the error.
            let store = shell::open_store(handle);
            let watcher_store = store.as_ref().ok().cloned();
            shell::manage_state(handle, store);

            if let Err(e) = hotkey::register(handle, hotkey::DEFAULT_SHORTCUT) {
                // Not fatal: the tray menu still opens the window. Most likely
                // another app already owns the shortcut.
                eprintln!("{e}");
            }

            if let Some(store) = watcher_store {
                shell::start_watcher(handle, store);
            }

            // Keep the window alive and hidden; it is shown on demand.
            if let Some(w) = app.get_webview_window(window::MAIN) {
                let _ = w.hide();
            }
            Ok(())
        });

    if let Err(e) = builder.run(tauri::generate_context!()) {
        eprintln!("fatal: could not start the application: {e}");
        std::process::exit(1);
    }
}
