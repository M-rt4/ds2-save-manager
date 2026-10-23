#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod commands_about;
mod commands_backups;
mod commands_files;
mod commands_games;
mod conversion;
mod commands_raw;
mod commands_document;
mod commands_inventory;
mod gamepad;

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// WebKitGTK's DMA-BUF renderer is behind most blank Tauri windows on Linux. The AppImage's GTK
/// hook pins `GDK_BACKEND=x11`, where that renderer brings nothing, and the Steam Deck's Game
/// Mode draws through gamescope; in both it is turned off, unless the user set the variable
/// (`WEBKIT_DISABLE_DMABUF_RENDERER=0` keeps it on). This runs before any thread or webview exists.
#[cfg(target_os = "linux")]
fn linux_webkit_defaults() {
    let appimage = std::env::var_os("APPIMAGE").is_some();
    let game_mode = std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some() || std::env::var_os("SteamDeck").is_some_and(|v| v == "1");
    if (appimage || game_mode) && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    linux_webkit_defaults();
    let mut context = tauri::generate_context!();
    // Development builds keep their own settings (`settings.json`) and WebView storage (language,
    // preferences), so a dev copy never reads or changes the released app's: both folders are
    // named after the identifier. `DS2SM_CONFIG_DIR` and `WEBVIEW2_USER_DATA_FOLDER` still win.
    if cfg!(debug_assertions) {
        let identifier = format!("{}.dev", context.config().identifier);
        context.config_mut().identifier = identifier;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState::default())
        .manage(gamepad::Pads::default())
        .setup(|app| {
            let title = match commands_about::dev_label() {
                Some(label) => format!("DS2 Save Manager (dev: {label})"),
                None => "DS2 Save Manager".into(),
            };
            let mut window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title(title)
                .inner_size(1280.0, 800.0)
                .min_inner_size(1040.0, 660.0)
                .decorations(false)
                .background_color(tauri::window::Color(7, 7, 10, 255))
                .center();
            // Development copies stay on top of everything, without taking focus when they
            // open, so the user can watch changes land while working in other windows.
            if cfg!(debug_assertions) {
                window = window.always_on_top(true).focused(false);
            }
            window.build()?;
            gamepad::start(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_saves,
            commands::saves_root,
            commands::game_running,
            commands::open_save,
            commands::get_save,
            commands::close_save,
            commands::reload_save,
            commands::get_character,
            commands::edit_character,
            commands::item_catalog,
            commands::write_save,
            commands::set_account,
            conversion::plan_conversion,
            conversion::run_conversion,
            conversion::default_target,
            commands_backups::list_backups,
            commands_backups::create_backup,
            commands_backups::restore_backup,
            commands_backups::delete_backup,
            commands_games::list_games,
            commands_games::add_game_folder,
            commands_games::remove_game_folder,
            commands_about::system_info,
            commands_about::dev_label,
            commands_about::test_saves_root,
            commands_raw::raw_read,
            commands_raw::raw_write,
            commands_raw::raw_find,
            commands_document::document_export,
            commands_document::document_preview,
            commands_document::document_apply,
            commands_document::document_write_file,
            commands_document::document_read_file,
            commands_inventory::inventory_changes,
            commands_inventory::inventory_revert,
            gamepad::gamepads,
            commands_files::browse_places,
            commands_files::list_dir,
        ])
        .run(context)
        .expect("error while running DS2 Save Manager");
}
