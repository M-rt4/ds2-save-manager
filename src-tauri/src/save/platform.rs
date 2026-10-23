//! Where Dark Souls II keeps things on each platform the app supports: Windows, Linux and
//! Steam Deck. On Windows both releases write to `%APPDATA%\DarkSoulsII`. On Linux the
//! (Windows) game runs inside a Wine prefix: Steam gives each release its own Proton prefix
//! (`steamapps/compatdata/<appid>/pfx`), so the two releases keep separate save folders, and
//! Heroic, Lutris or plain Wine keep theirs elsewhere. The Linux functions take the home
//! folder as an argument so they can be tested on any system.

use std::fs;
use std::path::{Path, PathBuf};

use super::edition::Edition;

/// Steam app ids of the two releases.
pub fn steam_app(edition: Edition) -> u32 {
    match edition {
        Edition::Original => 236430,
        Edition::Scholar => 335300,
    }
}

/// Steam installs under a Linux home: the native one, the legacy symlinks, Flatpak and Snap.
pub fn linux_steam_dirs(home: &Path) -> Vec<PathBuf> {
    let candidates = [
        ".local/share/Steam",
        ".steam/steam",
        ".steam/root",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        "snap/steam/common/.local/share/Steam",
    ];
    let mut found: Vec<PathBuf> = Vec::new();
    for c in candidates {
        let dir = home.join(c);
        if !dir.join("steamapps").is_dir() {
            continue;
        }
        // ~/.steam/steam is usually a symlink to ~/.local/share/Steam.
        let real = fs::canonicalize(&dir).unwrap_or(dir);
        if !found.contains(&real) {
            found.push(real);
        }
    }
    found
}

/// A Steam install's libraries: the install itself plus every `path` in libraryfolders.vdf
/// (on a Steam Deck this is where the microSD card shows up).
pub fn steam_libraries(steam: &Path) -> Vec<PathBuf> {
    let mut libraries = vec![steam.to_path_buf()];
    if let Ok(vdf) = fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
        for path in vdf_values(&vdf, "path") {
            let path = PathBuf::from(path);
            if !libraries.contains(&path) {
                libraries.push(path);
            }
        }
    }
    libraries
}

/// The `DarkSoulsII` save folders inside a Wine prefix, for whatever user names it has.
pub fn prefix_save_roots(prefix: &Path) -> Vec<PathBuf> {
    let users = prefix.join("drive_c").join("users");
    let mut out: Vec<PathBuf> = fs::read_dir(&users)
        .into_iter()
        .flatten()
        .flatten()
        .map(|u| u.path().join("AppData").join("Roaming").join("DarkSoulsII"))
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// Every save folder a release can have on Linux: its Proton prefix in each Steam library,
/// then prefixes outside Steam (plain Wine, Heroic, Lutris), which can hold either release.
pub fn linux_save_roots(home: &Path, steam_dirs: &[PathBuf], edition: Edition) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |roots: Vec<PathBuf>| {
        for r in roots {
            if !out.contains(&r) {
                out.push(r);
            }
        }
    };
    for steam in steam_dirs {
        for library in steam_libraries(steam) {
            let prefix = library.join("steamapps").join("compatdata").join(steam_app(edition).to_string()).join("pfx");
            push(prefix_save_roots(&prefix));
        }
    }
    push(prefix_save_roots(&home.join(".wine")));
    for base in [home.join("Games").join("Heroic").join("Prefixes"), home.join("Games")] {
        for dir in fs::read_dir(&base).into_iter().flatten().flatten() {
            push(prefix_save_roots(&dir.path()));
            push(prefix_save_roots(&dir.path().join("pfx")));
        }
    }
    out
}

/// Where the game keeps saves on Linux or would make them, whether or not they exist yet:
/// each release's whole Proton prefix in every Steam library, and the DarkSoulsII folder of
/// every user of a plain Wine, Heroic or Lutris prefix. Development builds write inside none
/// of them (`disk::ensure_writable`), so a first save in a prefix where the game has not saved
/// yet is refused as well.
pub fn linux_guard_roots(home: &Path, steam_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for steam in steam_dirs {
        for library in steam_libraries(steam) {
            for edition in Edition::ALL {
                out.push(library.join("steamapps").join("compatdata").join(steam_app(edition).to_string()));
            }
        }
    }
    let mut prefixes = vec![home.join(".wine")];
    for base in [home.join("Games").join("Heroic").join("Prefixes"), home.join("Games")] {
        for dir in fs::read_dir(&base).into_iter().flatten().flatten() {
            prefixes.push(dir.path().join("pfx"));
            prefixes.push(dir.path());
        }
    }
    for prefix in prefixes {
        for user in fs::read_dir(prefix.join("drive_c").join("users")).into_iter().flatten().flatten() {
            out.push(user.path().join("AppData").join("Roaming").join("DarkSoulsII"));
        }
    }
    out
}

