mod browsers;
mod commands;
mod db;
mod error;
mod metadata;
mod shortcut;
mod transfer;
mod tray;

use std::sync::Mutex;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;
use tauri_plugin_window_state::StateFlags;

/// Command-line flags. `--hidden`: start in the tray (used by launch at login).
/// `--search`: show the window with search open; a second launch forwards it to the
/// running app, so it can be bound to a desktop shortcut where global shortcuts don't
/// work (Wayland).
const HIDDEN_FLAG: &str = "--hidden";
const SEARCH_FLAG: &str = "--search";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // single-instance must be registered first.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|a| a == SEARCH_FLAG) {
                tray::open_view(app, "search");
            } else {
                tray::show_main(app);
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        tray::open_view(app, "search");
                    }
                })
                .build(),
        )
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args([HIDDEN_FLAG])
                .build(),
        )
        // Visibility is ours to decide (start hidden in the tray), not the saved state's.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("links.db"))?;
            let fetcher = metadata::Fetcher::new(dir.join("favicons"))?;
            // Housekeeping only; a failure here shouldn't stop the app.
            if let Ok(used) = db::bookmarks::favicons_in_use(&conn) {
                let _ = metadata::remove_unused(&fetcher.favicon_dir, &used);
            }
            let configured_shortcut = db::settings::get(&conn, db::settings::GLOBAL_SHORTCUT)?
                .unwrap_or_else(|| shortcut::DEFAULT.to_string());
            app.manage(fetcher);
            app.manage(db::Db(Mutex::new(conn)));
            app.manage(browsers::BrowserCache::default());
            app.manage(shortcut::ActiveShortcut::default());

            // A shortcut that can't be registered isn't fatal; Settings shows the error.
            let _ = shortcut::apply(app.handle(), Some(&configured_shortcut));
            tray::create(app.handle())?;

            let args: Vec<String> = std::env::args().collect();
            if args.iter().any(|a| a == SEARCH_FLAG) {
                tray::open_view(app.handle(), "search");
            } else if !args.iter().any(|a| a == HIDDEN_FLAG) {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if commands::close_to_tray(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_folders,
            commands::create_folder,
            commands::rename_folder,
            commands::delete_folder,
            commands::list_bookmarks,
            commands::create_bookmark,
            commands::update_bookmark,
            commands::delete_bookmark,
            commands::list_browsers,
            commands::get_browser_prefs,
            commands::set_browser_prefs,
            commands::open_url,
            commands::get_setting,
            commands::set_setting,
            commands::search,
            commands::move_bookmark,
            commands::move_folder,
            commands::set_folder_target,
            commands::resolve_target,
            commands::list_tags,
            commands::fetch_metadata,
            commands::refresh_metadata,
            commands::fetch_missing_favicons,
            commands::favicon_dir,
            commands::get_desktop_settings,
            commands::set_global_shortcut,
            commands::set_close_to_tray,
            commands::set_autostart,
            commands::export_backup,
            commands::export_html,
            commands::inspect_import,
            commands::import_file,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, _event| {
        // macOS: clicking the Dock icon while the window is hidden brings it back.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = _event {
            tray::show_main(_app);
        }
    });
}
