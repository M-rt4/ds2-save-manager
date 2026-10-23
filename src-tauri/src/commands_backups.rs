//! The commands behind the Backups tab: list, take, restore and delete the backups kept
//! next to a save.

use std::path::PathBuf;

use ds2save::save::backups::{self, Backup, Reason, Restored};
use ds2save::save::{disk, SaveError};
use tauri::State;

use crate::commands::AppState;

type CmdResult<T> = Result<T, SaveError>;

#[tauri::command]
pub async fn list_backups(save_path: String) -> Vec<Backup> {
    backups::list(&PathBuf::from(save_path))
}

/// Backs up the file on disk as it is now; unsaved edits in the app are not in it.
#[tauri::command]
pub async fn create_backup(save_path: String) -> CmdResult<Backup> {
    backups::create(&PathBuf::from(save_path), Reason::Manual)
}

/// Restores a backup over `target_path`. Refused while the game runs. A save open in the
/// app is read again afterwards, so its unsaved edits are dropped.
#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, backup_path: String, target_path: String) -> CmdResult<Restored> {
    if disk::game_running() {
        return Err(SaveError::new("gameRunning", "Dark Souls II is running. Quit to the desktop first."));
    }
    let target = PathBuf::from(target_path);
    let restored = backups::restore(&PathBuf::from(backup_path), &target)?;
    state.reload_if_open(&target)?;
    Ok(restored)
}

#[tauri::command]
pub async fn delete_backup(path: String) -> CmdResult<()> {
    backups::delete(&PathBuf::from(path))
}