/// The values of every `"key" "value"` pair named `key` in a Valve KeyValues text
/// (`libraryfolders.vdf`, `appmanifest_*.acf`), unescaped.
pub fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let tokens = quoted(line.trim());
        if let [k, v] = tokens.as_slice() {
            if k.eq_ignore_ascii_case(key) {
                out.push(v.clone());
            }
        }
    }
    out
}

fn quoted(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => s.extend(chars.next()),
                c => s.push(c),
            }
        }
        tokens.push(s);
    }
    tokens
}

/// Whether a running process's command line names the game: on Linux Proton and Wine run
/// `DarkSoulsII.exe`, which shows in `/proc/<pid>/cmdline`.
pub fn cmdline_is_game(cmdline: &[u8]) -> bool {
    String::from_utf8_lossy(cmdline).to_ascii_lowercase().contains("darksoulsii.exe")
}

/// Same path, by the rules of the platform: Windows ignores case and the separator kind.
pub fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        let norm = |s: &str| s.trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase();
        norm(a) == norm(b)
    } else {
        a.trim_end_matches('/') == b.trim_end_matches('/')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch_dir(p: &Path) {
        fs::create_dir_all(p).unwrap();
    }

    #[test]
    fn finds_proton_and_wine_save_folders() {
        let home = std::env::temp_dir().join(format!("ds2sm-linux-home-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let steam = home.join(".local/share/Steam");
        let card = home.join("sdcard");
        touch_dir(&steam.join("steamapps"));
        fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n", card.display().to_string().replace('\\', "\\\\")),
        )
        .unwrap();
        let roaming = |prefix: &Path| prefix.join("drive_c/users/steamuser/AppData/Roaming/DarkSoulsII");
        let scholar_on_card = roaming(&card.join("steamapps/compatdata/335300/pfx"));
        let original_internal = roaming(&steam.join("steamapps/compatdata/236430/pfx"));
        let heroic = home.join("Games/Heroic/Prefixes/DS2/pfx/drive_c/users/me/AppData/Roaming/DarkSoulsII");
        for d in [&scholar_on_card, &original_internal, &heroic] {
            touch_dir(d);
        }

        let dirs = linux_steam_dirs(&home);
        assert_eq!(dirs.len(), 1);
        let scholar = linux_save_roots(&home, &dirs, Edition::Scholar);
        let original = linux_save_roots(&home, &dirs, Edition::Original);
        let canon = |p: &Path| fs::canonicalize(p).unwrap();
        let canon_all = |v: Vec<PathBuf>| v.iter().map(|p| canon(p)).collect::<Vec<_>>();
        assert_eq!(canon_all(scholar), vec![canon(&scholar_on_card), canon(&heroic)]);
        assert_eq!(canon_all(original), vec![canon(&original_internal), canon(&heroic)]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn guards_save_folders_the_game_has_not_made_yet() {
        let home = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch/linux-guard-home");
        let _ = fs::remove_dir_all(&home);
        let steam = home.join(".local/share/Steam");
        let card = home.join("sdcard");
        touch_dir(&steam.join("steamapps"));
        fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n", card.display().to_string().replace('\\', "\\\\")),
        )
        .unwrap();
        // Scholar's prefix exists on the card but the game never saved there; a Lutris prefix
        // has a user and no DarkSoulsII folder.
        touch_dir(&card.join("steamapps/compatdata/335300/pfx/drive_c/users/steamuser/AppData/Roaming"));
        touch_dir(&home.join("Games/ds2/drive_c/users/me"));

        let dirs = linux_steam_dirs(&home);
        assert!(linux_save_roots(&home, &dirs, Edition::Scholar).is_empty(), "no save folder exists yet");
        let guarded = linux_guard_roots(&home, &dirs);
        let first_save = card.join("steamapps/compatdata/335300/pfx/drive_c/users/steamuser/AppData/Roaming/DarkSoulsII/0110000100000666/DS2SOFS0000.sl2");
        let lutris = home.join("Games/ds2/drive_c/users/me/AppData/Roaming/DarkSoulsII/0110000100000666/DARKSII0000.sl2");
        // The install itself, as linux_steam_dirs gives it (canonical).
        let original_internal = dirs[0].join("steamapps/compatdata/236430/pfx/drive_c/users/steamuser/AppData/Roaming/DarkSoulsII/x.sl2");
        for target in [&first_save, &lutris, &original_internal] {
            assert!(guarded.iter().any(|root| target.starts_with(root)), "{} is guarded", target.display());
        }
        assert!(!guarded.iter().any(|root| home.join("scratch/DS2SOFS0000.sl2").starts_with(root)));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn recognises_the_game_process() {
        assert!(cmdline_is_game(b"Z:\\home\\deck\\.local\\share\\Steam\\steamapps\\common\\Dark Souls II Scholar of the First Sin\\Game\\DarkSoulsII.exe\0"));
        assert!(!cmdline_is_game(b"/usr/bin/steam\0-silent"));
    }
}
