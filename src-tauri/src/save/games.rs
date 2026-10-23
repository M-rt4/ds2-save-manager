//! Where each release of Dark Souls II is installed, and where it keeps its saves. On
//! Windows both releases write to the same `%APPDATA%\DarkSoulsII\<account>` folders, each
//! under its own file name; on Linux and Steam Deck each release has its own Proton prefix
//! (see `platform`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::character;
use super::disk;
use super::edition::Edition;
use super::error::{Result, SaveError};
use super::file::{SaveFile, SLOT_COUNT};
use super::regulation::{self, Regulation};

pub use super::platform::vdf_values;
use super::platform::{same_path, steam_app, steam_libraries};

const EXE: &str = "DarkSoulsII.exe";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallSource {
    Steam,
    /// An installer's entry under the Windows "Uninstall" keys.
    Registry,
    /// A folder the user added by hand.
    Manual,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub path: String,
    /// `DarkSoulsII.exe`, when it is (still) there.
    pub exe: Option<String>,
    pub source: InstallSource,
    /// The executable's file version, e.g. "1.0.1.0". A save also carries the version of
    /// the game that wrote it, and the game refuses one that does not match.
    pub exe_version: Option<String>,
    /// The version of the install's `enc_regulation.bnd.dcx` (game parameters).
    pub regulation_version: Option<String>,
}

impl Install {
    fn new(path: String, exe: Option<String>, source: InstallSource) -> Self {
        let (exe_version, regulation_version) = exe.as_deref().map(|e| versions(Path::new(e))).unwrap_or_default();
        Self { path, exe, source, exe_version, regulation_version }
    }
}

type Versions = (Option<String>, Option<String>);
type VersionCache = HashMap<PathBuf, ((Option<SystemTime>, Option<SystemTime>), Versions)>;

/// Reading the versions scans a 30 MB executable and decrypts a 9 MB archive, so they are
/// kept per executable until it or the regulation file next to it changes.
static VERSIONS: Mutex<Option<VersionCache>> = Mutex::new(None);

/// The executable's file version and the regulation version of the install around it.
fn versions(exe: &Path) -> Versions {
    let Some(dir) = exe.parent() else { return (None, None) };
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    let stamp = (modified(exe), modified(&dir.join("enc_regulation.bnd.dcx")));
    let mut cache = VERSIONS.lock().unwrap();
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some((seen, v)) = cache.get(exe) {
        if *seen == stamp {
            return v.clone();
        }
    }
    let exe_version = regulation::exe_version(exe).ok().map(|(a, b, c, d)| format!("{a}.{b}.{c}.{d}"));
    let regulation_version = Regulation::from_install(dir).ok().map(|r| r.info().version);
    let v = (exe_version, regulation_version);
    cache.insert(exe.to_path_buf(), (stamp, v.clone()));
    v
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotBrief {
    pub slot: usize,
    pub name: String,
    pub level: u16,
}

/// A release's save in one folder under the saves root, or the place it would go.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSave {
    pub path: String,
    pub file_name: String,
    pub folder: String,
    /// SteamID64 as decimal text, when the folder is named after an account.
    pub account: Option<String>,
    pub exists: bool,
    pub size: u64,
    pub modified: u64,
    pub slots: Vec<SlotBrief>,
    pub error: Option<SaveError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub edition: Edition,
    pub steam_app_id: u32,
    pub installs: Vec<Install>,
    pub saves_root: Option<String>,
    pub saves: Vec<GameSave>,
}

/// A game folder the user added, remembered with the release it was recognised as so it
/// still shows under the right game after the folder goes away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualFolder {
    pub path: String,
    pub edition: Edition,
}

