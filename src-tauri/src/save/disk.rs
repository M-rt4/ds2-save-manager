//! Finding saves on disk, and writing them back safely: never while the game runs, never
//! over a file that changed since it was read, always after a timestamped backup.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use super::crypto::md5;
use super::edition::Edition;
use super::error::{Result, SaveError};
use super::header::STEAM_ID_HIGH;

pub const BACKUP_DIR: &str = "DS2 Save Manager Backups";

/// `%APPDATA%\DarkSoulsII`, where both releases keep one folder per Steam account.
pub fn saves_root() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("DarkSoulsII"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundSave {
    pub path: String,
    pub file_name: String,
    pub folder: String,
    /// SteamID64 as decimal text, when the folder is named after an account.
    pub account: Option<String>,
    pub edition: Option<Edition>,
    pub size: u64,
    pub modified: u64,
}

/// The account a save folder is named after (16 hex digits of the SteamID64).
pub fn account_of_folder(folder: &str) -> Option<u64> {
    if folder.len() != 16 {
        return None;
    }
    u64::from_str_radix(folder, 16).ok().filter(|id| id >> 32 == STEAM_ID_HIGH)
}

pub fn found_save(path: &Path) -> Option<FoundSave> {
    let meta = fs::metadata(path).ok()?;
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let folder = path.parent().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    Some(FoundSave {
        path: path.to_string_lossy().into_owned(),
        account: account_of_folder(&folder).map(|id| id.to_string()),
        edition: Edition::from_file_name(&file_name),
        folder,
        file_name,
        size: meta.len(),
        modified: modified_ms(&meta),
    })
}

/// Every `.sl2` in the account folders under `%APPDATA%\DarkSoulsII`, newest first.
pub fn discover() -> Vec<FoundSave> {
    let Some(root) = saves_root() else { return Vec::new() };
    let mut found = Vec::new();
    for dir in fs::read_dir(&root).into_iter().flatten().flatten() {
        if !dir.path().is_dir() {
            continue;
        }
        for file in fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let p = file.path();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sl2")) {
                found.extend(found_save(&p));
            }
        }
    }
    found.sort_by(|a, b| b.modified.cmp(&a.modified));
    found
}

fn modified_ms(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64)
}

/// A fingerprint of a file's bytes, to notice when the game wrote it after we read it.
pub fn fingerprint(bytes: &[u8]) -> [u8; 16] {
    md5(bytes)
}

/// Copies `path` into the backup folder next to it and returns the copy's path.
pub fn backup(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let dir = path.parent().map(|p| p.join(BACKUP_DIR)).ok_or_else(|| SaveError::invalid("Bad path."))?;
    fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut target = dir.join(format!("{name}.{stamp}.bak"));
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("{name}.{stamp}-{n}.bak"));
        n += 1;
    }
    fs::copy(path, &target)?;
    Ok(Some(target))
}

/// Writes through a temporary file in the same folder, then swaps it in.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("sl2.tmp");
    fs::write(&tmp, bytes)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Whether a Dark Souls II process (either release, both ship `DarkSoulsII.exe`) runs.
pub fn game_running() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq DarkSoulsII.exe", "/FO", "CSV", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains("darksoulsii.exe"))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}
