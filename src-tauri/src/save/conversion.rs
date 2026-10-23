//! Building the save a conversion or a character transfer writes. A written save always ends
//! up in the target game's own format: its key, its regulation copy and version stamp, its
//! entry layout and the account its folder belongs to. Every difference from what came in is
//! reported as a `Change`. Reading the files involved and writing the result stay with the
//! caller.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::character;
use super::chunks;
use super::disk;
use super::edition::Edition;
use super::error::{Result, SaveError};
use super::file::{character_entry, world_entry, SaveFile, HEADER_ENTRY, SLOT_COUNT, SUMMARY_ENTRY};
use super::header;
use super::regulation::{self, RegulationInfo};
use super::transfer::{self, TargetFormat};

/// Where the written save goes.
pub struct Target<'a> {
    /// `None` moves the whole save; `Some((from, to))` one character into slot `to`.
    pub slot: Option<(usize, usize)>,
    pub edition: Edition,
    pub path: &'a Path,
    /// A SteamID64 the user typed; otherwise the account the target folder belongs to.
    pub account: Option<&'a str>,
    /// Whether `path` is the source's own file.
    pub same_file: bool,
}

/// What the target game expects, best source first: its install, then the target file, then
/// another local save of that release (asked only when nothing better is at hand).
pub struct Formats<'a> {
    /// The installed target game's format, with the folder it was read from.
    pub install: Option<(Result<TargetFormat>, String)>,
    pub other_save: &'a mut dyn FnMut() -> Option<(TargetFormat, PathBuf)>,
}