/// Every release with its installs and saves, Scholar first.
pub fn list(manual: &[ManualFolder]) -> Vec<GameInfo> {
    let mut found = detect_steam();
    found.extend(detect_uninstall_entries());
    for m in manual {
        let dir = PathBuf::from(&m.path);
        let exe = classify(&dir).filter(|(_, e)| *e == m.edition).map(|(exe, _)| display(&exe));
        found.push((m.edition, Install::new(m.path.clone(), exe, InstallSource::Manual)));
    }
    Edition::ALL
        .into_iter()
        .map(|edition| {
            let mut installs: Vec<Install> = Vec::new();
            for (e, install) in &found {
                if *e == edition && !installs.iter().any(|i| same_path(&i.path, &install.path)) {
                    installs.push(install.clone());
                }
            }
            let roots = disk::saves_roots(Some(edition));
            GameInfo {
                edition,
                steam_app_id: steam_app(edition),
                installs,
                saves_root: roots.first().map(|r| display(r)),
                saves: roots.iter().flat_map(|r| saves_in(r, edition)).collect(),
            }
        })
        .collect()
}

/// Recognises a game folder (or its `Game` subfolder): the path of `DarkSoulsII.exe` and
/// the release it belongs to.
pub fn classify(dir: &Path) -> Option<(PathBuf, Edition)> {
    let exe = [dir.join(EXE), dir.join("Game").join(EXE)].into_iter().find(|p| p.is_file())?;
    let folder = exe.parent()?;
    let by_app_id = fs::read_to_string(folder.join("steam_appid.txt")).ok().and_then(|s| {
        let id: u32 = s.trim().parse().ok()?;
        Edition::ALL.into_iter().find(|e| steam_app(*e) == id)
    });
    let by_api = || {
        if folder.join("steam_api64.dll").is_file() {
            Some(Edition::Scholar)
        } else if folder.join("steam_api.dll").is_file() {
            Some(Edition::Original)
        } else {
            None
        }
    };
    let edition = by_app_id.or_else(by_api).or_else(|| exe_bits(&exe))?;
    Some((exe, edition))
}

/// Scholar of the First Sin only ships a 64-bit executable, the original only a 32-bit one.
fn exe_bits(exe: &Path) -> Option<Edition> {
    use std::io::Read;
    let mut head = vec![0u8; 4096];
    let n = fs::File::open(exe).ok()?.read(&mut head).ok()?;
    let head = &head[..n];
    let pe = u32::from_le_bytes(head.get(0x3C..0x40)?.try_into().ok()?) as usize;
    if head.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    match u16::from_le_bytes(head.get(pe + 4..pe + 6)?.try_into().ok()?) {
        0x8664 => Some(Edition::Scholar),
        0x014C => Some(Edition::Original),
        _ => None,
    }
}

/// The release's save files in each folder under `root`, and for every account folder
/// without one, the file the game would create there.
pub fn saves_in(root: &Path, edition: Edition) -> Vec<GameSave> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root).into_iter().flatten().flatten().map(|d| d.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    let mut saves = Vec::new();
    for dir in dirs {
        let folder = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let account = disk::account_of_folder(&folder);
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|f| f.path())
            .filter(|p| {
                p.is_file()
                    && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("sl2"))
                    && p.file_name().and_then(|n| Edition::from_file_name(&n.to_string_lossy())) == Some(edition)
            })
            .collect();
        files.sort();
        if files.is_empty() && account.is_some() {
            saves.push(GameSave {
                path: display(&dir.join(edition.file_name())),
                file_name: edition.file_name().to_string(),
                folder,
                account: account.map(|a| a.to_string()),
                exists: false,
                size: 0,
                modified: 0,
                slots: Vec::new(),
                error: None,
            });
            continue;
        }
        for file in files {
            saves.extend(existing_save(&file, &folder, account));
        }
    }
    saves.sort_by_key(|s| (!s.exists, std::cmp::Reverse(s.modified)));
    saves
}

type Characters = (Vec<SlotBrief>, Option<SaveError>);
type CharacterCache = HashMap<PathBuf, ((u64, u64), Characters)>;

/// Decrypting a save takes a while and the games are listed again after every write, so
/// each file's characters are kept until its size or time changes.
static CHARACTERS: Mutex<Option<CharacterCache>> = Mutex::new(None);

