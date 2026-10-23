//! Read-only folder listings for the in-app file browser, which stands in for the system's
//! open/save windows while the app is driven by a gamepad (they can't be used with one, and
//! on the Steam Deck's Game Mode they leave the app). Nothing here writes.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ds2save::save::{disk, SaveError};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    path: String,
    /// "saves", "home", "drive" or "media"; the UI names them.
    kind: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    name: String,
    path: String,
    dir: bool,
    size: u64,
    modified: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    path: String,
    parent: Option<String>,
    entries: Vec<Entry>,
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Where browsing can start: the save folders, the home folder, then the drives (Windows) or
/// the file system root and removable media such as the Steam Deck's microSD card (Linux).
#[tauri::command]
pub async fn browse_places() -> Vec<Place> {
    let mut places: Vec<Place> = disk::saves_roots(None).iter().filter(|p| p.is_dir()).map(|p| Place { path: text(p), kind: "saves" }).collect();
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if let Some(home) = home.filter(|h| h.is_dir()) {
        places.push(Place { path: text(&home), kind: "home" });
    }
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let drive = PathBuf::from(format!("{}:\\", letter as char));
        if drive.is_dir() {
            places.push(Place { path: text(&drive), kind: "drive" });
        }
    }
    #[cfg(not(windows))]
    {
        places.push(Place { path: "/".into(), kind: "drive" });
        places.push(Place { path: "/home".into(), kind: "drive" });
        // /run/media/<card> on the Deck, /run/media/<user>/<label> on most desktops.
        for dir in fs::read_dir("/run/media").into_iter().flatten().flatten().map(|d| d.path()).filter(|p| p.is_dir()) {
            let inner: Vec<PathBuf> = fs::read_dir(&dir).into_iter().flatten().flatten().map(|d| d.path()).filter(|p| p.is_dir()).collect();
            if inner.is_empty() || dir.join("steamapps").is_dir() {
                places.push(Place { path: text(&dir), kind: "media" });
            } else {
                places.extend(inner.iter().map(|p| Place { path: text(p), kind: "media" }));
            }
        }
    }
    places
}

/// A folder's entries, folders first, each group by name.
#[tauri::command]
pub async fn list_dir(path: String) -> Result<Listing, SaveError> {
    let dir = PathBuf::from(&path);
    let mut entries: Vec<Entry> = fs::read_dir(&dir)?
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                path: text(&e.path()),
                dir: meta.is_dir(),
                size: if meta.is_dir() { 0 } else { meta.len() },
                modified: meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64),
            })
        })
        .collect();
    entries.sort_by_key(|e| (!e.dir, e.name.to_lowercase()));
    Ok(Listing { path: text(&dir), parent: dir.parent().map(text), entries })
}
