//! The backups kept next to every save in `DS2 Save Manager Backups`: what each one holds,
//! and putting one back. A backup is named `<file>.<YYYYMMDD-HHMMSS>[-n].<reason>.bak`
//! (local time); the first releases wrote `<file>.<YYYYMMDD-HHMMSS>[-n].bak`, without a
//! reason, and those are listed too.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, NaiveDateTime, TimeZone};
use serde::Serialize;

use super::character;
use super::disk::{self, BACKUP_DIR};
use super::edition::Edition;
use super::error::{Result, SaveError};
use super::file::{SaveFile, SLOT_COUNT};

/// Why a backup was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    /// Asked for in the Backups tab.
    Manual,
    /// Before the app saved edits over the file.
    Save,
    /// Before a conversion or transfer wrote the file.
    Transfer,
    /// Before a backup was restored over the file.
    Restore,
}

impl Reason {
    const ALL: [Reason; 4] = [Reason::Manual, Reason::Save, Reason::Transfer, Reason::Restore];

    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Manual => "manual",
            Reason::Save => "save",
            Reason::Transfer => "transfer",
            Reason::Restore => "restore",
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a backup's file name says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupName {
    /// The save it was taken of, e.g. `DS2SOFS0000.sl2`.
    pub original: String,
    pub taken: NaiveDateTime,
    /// The `-n` that keeps two backups of the same second apart.
    pub seq: u32,
    /// `None` on backups from before reasons were recorded.
    pub reason: Option<Reason>,
}

pub fn parse_name(file_name: &str) -> Option<BackupName> {
    let cut = file_name.len().checked_sub(4)?;
    let stem = file_name.get(cut..).filter(|ext| ext.eq_ignore_ascii_case(".bak")).map(|_| &file_name[..cut])?;
    let (rest, last) = stem.rsplit_once('.')?;
    let (original, stamp, reason) = match Reason::ALL.into_iter().find(|r| r.as_str() == last) {
        Some(reason) => {
            let (original, stamp) = rest.rsplit_once('.')?;
            (original, stamp, Some(reason))
        }
        None => (rest, last, None),
    };
    // `YYYYMMDD-HHMMSS` is 15 characters; a `-n` may follow.
    let seq = match stamp.get(15..)? {
        "" => 0,
        more => more.strip_prefix('-')?.parse().ok()?,
    };
    let taken = NaiveDateTime::parse_from_str(stamp.get(..15)?, "%Y%m%d-%H%M%S").ok()?;
    (!original.is_empty()).then(|| BackupName { original: original.to_string(), taken, seq, reason })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCharacter {
    pub slot: usize,
    pub name: String,
    pub level: u16,
}

/// What a backup holds, found by decrypting it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    /// Whether it opens as a Dark Souls II save at all.
    pub readable: bool,
    pub edition: Option<Edition>,
    /// The occupied slots.
    pub characters: Vec<BackupCharacter>,
}

/// One backup as the Backups tab lists it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub path: String,
    pub file_name: String,
    /// When it was taken, in ms since the epoch.
    pub created: i64,
    pub reason: Option<Reason>,
    pub size: u64,
    #[serde(flatten)]
    pub contents: Contents,
}

/// The second in the name, then the moment of the copy (backups of one second with
/// different reasons share a number), then the number.
type SortKey = (NaiveDateTime, Option<SystemTime>, u32);

/// The backup folder that belongs to a save.
pub fn dir_of(save: &Path) -> Option<PathBuf> {
    save.parent().map(|p| p.join(BACKUP_DIR))
}

/// The backups of `save`, newest first.
pub fn list(save: &Path) -> Vec<Backup> {
    let (Some(dir), Some(name)) = (dir_of(save), save.file_name()) else { return Vec::new() };
    let name = name.to_string_lossy();
    let mut found: Vec<(SortKey, Backup)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| describe(&entry.path()))
        .filter(|(name_info, _, _)| name_info.original.eq_ignore_ascii_case(&name))
        .map(|(_, key, backup)| (key, backup))
        .collect();
    found.sort_by_key(|(key, _)| std::cmp::Reverse(*key));
    found.into_iter().map(|(_, b)| b).collect()
}

