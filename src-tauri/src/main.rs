#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod commands_backups;
mod conversion;
mod commands_games;

use tauri::{WebviewUrl, WebviewWindowBuilder};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState::default())
        .setup(|app| {
            let mut window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("DS2 Save Manager")
                .inner_size(1280.0, 800.0)
                .min_inner_size(1040.0, 660.0)
                .decorations(false)
                .background_color(tauri::window::Color(7, 7, 10, 255))
                .center();
            // Debug builds started with DS2SM_TEST_WINDOW open off screen, unfocused and
            // out of the taskbar, so a test copy never steps in front of the game or the
            // user's own windows. It is driven over WebView2's CDP port instead.
            if cfg!(debug_assertions) && std::env::var_os("DS2SM_TEST_WINDOW").is_some() {
                window = window.position(-4000.0, -4000.0).focused(false).skip_taskbar(true);
            }
            window.build()?;
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running DS2 Save Manager");
}
