use tauri::{
    AppHandle, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder, TrayIconEvent},
};

use crate::{app_name, window};

pub const TRAY_ID: &str = "main-tray";

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let name = app_name(app);
    let open = MenuItem::with_id(app, "open", format!("Open {name}"), true, None::<&str>)?;
    // Wired up to real behaviour in later stages (capture pause, settings).
    let pause = MenuItem::with_id(app, "pause", "Pause capture", false, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", format!("Quit {name}"), true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &pause, &settings, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(&name)
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => window::show(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(on_tray_click);

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Windows convention: left click opens the window, right click shows the menu.
#[cfg(not(target_os = "macos"))]
fn on_tray_click<R: Runtime>(tray: &TrayIcon<R>, event: TrayIconEvent) {
    use tauri::tray::{MouseButton, MouseButtonState};
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        window::show(tray.app_handle());
    }
}

/// macOS convention: any click on a menu bar item opens its menu.
#[cfg(target_os = "macos")]
fn on_tray_click<R: Runtime>(_tray: &TrayIcon<R>, _event: TrayIconEvent) {}