fn describe(path: &Path) -> Option<(BackupName, SortKey, Backup)> {
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let parsed = parse_name(&file_name)?;
    let meta = fs::metadata(path).ok().filter(|m| m.is_file())?;
    let created = Local.from_local_datetime(&parsed.taken).earliest().map_or_else(|| modified_ms(&meta), |t| t.timestamp_millis());
    let key = (parsed.taken, meta.created().ok(), parsed.seq);
    let backup = Backup {
        path: path.to_string_lossy().into_owned(),
        file_name,
        created,
        reason: parsed.reason,
        size: meta.len(),
        contents: contents(path, &meta),
    };
    Some((parsed, key, backup))
}

fn modified_ms(meta: &fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64)
}

/// What was read from each backup, with the size and modification time it had then.
type ContentsCache = HashMap<PathBuf, (u64, i64, Contents)>;

/// Decrypting a save takes a moment and backups never change, so each is read once per
/// size and modification time.
static CONTENTS: LazyLock<Mutex<ContentsCache>> = LazyLock::new(Default::default);

fn contents(path: &Path, meta: &fs::Metadata) -> Contents {
    let (len, ms) = (meta.len(), modified_ms(meta));
    if let Some((l, m, c)) = CONTENTS.lock().unwrap().get(path) {
        if (*l, *m) == (len, ms) {
            return c.clone();
        }
    }
    let c = read_contents(path);
    CONTENTS.lock().unwrap().insert(path.to_path_buf(), (len, ms, c.clone()));
    c
}

pub fn read_contents(path: &Path) -> Contents {
    match fs::read(path).map_err(SaveError::from).and_then(SaveFile::parse) {
        Ok(save) => Contents {
            readable: true,
            edition: Some(save.source_edition),
            characters: (0..SLOT_COUNT)
                .map(|i| character::summary(&save, i))
                .filter(|s| s.occupied)
                .map(|s| BackupCharacter { slot: s.slot, name: s.name, level: s.level })
                .collect(),
        },
        Err(_) => Contents { readable: false, edition: None, characters: Vec::new() },
    }
}

/// Backs up `save` now and describes the copy.
pub fn create(save: &Path, reason: Reason) -> Result<Backup> {
    let path = disk::backup(save, reason)?.ok_or_else(|| SaveError::new("backupNoSave", "There is no file to back up."))?;
    describe(&path).map(|(_, _, b)| b).ok_or_else(|| SaveError::new("io", "The backup was written but can't be found again."))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    pub path: String,
    /// Where the file that was there before went.
    pub backup: Option<String>,
    pub edition: Edition,
}

/// Puts a backup back as `target`: checks it is a readable save of the release `target`
/// belongs to, backs up the current file, writes the backup over it and reads it back.
/// The caller makes sure the game is not running.
pub fn restore(backup: &Path, target: &Path) -> Result<Restored> {
    let backup = checked_backup(backup)?;
    let bytes = fs::read(&backup)?;
    let save = SaveFile::parse(bytes.clone())
        .map_err(|e| SaveError::new("backupUnreadable", format!("The backup can't be read: {}", e.message)))?;
    if let Some(expected) = expected_edition(target) {
        if expected != save.source_edition {
            return Err(SaveError::new(
                "backupEdition",
                format!(
                    "This backup is a {} save; {} is read by {}.",
                    edition_name(save.source_edition),
                    target.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                    edition_name(expected)
                ),
            ));
        }
    }
    let previous = disk::backup(target, Reason::Restore)?;
    disk::write_atomic(target, &bytes)?;
    let written = fs::read(target)?;
    if written != bytes || SaveFile::parse(written).is_err() {
        return Err(SaveError::new(
            "backupVerify",
            "The restored file did not read back the same. The file that was there before is in the backups.",
        ));
    }
    Ok(Restored {
        path: target.to_string_lossy().into_owned(),
        backup: previous.map(|p| p.to_string_lossy().into_owned()),
        edition: save.source_edition,
    })
}

/// Deletes one backup, and nothing that isn't one.
pub fn delete(backup: &Path) -> Result<()> {
    let backup = checked_backup(backup)?;
    fs::remove_file(backup)?;
    Ok(())
}

