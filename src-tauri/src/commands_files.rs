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
    // The drives Windows lists, without probing every letter: a network drive that has gone
    // away can take many seconds to answer, and the browser waits for this list. Network drives
    // are listed untouched (opening a dead one fails on its own); a local drive is listed when
    // it answers, which an empty card reader or disc drive does at once, with nothing inside.
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
        use windows_sys::Win32::System::WindowsProgramming::{DRIVE_NO_ROOT_DIR, DRIVE_REMOTE, DRIVE_UNKNOWN};
        // SAFETY: only reads the bitmask of drive letters in use.
        let letters = unsafe { GetLogicalDrives() };
        for i in (0..26u8).filter(|i| letters & (1 << i) != 0) {
            let root = format!("{}:\\", (b'A' + i) as char);
            let wide: Vec<u16> = root.encode_utf16().chain([0]).collect();
            // SAFETY: a NUL-terminated path that outlives the call.
            let listed = match unsafe { GetDriveTypeW(wide.as_ptr()) } {
                DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR => false,
                DRIVE_REMOTE => true,
                _ => Path::new(&root).is_dir(),
            };
            if listed {
                places.push(Place { path: root, kind: "drive" });
            }
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
    Ok(listing(Path::new(&path))?)
}

fn listing(dir: &Path) -> std::io::Result<Listing> {
    let mut entries: Vec<Entry> = fs::read_dir(dir)?
        .flatten()
        .filter_map(|e| {
            // Through links: a linked folder (a junction on Windows, ~/.steam/steam on the Deck)
            // is a folder to open, not a file. A dangling link keeps the link's own metadata.
            let meta = fs::metadata(e.path()).or_else(|_| e.metadata()).ok()?;
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
    Ok(Listing { path: text(dir), parent: dir.parent().map(text), entries })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::listing;

    /// A folder link as the platform makes one without special rights: a junction on Windows,
    /// a symbolic link elsewhere (~/.steam/steam on the Deck is one).
    fn link_dir(target: &Path, link: &Path) {
        #[cfg(windows)]
        let ok = std::process::Command::new("cmd").arg("/c").arg("mklink").arg("/J").arg(link).arg(target).output().unwrap().status.success();
        #[cfg(not(windows))]
        let ok = std::os::unix::fs::symlink(target, link).is_ok();
        assert!(ok, "could not link {} to {}", link.display(), target.display());
    }

    #[test]
    fn linked_folders_list_as_folders() {
        let base = std::env::temp_dir().join(format!("ds2sm-browse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("real")).unwrap();
        std::fs::write(base.join("save.sl2"), b"x").unwrap();
        link_dir(&base.join("real"), &base.join("linked"));
        let list = listing(&base).unwrap();
        let dir = |name: &str| list.entries.iter().find(|e| e.name == name).map(|e| e.dir);
        let names: Vec<&str> = list.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!((dir("real"), dir("linked"), dir("save.sl2")), (Some(true), Some(true), Some(false)), "{names:?}");
        // Folders first, linked ones among them.
        assert_eq!(names, ["linked", "real", "save.sl2"]);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
