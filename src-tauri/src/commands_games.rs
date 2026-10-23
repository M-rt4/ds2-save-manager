//! The commands behind the Games rail: which releases are installed, where each keeps its
//! saves, and the game folders the user adds by hand.

use std::path::{Path, PathBuf};

use ds2save::save::games::{self, GameInfo, ManualFolder};
use ds2save::save::SaveError;
use tauri::{AppHandle, Manager};

type CmdResult<T> = Result<T, SaveError>;

/// `settings.json` in the app's config folder. `DS2SM_CONFIG_DIR` points test copies at a
/// scratch folder instead.
fn settings_file(app: &AppHandle) -> CmdResult<PathBuf> {
    let dir = match std::env::var_os("DS2SM_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => app.path().app_config_dir().map_err(|e| SaveError::new("io", e.to_string()))?,
    };
    Ok(dir.join("settings.json"))
}

fn manual(app: &AppHandle) -> Vec<ManualFolder> {
    settings_file(app).map(|f| games::manual_folders(&f)).unwrap_or_default()
}

/// Lists off the async runtime: the first look at an install reads its executable and
/// regulation file.
async fn list(app: &AppHandle) -> Vec<GameInfo> {
    let manual = manual(app);
    tauri::async_runtime::spawn_blocking(move || games::list(&manual)).await.unwrap_or_default()
}

#[tauri::command]
pub async fn list_games(app: AppHandle) -> Vec<GameInfo> {
    list(&app).await
}

/// Remembers a game folder (after checking it holds `DarkSoulsII.exe`) and lists again.
#[tauri::command]
pub async fn add_game_folder(app: AppHandle, path: String) -> CmdResult<Vec<GameInfo>> {
    games::add_folder(&settings_file(&app)?, Path::new(&path))?;
    Ok(list(&app).await)
}

#[tauri::command]
pub async fn remove_game_folder(app: AppHandle, path: String) -> CmdResult<Vec<GameInfo>> {
    games::remove_folder(&settings_file(&app)?, &path)?;
    Ok(list(&app).await)
}