/// Only `.bak` files directly inside a `DS2 Save Manager Backups` folder are ever restored
/// or deleted. Links and `..` are resolved first, so neither leads out of it.
fn checked_backup(path: &Path) -> Result<PathBuf> {
    let real = fs::canonicalize(path)?;
    let in_folder = real.parent().and_then(|p| p.file_name()).is_some_and(|d| d.to_string_lossy().eq_ignore_ascii_case(BACKUP_DIR));
    let is_bak = real.extension().is_some_and(|e| e.eq_ignore_ascii_case("bak"));
    if !(in_folder && is_bak && fs::metadata(&real)?.is_file()) {
        return Err(SaveError::new(
            "backupOutside",
            "Only .bak files in a DS2 Save Manager Backups folder can be restored or deleted.",
        ));
    }
    Ok(real)
}

/// The release a save file is read by: the game's own file names say it; any other file
/// keeps the release of what is in it now.
fn expected_edition(target: &Path) -> Option<Edition> {
    target
        .file_name()
        .and_then(|n| Edition::from_file_name(&n.to_string_lossy()))
        .or_else(|| fs::read(target).ok().and_then(|b| SaveFile::parse(b).ok()).map(|s| s.source_edition))
}

fn edition_name(e: Edition) -> &'static str {
    match e {
        Edition::Original => "Dark Souls II",
        Edition::Scholar => "Scholar of the First Sin",
    }
}

#[cfg(test)]
mod tests {
    use super::super::testdata;
    use super::*;

    /// A fresh folder under the crate's own `target`, so tests stay on the project's drive.
    fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("test-scratch").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sl2(edition: Edition, names: &[(usize, &str, u16)]) -> Vec<u8> {
        testdata::container(edition, &testdata::save_blocks(names, 64))
    }

