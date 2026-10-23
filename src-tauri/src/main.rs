#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod commands_about;
mod commands_backups;
mod commands_files;
mod commands_games;
mod conversion;
mod commands_raw;
mod gamepad;

use tauri::{WebviewUrl, WebviewWindowBuilder};

fn main() {
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
            commands_raw::raw_read,
            commands_raw::raw_write,
            commands_raw::raw_find,
            gamepad::gamepads,
            commands_files::browse_places,
            commands_files::list_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DS2 Save Manager");
}
