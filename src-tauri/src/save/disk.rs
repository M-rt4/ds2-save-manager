//! Finding saves on disk, and writing them back safely: never while the game runs, never
//! over a file that changed since it was read, always after a timestamped backup.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use super::backups::Reason;
use super::crypto::md5;
use super::edition::Edition;
use super::error::{Result, SaveError};
use super::header::STEAM_ID_HIGH;

pub const BACKUP_DIR: &str = "DS2 Save Manager Backups";

/// The `DarkSoulsII` folders holding one folder per Steam account, for one release or (with
/// `None`) both. Windows has a single one, `%APPDATA%\DarkSoulsII`, shared by both releases;
/// Linux has one per Wine/Proton prefix that ran the game (see `platform`).
/// `DS2SM_SAVES_ROOT` points test copies at a scratch tree instead of the real saves.
pub fn saves_roots(edition: Option<Edition>) -> Vec<PathBuf> {
    if let Some(root) = std::env::var_os("DS2SM_SAVES_ROOT") {
        return vec![PathBuf::from(root)];
    }
    game_saves_roots(edition)
}

/// The game's own save folders, whatever `DS2SM_SAVES_ROOT` says.
fn game_saves_roots(edition: Option<Edition>) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let _ = edition;
        std::env::var_os("APPDATA").map(|a| vec![PathBuf::from(a).join("DarkSoulsII")]).unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
        let steam = super::platform::linux_steam_dirs(&home);
        let editions = edition.map_or(Edition::ALL.to_vec(), |e| vec![e]);
        let mut roots: Vec<PathBuf> = Vec::new();
        for e in editions {
            for r in super::platform::linux_save_roots(&home, &steam, e) {
                if !roots.contains(&r) {
                    roots.push(r);
                }
            }
        }
        roots
    }
}

/// The first saves folder, for places that show a single one.
pub fn saves_root() -> Option<PathBuf> {
    saves_roots(None).into_iter().next()
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

/// Every `.sl2` in the account folders of every saves folder, newest first.
pub fn discover() -> Vec<FoundSave> {
    let mut found = Vec::new();
    for dir in saves_roots(None).iter().flat_map(|root| fs::read_dir(root).into_iter().flatten().flatten()) {
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
    found.sort_by_key(|f| std::cmp::Reverse(f.modified));
    found
}

fn modified_ms(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64)
}

/// A fingerprint of a file's bytes, to notice when the game wrote it after we read it.
pub fn fingerprint(bytes: &[u8]) -> [u8; 16] {
    md5(bytes)
}

/// Copies `path` into the backup folder next to it as `<file>.<stamp>.<reason>.bak` and
/// returns the copy's path.
pub fn backup(path: &Path, reason: Reason) -> Result<Option<PathBuf>> {
    ensure_writable(path)?;
    if !path.exists() {
        return Ok(None);
    }
    let dir = path.parent().map(|p| p.join(BACKUP_DIR)).ok_or_else(|| SaveError::invalid("Bad path."))?;
    fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut target = dir.join(format!("{name}.{stamp}.{reason}.bak"));
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("{name}.{stamp}-{n}.{reason}.bak"));
        n += 1;
    }
    fs::copy(path, &target)?;
    Ok(Some(target))
}

/// Development builds change simulated data only: anything inside the game's real save folders
/// is refused, wherever the path came from (the list, a file picker, a typed path), even when
/// `DS2SM_SAVES_ROOT` itself points there. Only the released app writes real saves.
pub fn ensure_writable(path: &Path) -> Result<()> {
    if cfg!(debug_assertions) && inside_any(path, &guard_roots()) {
        return Err(SaveError::new(
            "devRealSave",
            "This is a development build. It never changes the game's real save folders; only the released app does.",
        ));
    }
    Ok(())
}

/// What development builds never write into. On Windows that is `DarkSoulsII` in the roaming
/// folder, made or not; on Linux the game's save folders that exist under the home folder plus
/// every place it would make one.
fn guard_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        guard_bases(std::env::var_os("APPDATA")).into_iter().map(|roaming| roaming.join("DarkSoulsII")).collect()
    }
    #[cfg(not(windows))]
    {
        let mut roots = Vec::new();
        for home in guard_bases(std::env::var_os("HOME")) {
            let steam = super::platform::linux_steam_dirs(&home);
            for e in Edition::ALL {
                roots.extend(super::platform::linux_save_roots(&home, &steam, e));
            }
            roots.extend(super::platform::linux_guard_roots(&home, &steam));
        }
        roots
    }
}