    fn names(b: &Backup) -> Vec<&str> {
        b.contents.characters.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn parses_backup_names() {
        let at = |s: &str| NaiveDateTime::parse_from_str(s, "%Y%m%d-%H%M%S").unwrap();
        let p = parse_name("DS2SOFS0000.sl2.20261023-143205.manual.bak").unwrap();
        assert_eq!(p, BackupName { original: "DS2SOFS0000.sl2".into(), taken: at("20261023-143205"), seq: 0, reason: Some(Reason::Manual) });
        let p = parse_name("DS2SOFS0000.sl2.20261023-143205-2.restore.BAK").unwrap();
        assert_eq!((p.seq, p.reason), (2, Some(Reason::Restore)));
        // The first releases' names, without a reason.
        let p = parse_name("DARKSII0000.sl2.20261023-143205.bak").unwrap();
        assert_eq!((p.original.as_str(), p.seq, p.reason), ("DARKSII0000.sl2", 0, None));
        assert_eq!(parse_name("DARKSII0000.sl2.20261023-143205-1.bak").unwrap().seq, 1);
        for bad in [
            "DS2SOFS0000.sl2",
            "DS2SOFS0000.sl2.bak",
            "notes.bak",
            "DS2SOFS0000.sl2.20261323-143205.save.bak",
            "DS2SOFS0000.sl2.20261023-143205.weird.bak",
            "DS2SOFS0000.sl2.20261023-143205-x.bak",
            ".20261023-143205.save.bak",
            "ö.bak",
        ] {
            assert_eq!(parse_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn takes_lists_restores_and_deletes() {
        let root = scratch("backups-cycle");
        let save = root.join(Edition::Scholar.file_name());
        let first = sl2(Edition::Scholar, &[(0, "Nele", 37), (4, "Pate", 12)]);
        fs::write(&save, &first).unwrap();

        // Two manual backups in the same second stay apart.
        let a = create(&save, Reason::Manual).unwrap();
        let b = create(&save, Reason::Manual).unwrap();
        assert_ne!(a.path, b.path);
        assert!(a.file_name.starts_with("DS2SOFS0000.sl2.") && a.file_name.ends_with(".manual.bak"), "{}", a.file_name);
        assert!(a.contents.readable);
        assert_eq!(a.contents.edition, Some(Edition::Scholar));
        assert_eq!(names(&a), ["Nele", "Pate"]);
        let pate = character::summary(&SaveFile::parse(first.clone()).unwrap(), 4);
        assert_eq!((a.contents.characters[1].slot, a.contents.characters[1].level), (4, pate.level));

        let dir = dir_of(&save).unwrap();
        // An old-style name, an unreadable file, the other release's file and a stray file.
        let old = dir.join("DS2SOFS0000.sl2.20240101-101010.bak");
        fs::write(&old, sl2(Edition::Scholar, &[(2, "Lucatiel", 50)])).unwrap();
        let junk = dir.join("DS2SOFS0000.sl2.20240102-101010-1.save.bak");
        fs::write(&junk, b"not a save").unwrap();
        let wrong_release = dir.join("DS2SOFS0000.sl2.20230101-000000.transfer.bak");
        fs::write(&wrong_release, sl2(Edition::Original, &[(0, "Benhart", 20)])).unwrap();
        fs::write(dir.join("DARKSII0000.sl2.20240101-101010.save.bak"), sl2(Edition::Original, &[])).unwrap();
        fs::write(dir.join("readme.txt"), b"x").unwrap();

        let listed = list(&save);
        let files: Vec<_> = listed.iter().map(|x| x.file_name.as_str()).collect();
        assert_eq!(files, [b.file_name.as_str(), &a.file_name, "DS2SOFS0000.sl2.20240102-101010-1.save.bak", "DS2SOFS0000.sl2.20240101-101010.bak", "DS2SOFS0000.sl2.20230101-000000.transfer.bak"]);
        assert_eq!(listed[2].reason, Some(Reason::Save));
        assert!(!listed[2].contents.readable && listed[2].contents.edition.is_none());
        assert_eq!((listed[3].reason, names(&listed[3])), (None, vec!["Lucatiel"]));
        assert_eq!(listed[4].contents.edition, Some(Edition::Original));
        let local = Local.from_local_datetime(&NaiveDateTime::parse_from_str("20240101-101010", "%Y%m%d-%H%M%S").unwrap());
        assert_eq!(listed[3].created, local.earliest().unwrap().timestamp_millis());

        // The game moved on; restoring the old backup brings Lucatiel back and keeps what
        // was there as a "restore" backup.
        let moved_on = sl2(Edition::Scholar, &[(0, "Nele", 60)]);
        fs::write(&save, &moved_on).unwrap();
        let r = restore(&old, &save).unwrap();
        assert_eq!(r.edition, Edition::Scholar);
        assert_eq!(fs::read(&save).unwrap(), fs::read(&old).unwrap());
        let kept = PathBuf::from(r.backup.unwrap());
        assert!(kept.to_string_lossy().ends_with(".restore.bak"));
        assert_eq!(fs::read(&kept).unwrap(), moved_on);
        assert_eq!(list(&save)[0].reason, Some(Reason::Restore));

        // Refused: the other release's save, an unreadable file, anything outside the folder.
        let before = fs::read(&save).unwrap();
        assert_eq!(restore(&wrong_release, &save).unwrap_err().code, "backupEdition");
        assert_eq!(restore(&junk, &save).unwrap_err().code, "backupUnreadable");
        assert_eq!(restore(&save, &save).unwrap_err().code, "backupOutside");
        assert_eq!(fs::read(&save).unwrap(), before);
        assert_eq!(list(&save).len(), 6, "a refused restore takes no backup");

        assert_eq!(delete(&save).unwrap_err().code, "backupOutside");
        assert_eq!(delete(&dir.join("readme.txt")).unwrap_err().code, "backupOutside");
        assert_eq!(delete(&dir.join("..").join(BACKUP_DIR).join("..").join(Edition::Scholar.file_name())).unwrap_err().code, "backupOutside");
        assert!(save.exists());
        delete(Path::new(&a.path)).unwrap();
        assert!(!Path::new(&a.path).exists());
        assert_eq!(list(&save).len(), 5);

        assert_eq!(create(&root.join("missing.sl2"), Reason::Manual).unwrap_err().code, "backupNoSave");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn restores_into_a_file_named_for_neither_release() {
        let root = scratch("backups-other-name");
        let save = root.join("copy.sl2");
        fs::write(&save, sl2(Edition::Original, &[(0, "Nele", 37)])).unwrap();
        let b = create(&save, Reason::Manual).unwrap();
        fs::write(&save, sl2(Edition::Original, &[])).unwrap();
        restore(Path::new(&b.path), &save).unwrap();
        assert_eq!(fs::read(&save).unwrap(), fs::read(&b.path).unwrap());
        // A Scholar file there now makes an original backup the wrong release.
        fs::write(&save, sl2(Edition::Scholar, &[])).unwrap();
        assert_eq!(restore(Path::new(&b.path), &save).unwrap_err().code, "backupEdition");
        let _ = fs::remove_dir_all(&root);
    }
}