fn characters(path: &Path, size: u64, modified: u64) -> Characters {
    let mut cache = CHARACTERS.lock().unwrap();
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some((seen, c)) = cache.get(path) {
        if *seen == (size, modified) {
            return c.clone();
        }
    }
    let c = match fs::read(path).map_err(SaveError::from).and_then(SaveFile::parse) {
        Ok(save) => (
            (0..SLOT_COUNT)
                .map(|i| character::summary(&save, i))
                .filter(|s| s.occupied)
                .map(|s| SlotBrief { slot: s.slot, name: s.name, level: s.level })
                .collect(),
            None,
        ),
        Err(e) => (Vec::new(), Some(e)),
    };
    cache.insert(path.to_path_buf(), ((size, modified), c.clone()));
    c
}

fn existing_save(path: &Path, folder: &str, account: Option<u64>) -> Option<GameSave> {
    let found = disk::found_save(path)?;
    let (slots, error) = characters(path, found.size, found.modified);
    Some(GameSave {
        path: found.path,
        file_name: found.file_name,
        folder: folder.to_string(),
        account: account.map(|a| a.to_string()),
        exists: true,
        size: found.size,
        modified: found.modified,
        slots,
        error,
    })
}

// ---------- Steam ----------

fn detect_steam() -> Vec<(Edition, Install)> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for steam in steam_dirs() {
        for library in steam_libraries(&steam) {
            if !libraries.iter().any(|l| same_path(&display(l), &display(&library))) {
                libraries.push(library);
            }
        }
    }
    let mut found = Vec::new();
    for library in &libraries {
        for edition in Edition::ALL {
            let manifest = library.join("steamapps").join(format!("appmanifest_{}.acf", steam_app(edition)));
            let Some(dir) = fs::read_to_string(&manifest).ok().and_then(|acf| vdf_values(&acf, "installdir").into_iter().next()) else {
                continue;
            };
            let path = library.join("steamapps").join("common").join(dir);
            let exe = classify(&path).map(|(exe, _)| display(&exe));
            if exe.is_some() || path.is_dir() {
                found.push((edition, Install::new(display(&path), exe, InstallSource::Steam)));
            }
        }
    }
    found
}

/// Steam installs: from the registry on Windows, from the usual places under the home folder
/// on Linux and Steam Deck (native, Flatpak, Snap).
fn steam_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        let from_registry = [
            (Hive::CurrentUser, r"Software\Valve\Steam", "SteamPath"),
            (Hive::LocalMachine, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
            (Hive::LocalMachine, r"SOFTWARE\Valve\Steam", "InstallPath"),
        ];
        from_registry
            .into_iter()
            .filter_map(|(hive, key, value)| registry::string(hive, key, value))
            .map(|p| PathBuf::from(p.replace('/', "\\")))
            .find(|p| p.join("steamapps").is_dir())
            .into_iter()
            .collect()
    } else {
        std::env::var_os("HOME").map(|h| super::platform::linux_steam_dirs(Path::new(&h))).unwrap_or_default()
    }
}

// ---------- Installers ----------

