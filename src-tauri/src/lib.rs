//! Magpie: a local-first clipboard manager.
//!
//! The product name lives in exactly one place: `productName` in
//! `tauri.conf.json`. Rust reads it through [`app_name`] and the frontend
//! through `getName()` from `@tauri-apps/api/app`.

mod error;
mod hotkey;
mod tray;
mod window;

use tauri::{AppHandle, Manager, Runtime};

/// The user-facing product name, read from `tauri.conf.json` (`productName`).
pub fn app_name<R: Runtime>(app: &AppHandle<R>) -> String {
    app.config()
        .product_name
        .clone()
        .unwrap_or_else(|| app.package_info().name.clone())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // A second launch (e.g. from Finder or the Start menu) just opens the
        // existing history window instead of starting a second listener.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![window::hide_window])
        .on_window_event(window::on_window_event)
        .setup(|app| {
            // Menu-bar-only app on macOS: no Dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::create(app.handle())?;

            if let Err(e) = hotkey::register(app.handle(), hotkey::DEFAULT_SHORTCUT) {
                // Not fatal: the tray menu still opens the window. Most likely
                // another app already owns the shortcut.
                eprintln!("{e}");
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