/// Where the target format came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatSource {
    /// `install`, `targetFile` or `otherSave`.
    pub kind: &'static str,
    pub path: String,
    pub regulation: RegulationInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAccount {
    pub id: String,
    pub folder: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPlan {
    pub source: Option<String>,
    pub target_file: Option<String>,
    pub target_folder: Option<String>,
    pub result: Option<String>,
    /// Where `result` came from: `manual`, `folder`, `targetFile` or `source`.
    pub origin: &'static str,
    /// Filled in by the caller, which knows the saves root.
    pub local: Vec<LocalAccount>,
}

/// One difference between what goes in and what gets written.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Change {
    #[serde(rename_all = "camelCase")]
    Key { from: Edition, to: Edition },
    #[serde(rename_all = "camelCase")]
    Regulation { from: Option<RegulationInfo>, to: RegulationInfo, source: FormatSource },
    #[serde(rename_all = "camelCase")]
    TitleVersion { from: u32, to: u32 },
    #[serde(rename_all = "camelCase")]
    EntrySize { entry: usize, from: usize, to: usize },
    #[serde(rename_all = "camelCase")]
    Account { from: Option<String>, to: String, origin: &'static str },
    #[serde(rename_all = "camelCase")]
    Slot { slot: usize, before: Option<String>, after: Option<String> },
    #[serde(rename_all = "camelCase")]
    KeptTargetSystem { path: String },
    #[serde(rename_all = "camelCase")]
    NewFile { path: String },
    #[serde(rename_all = "camelCase")]
    Backup { path: String },
}

pub struct Built {
    pub out: SaveFile,
    pub changes: Vec<Change>,
    pub format: FormatSource,
    pub accounts: AccountPlan,
}

/// Whether a format can stand for `edition`: that release's regulation (Scholar's are
/// versioned 02xxxxxx, the original's 01xxxxxx) and that release's entry layout. A save
/// another tool re-keyed can carry the right key with the other release's insides.
pub fn trusted(edition: Edition, fmt: &TargetFormat) -> bool {
    let major = if edition == Edition::Scholar { "02" } else { "01" };
    let (capacity, title_version) = transfer::edition_layout(edition);
    fmt.edition == edition
        && fmt.regulation.info().version.starts_with(major)
        && fmt.capacity == capacity
        && fmt.title_version == title_version
}

pub fn folder_account(path: &Path) -> Option<u64> {
    path.parent().and_then(|p| p.file_name()).and_then(|f| disk::account_of_folder(&f.to_string_lossy()))
}

pub fn parse_account(text: &str) -> Result<u64> {
    let id: u64 = text.trim().parse().map_err(|_| SaveError::new("badAccount", "A SteamID64 is a 17-digit number."))?;
    if id >> 32 != header::STEAM_ID_HIGH || id as u32 == 0 {
        return Err(SaveError::new("badAccount", "That is not an individual Steam account id."));
    }
    Ok(id)
}

/// The save already at the target path: `None` when there is none, an error when there is a
/// file that can't be read (it is never written over, and nothing can be merged into it).
pub fn read_target(path: &Path) -> Result<Option<SaveFile>> {
    if !path.exists() {
        return Ok(None);
    }
    let unreadable = |e: SaveError| {
        SaveError::new("targetUnreadable", format!("The target file can't be read, so it is not written over: {}", e.message))
    };
    let bytes = std::fs::read(path).map_err(|e| unreadable(e.into()))?;
    SaveFile::parse(bytes).map(Some).map_err(unreadable)
}

/// A file is read with one release's key, chosen by its name: it can't change release in place.
pub fn check_same_file(source: &SaveFile, target: &Target) -> Result<()> {
    if target.same_file && target.edition != source.source_edition {
        return Err(SaveError::new("sameFile", "The target is the source file itself."));
    }
    Ok(())
}

/// Builds the save that would be written, without writing it. `base` is the save already at
/// the target path.
pub fn build(source: &SaveFile, target: &Target, base: Option<&SaveFile>, formats: Formats) -> Result<Built> {
    check_same_file(source, target)?;
    let path = target.path;
    let path_text = path.to_string_lossy().into_owned();
    let mut changes = Vec::new();

    // The format the written file must have. A save being repaired can't vouch for itself.
    let repair = target.same_file && target.slot.is_none();
    let base_format = base
        .filter(|b| !repair && b.source_edition == target.edition)
        .and_then(|b| TargetFormat::from_save(b).ok())
        .filter(|f| trusted(target.edition, f));
    let (fmt, format) = match (formats.install, base_format) {
        (Some((Ok(f), dir)), _) => {
            let info = f.regulation.info();
            (f, FormatSource { kind: "install", path: dir, regulation: info })
        }
        (Some((Err(e), _)), _) => return Err(e),
        (None, Some(f)) => {
            let info = f.regulation.info();
            (f, FormatSource { kind: "targetFile", path: path_text.clone(), regulation: info })
        }
        (None, None) => match (formats.other_save)() {
            Some((f, p)) => {
                let info = f.regulation.info();
                (f, FormatSource { kind: "otherSave", path: p.to_string_lossy().into_owned(), regulation: info })
            }
            None => {
                return Err(SaveError::new(
                    "needsGameInfo",
                    "Nothing tells what the target game expects: choose its install folder, or start it once so it writes a save.",
                ))
            }
        },
    };

    // Characters.
    let mut out = match (target.slot, base) {
        (None, Some(b)) => {
            let merged = transfer::merge(source, b)?;
            for s in 0..SLOT_COUNT {
                let before = character::summary(b, s);
                let after = character::summary(&merged, s);
                let (before, after) = (before.occupied.then_some(before.name), after.occupied.then_some(after.name));
                let same_data = b.block(character_entry(s)) == merged.block(character_entry(s))
                    && b.block(world_entry(s)) == merged.block(world_entry(s));
                // Empty slots still hold leftover bytes (deleted characters, old data); an
                // empty slot staying empty is no change to anyone.
                if !same_data && (before.is_some() || after.is_some()) {
                    changes.push(Change::Slot { slot: s, before, after });
                }
            }
            changes.push(Change::KeptTargetSystem { path: path_text.clone() });
            merged
        }
        (None, None) => {
            changes.push(Change::NewFile { path: path_text.clone() });
            transfer::convert(source, target.edition)
        }
        (Some((from, to)), Some(b)) => {
            if from >= SLOT_COUNT || !character::summary(source, from).occupied {
                return Err(SaveError::invalid("The source slot has no character."));
            }
            let mut out = b.clone();
            transfer::transfer(source, from, &mut out, to)?;
            let before = character::summary(b, to.min(SLOT_COUNT - 1));
            changes.push(Change::Slot {
                slot: to,
                before: before.occupied.then_some(before.name),
                after: Some(character::summary(source, from).name),
            });
            out
        }
        (Some(_), None) => {
            return Err(SaveError::new("needsTarget", "One character can only go into an existing save of the target release."))
        }
    };

    // Format: key, regulation copy, title list version, entry 21 capacity.
    let reference = match (base, target.slot) {
        (Some(b), Some(_)) => b,
        _ => &out,
    };
    let before_reg = transfer::regulation_info(reference);
    let before_title = chunks::find(reference.block(SUMMARY_ENTRY), header::TITLE_CHUNK).map(|c| c.version);
    let before_cap = reference.block(regulation::ENTRY).len();
    let source_key = source.source_edition;
    transfer::conform(&mut out, &fmt)?;
    if source_key != fmt.edition {
        changes.push(Change::Key { from: source_key, to: fmt.edition });
    }
    let after_reg = fmt.regulation.info();
    if before_reg.as_ref() != Some(&after_reg) {
        changes.push(Change::Regulation { from: before_reg, to: after_reg, source: format.clone() });
    }
    if let Some(from) = before_title.filter(|v| *v != fmt.title_version) {
        changes.push(Change::TitleVersion { from, to: fmt.title_version });
    }
    if before_cap != fmt.capacity {
        changes.push(Change::EntrySize { entry: regulation::ENTRY, from: before_cap, to: fmt.capacity });
    }

    // Account: typed by the user, else the target folder's, else whatever the file names.
    let current = header::steam_id(out.block(HEADER_ENTRY));
    let folder = folder_account(path);
    let manual = target.account.filter(|a| !a.trim().is_empty()).map(parse_account).transpose()?;
    let (result, origin) = match (manual, folder) {
        (Some(id), _) => (Some(id), "manual"),
        (None, Some(id)) => (Some(id), "folder"),
        (None, None) => (current, if base.is_some() { "targetFile" } else { "source" }),
    };
    if let Some(id) = result.filter(|id| Some(*id) != current) {
        header::set_steam_id(out.block_mut(HEADER_ENTRY), id)?;
    }
    // Reported against the source's account, or against the target file's when only that one
    // is replaced.
    let source_account = header::steam_id(source.block(HEADER_ENTRY));
    if let Some(id) = result {
        let replaced = [source_account, current].into_iter().find(|a| *a != Some(id));
        if let Some(from) = replaced {
            changes.push(Change::Account { from: from.map(|a| a.to_string()), to: id.to_string(), origin });
        }
    }
    let accounts = AccountPlan {
        source: source_account.map(|a| a.to_string()),
        target_file: base.and_then(|b| header::steam_id(b.block(HEADER_ENTRY))).map(|a| a.to_string()),
        target_folder: folder.map(|a| a.to_string()),
        result: result.map(|a| a.to_string()),
        origin,
        local: Vec::new(),
    };
    Ok(Built { out, changes, format, accounts })
}