fn detect_uninstall_entries() -> Vec<(Edition, Install)> {
    let roots = [
        (Hive::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (Hive::LocalMachine, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (Hive::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    let mut found = Vec::new();
    for (hive, root) in roots {
        for sub in registry::subkeys(hive, root) {
            let key = format!(r"{root}\{sub}");
            if !registry::string(hive, &key, "DisplayName").is_some_and(|n| names_ds2(&n)) {
                continue;
            }
            let Some(location) = ["InstallLocation", "Inno Setup: App Path"]
                .into_iter()
                .filter_map(|v| registry::string(hive, &key, v))
                .map(|p| p.trim().trim_matches('"').trim_end_matches('\\').to_string())
                .find(|p| !p.is_empty())
            else {
                continue;
            };
            let path = PathBuf::from(&location);
            if let Some((exe, edition)) = classify(&path) {
                let source = if sub.starts_with("Steam App ") { InstallSource::Steam } else { InstallSource::Registry };
                found.push((edition, Install::new(location, Some(display(&exe)), source)));
            }
        }
    }
    found
}

/// Whether an installer's display name is Dark Souls II (either release), not III.
pub fn names_ds2(name: &str) -> bool {
    let plain: String = name.to_lowercase().chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    plain.contains("scholar of the first sin")
        || plain.contains("dark souls 2")
        || (plain.contains("dark souls ii") && !plain.contains("dark souls iii"))
}

// ---------- Folders the user added ----------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    game_folders: Vec<ManualFolder>,
    /// Whatever else other parts of the app keep in the same file.
    #[serde(flatten)]
    rest: serde_json::Map<String, serde_json::Value>,
}

fn read_settings(file: &Path) -> Settings {
    fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn write_settings(file: &Path, settings: &Settings) -> Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(|e| SaveError::new("io", e.to_string()))?;
    fs::write(file, json)?;
    Ok(())
}

/// The folders added by hand, from the app's settings file.
pub fn manual_folders(settings: &Path) -> Vec<ManualFolder> {
    read_settings(settings).game_folders
}

/// Remembers a game folder after checking it holds one of the releases.
pub fn add_folder(settings: &Path, folder: &Path) -> Result<ManualFolder> {
    let (_, edition) = classify(folder)
        .ok_or_else(|| SaveError::new("notGameFolder", format!("{EXE} was not found in this folder or its Game folder.")))?;
    let added = ManualFolder { path: display(folder), edition };
    let mut s = read_settings(settings);
    s.game_folders.retain(|m| !same_path(&m.path, &added.path));
    s.game_folders.push(added.clone());
    write_settings(settings, &s)?;
    Ok(added)
}

pub fn remove_folder(settings: &Path, folder: &str) -> Result<()> {
    let mut s = read_settings(settings);
    s.game_folders.retain(|m| !same_path(&m.path, folder));
    write_settings(settings, &s)
}

// ---------- Helpers ----------

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[derive(Clone, Copy)]
enum Hive {
    CurrentUser,
    LocalMachine,
}

#[cfg(windows)]
mod registry {
    use super::Hive;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    fn open(hive: Hive, key: &str) -> Option<RegKey> {
        let root = RegKey::predef(match hive {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        });
        root.open_subkey(key).ok()
    }

    pub fn string(hive: Hive, key: &str, value: &str) -> Option<String> {
        open(hive, key)?.get_value::<String, _>(value).ok()
    }

    pub fn subkeys(hive: Hive, key: &str) -> Vec<String> {
        open(hive, key).map(|k| k.enum_keys().flatten().collect()).unwrap_or_default()
    }
}

#[cfg(not(windows))]
mod registry {
    use super::Hive;

    pub fn string(_: Hive, _: &str, _: &str) -> Option<String> {
        None
    }

    pub fn subkeys(_: Hive, _: &str) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::testdata;

    fn scratch(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test-tmp").join(format!("games-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A minimal PE header with the given machine type, and a VS_FIXEDFILEINFO of 1.0.1.0.
    fn exe_for(machine: u16) -> Vec<u8> {
        let mut b = vec![0u8; 0x200];
        b[..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        b[0x100..0x104].copy_from_slice(&0xFEEF_04BDu32.to_le_bytes());
        b[0x108..0x10C].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        b[0x10C..0x110].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        b
    }

    #[test]
    fn reads_steam_library_and_manifest() {
        let vdf = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps"
		{
			"335300"		"18000000000"
		}
	}
	"1"
	{
		"path"		"Z:\\SteamLibrary"
	}
}"#;
        assert_eq!(vdf_values(vdf, "path"), [r"C:\Program Files (x86)\Steam", r"Z:\SteamLibrary"]);
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"335300\"\n\t\"installdir\"\t\t\"Dark Souls II Scholar of the First Sin\"\n}";
        assert_eq!(vdf_values(acf, "installdir"), ["Dark Souls II Scholar of the First Sin"]);
    }

    #[test]
    fn matches_installer_names() {
        assert!(names_ds2("Dark Souls 2: Scholar of the First Sin"));
        assert!(names_ds2("DARK SOULS™ II: Scholar of the First Sin"));
        assert!(names_ds2("Dark Souls II"));
        assert!(!names_ds2("DARK SOULS™ III"));
        assert!(!names_ds2("Dark Souls: Remastered"));
    }

    #[test]
    fn classifies_game_folders() {
        let root = scratch("classify");

        let repack = root.join("repack");
        fs::create_dir_all(&repack).unwrap();
        fs::write(repack.join(EXE), exe_for(0x8664)).unwrap();
        fs::write(repack.join("steam_appid.txt"), "335300\r\n").unwrap();
        assert_eq!(classify(&repack).map(|(_, e)| e), Some(Edition::Scholar));
        let install = Install::new(display(&repack), Some(display(&repack.join(EXE))), InstallSource::Manual);
        assert_eq!(install.exe_version.as_deref(), Some("1.0.1.0"));
        assert_eq!(install.regulation_version, None, "no enc_regulation.bnd.dcx here");

        let steam = root.join("Dark Souls II");
        fs::create_dir_all(steam.join("Game")).unwrap();
        fs::write(steam.join("Game").join(EXE), exe_for(0x14C)).unwrap();
        fs::write(steam.join("Game").join("steam_api.dll"), b"").unwrap();
        let (exe, edition) = classify(&steam).unwrap();
        assert_eq!(edition, Edition::Original);
        assert_eq!(exe, steam.join("Game").join(EXE));

        let bare = root.join("bare");
        fs::create_dir_all(&bare).unwrap();
        fs::write(bare.join(EXE), exe_for(0x8664)).unwrap();
        assert_eq!(classify(&bare).map(|(_, e)| e), Some(Edition::Scholar));

        assert!(classify(&root.join("missing")).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_saves_and_free_account_folders() {
        let root = scratch("saves");
        let mine = root.join(format!("{:016x}", testdata::STEAM));
        let other = root.join("0110000100000666");
        fs::create_dir_all(&mine).unwrap();
        fs::create_dir_all(&other).unwrap();
        fs::create_dir_all(root.join("not an account")).unwrap();
        let blocks = testdata::save_blocks(&[(0, "Nele", 37), (3, "Lucatiel", 30)], 64);
        fs::write(mine.join(Edition::Scholar.file_name()), testdata::container(Edition::Scholar, &blocks)).unwrap();
        fs::write(other.join(Edition::Original.file_name()), b"not a save").unwrap();

        let scholar = saves_in(&root, Edition::Scholar);
        assert_eq!(scholar.len(), 2);
        assert!(scholar[0].exists);
        assert_eq!(scholar[0].account.as_deref(), Some(testdata::STEAM.to_string().as_str()));
        let names: Vec<_> = scholar[0].slots.iter().map(|s| (s.slot, s.name.as_str(), s.level)).collect();
        // The level is read from the character block (sample attributes 90 + slot, minus 53).
        assert_eq!(names, [(0, "Nele", 37), (3, "Lucatiel", 40)]);
        assert!(!scholar[1].exists, "the other account has no Scholar save yet");
        assert_eq!(scholar[1].file_name, Edition::Scholar.file_name());
        assert_eq!(PathBuf::from(&scholar[1].path), other.join(Edition::Scholar.file_name()));

        let original = saves_in(&root, Edition::Original);
        assert_eq!(original.len(), 2);
        assert!(original[0].exists && original[0].error.is_some(), "an unreadable file is still listed");
        assert!(!original[1].exists);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn remembers_added_folders() {
        let root = scratch("settings");
        let settings = root.join("config").join("settings.json");
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(&settings, r#"{"lang":"tr"}"#).unwrap();
        let game = root.join("DS2");
        fs::create_dir_all(&game).unwrap();
        assert_eq!(add_folder(&settings, &game).unwrap_err().code, "notGameFolder");

        fs::write(game.join(EXE), exe_for(0x14C)).unwrap();
        let added = add_folder(&settings, &game).unwrap();
        assert_eq!(added.edition, Edition::Original);
        add_folder(&settings, &game).unwrap();
        assert_eq!(manual_folders(&settings), std::slice::from_ref(&added));
        let kept: serde_json::Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
        assert_eq!(kept["lang"], "tr", "other settings survive");

        remove_folder(&settings, &added.path.to_uppercase()).unwrap();
        assert!(manual_folders(&settings).is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
