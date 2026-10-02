//! Tray icon and the helpers that bring the window back.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// Frontend event asking it to open a view: "search" or "settings".
pub const OPEN_VIEW_EVENT: &str = "open-view";

pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Shows the window and opens `view` in it.
pub fn open_view<R: Runtime>(app: &AppHandle<R>, view: &str) {
    show_main(app);
    let _ = app.emit(OPEN_VIEW_EVENT, view);
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Link Opener", true, None::<&str>)?;
    let search = MenuItem::with_id(app, "search", "Quick search", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Link Opener", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &search, &settings, &separator, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Link Opener")
        .menu(&menu)
        // Left click shows the window; the menu is on right click.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "search" => open_view(app, "search"),
            "settings" => open_view(app, "settings"),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