/// The folders the guard looks in: the one the environment names (`APPDATA` on Windows, `HOME`
/// elsewhere) and the one the system itself reports. A copy started with the variable pointed
/// at a scratch folder, or without it, still keeps off the real saves, and the scratch folder
/// stands in for the real one in tests.
fn guard_bases(from_env: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = from_env.filter(|v| !v.is_empty()).map(PathBuf::from).into_iter().collect();
    // The shell's known folder, which never reads APPDATA.
    #[cfg(windows)]
    let system = dirs::data_dir();
    // The user database's home, which never reads HOME.
    #[cfg(unix)]
    let system = super::platform::passwd_home();
    if let Some(system) = system.filter(|s| !bases.contains(s)) {
        bases.push(system);
    }
    bases
}

/// Whether `path` lies inside one of `roots`, with links and `..` resolved on both sides.
fn inside_any(path: &Path, roots: &[PathBuf]) -> bool {
    let path = comparable(&resolved(path));
    roots.iter().any(|root| path.starts_with(comparable(&resolved(root))))
}

/// `path` made absolute, with its longest existing ancestor canonicalized and the rest (files
/// and folders not written yet) appended as given.
fn resolved(path: &Path) -> PathBuf {
    let full = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut head = full.as_path();
    let mut tail = Vec::new();
    loop {
        if let Ok(real) = fs::canonicalize(head) {
            return tail.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (head.parent(), head.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                head = parent;
            }
            _ => return full.clone(),
        }
    }
}

/// Windows paths are compared without regard to case; Linux paths as they are.
fn comparable(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.to_path_buf()
    }
}

/// Writes through a temporary file in the same folder, then swaps it in.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure_writable(path)?;
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
    // Debug test copies whose saves root is a scratch tree may write while the user plays.
    if cfg!(debug_assertions) && std::env::var_os("DS2SM_SAVES_ROOT").is_some() && std::env::var_os("DS2SM_IGNORE_GAME").is_some() {
        return false;
    }
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
    #[cfg(target_os = "linux")]
    {
        fs::read_dir("/proc")
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()))
            .any(|e| fs::read(e.path().join("cmdline")).is_ok_and(|c| super::platform::cmdline_is_game(&c)))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        false
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn inside_any_sees_through_new_files_case_and_dots() {
        let base = std::env::temp_dir().join(format!("ds2sm-guard-{}", std::process::id()));
        let root = base.join("DarkSoulsII");
        let elsewhere = base.join("scratch");
        fs::create_dir_all(root.join("0110000100000666")).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        let roots = [root.clone()];

        // A save that does not exist yet, in an account folder that does not exist yet.
        assert!(inside_any(&root.join("0110000100000999").join("DS2SOFS0000.sl2"), &roots));
        assert!(inside_any(&root.join("0110000100000666").join(BACKUP_DIR).join("x.bak"), &roots));
        // `..` leading back in from a sibling folder.
        assert!(inside_any(&elsewhere.join("..").join("DarkSoulsII").join("DS2SOFS0000.sl2"), &roots));
        assert!(!inside_any(&elsewhere.join("DS2SOFS0000.sl2"), &roots));
        // A sibling whose name only starts like the root.
        assert!(!inside_any(&base.join("DarkSoulsII-copy").join("DS2SOFS0000.sl2"), &roots));
        if cfg!(windows) {
            let shouted = PathBuf::from(root.to_string_lossy().to_uppercase()).join("DS2SOFS0000.sl2");
            assert!(inside_any(&shouted, &roots));
        }
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn guard_keeps_the_real_folder_when_the_environment_points_elsewhere() {
        #[cfg(windows)]
        let system = dirs::data_dir().expect("the shell names a roaming folder");
        #[cfg(unix)]
        let system = super::super::platform::passwd_home().expect("the user database lists a home");
        let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/guard-env");
        // Unset, redirected, and pointing where the system does.
        for from_env in [None, Some(scratch.clone()), Some(system.clone())] {
            let bases = guard_bases(from_env.clone().map(PathBuf::into_os_string));
            assert!(bases.contains(&system), "{from_env:?}");
            assert_eq!(bases.contains(&scratch), from_env.as_ref() == Some(&scratch), "{from_env:?}");
            assert_eq!(bases.iter().filter(|b| **b == system).count(), 1, "{from_env:?}");
        }
        // Only development builds refuse; under `cargo test --release` the same path is allowed.
        #[cfg(windows)]
        assert_eq!(
            ensure_writable(&system.join("DarkSoulsII/0110000100000999/DS2SOFS0000.sl2")).err().map(|e| e.code),
            cfg!(debug_assertions).then_some("devRealSave")
        );
    }
}
