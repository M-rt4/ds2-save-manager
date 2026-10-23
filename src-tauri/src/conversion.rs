//! Carrying saves and characters to a release (and an account) on this machine. A written save
//! always ends up in the target game's own format: its key, its regulation copy and version
//! stamp, its entry layout and the account its folder belongs to. Every difference from what
//! came in is reported as a `Change`, before (plan) and after (run) writing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ds2save::save::backups::Reason;
use ds2save::save::character;
use ds2save::save::disk;
use ds2save::save::file::{character_entry, world_entry, SaveFile, HEADER_ENTRY, SLOT_COUNT};
use ds2save::save::regulation::{self, RegulationInfo};
use ds2save::save::transfer::{self, Compatibility, TargetFormat};
use ds2save::save::{header, Edition, SaveError};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{doc_id, read_save, write_checked, AppState, CmdResult, Doc, SlotView};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionRequest {
    source_id: String,
    /// `None` moves the whole save; `Some((from, to))` one character into slot `to`.
    slot: Option<(usize, usize)>,
    target_edition: Edition,
    target_path: String,
    /// A SteamID64 the user typed; otherwise the account the target folder belongs to.
    account: Option<String>,
    /// The target game's install folder, read when no save of that release can be trusted.
    game_dir: Option<String>,
}

/// Where the target format came from.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatSource {
    /// `install`, `targetFile` or `otherSave`.
    kind: &'static str,
    path: String,
    regulation: RegulationInfo,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAccount {
    id: String,
    folder: String,
    files: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPlan {
    source: Option<String>,
    target_file: Option<String>,
    target_folder: Option<String>,
    result: Option<String>,
    /// Where `result` came from: `manual`, `folder`, `targetFile` or `source`.
    origin: &'static str,
    local: Vec<LocalAccount>,
}

/// One difference between what goes in and what gets written.
#[derive(Clone, Serialize)]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionPlan {
    target_path: String,
    target_exists: bool,
    target_edition: Option<Edition>,
    target_slots: Option<Vec<SlotView>>,
    target_error: Option<SaveError>,
    compatibility: Option<Compatibility>,
    source_regulation: Option<RegulationInfo>,
    format: Option<FormatSource>,
    accounts: AccountPlan,
    changes: Vec<Change>,
    /// Why the plan can't run, when it can't.
    blocked: Option<SaveError>,
    target_open_and_dirty: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionReport {
    path: String,
    backup: Option<String>,
    edition: Edition,
    changes: Vec<Change>,
}

/// Installed-game formats keyed by folder; reading one decrypts 9 MB and scans a 30 MB exe.
static INSTALL_FORMATS: Mutex<Option<HashMap<(PathBuf, Edition), TargetFormat>>> = Mutex::new(None);

fn install_format(edition: Edition, dir: &Path) -> Result<TargetFormat, SaveError> {
    let key = (dir.to_path_buf(), edition);
    if let Some(f) = INSTALL_FORMATS.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
        return Ok(f.clone());
    }
    let fmt = TargetFormat::from_install(edition, dir)?;
    if !regulation_matches(edition, &fmt.regulation.info()) {
        return Err(SaveError::new("wrongGame", format!("{} is not a {edition:?} install.", dir.display())));
    }
    INSTALL_FORMATS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, fmt.clone());
    Ok(fmt)
}

/// Scholar regulations are versioned 02xxxxxx, the original release's 01xxxxxx.
fn regulation_matches(edition: Edition, info: &RegulationInfo) -> bool {
    let major = if edition == Edition::Scholar { "02" } else { "01" };
    info.version.starts_with(major)
}

fn folder_account(path: &Path) -> Option<u64> {
    path.parent().and_then(|p| p.file_name()).and_then(|f| disk::account_of_folder(&f.to_string_lossy()))
}

/// The account folders a release can write to on this machine (on Linux each release has
/// its own prefix, so the list depends on the release).
fn local_accounts(edition: Edition) -> Vec<LocalAccount> {
    let mut out: Vec<LocalAccount> = disk::saves_roots(Some(edition))
        .iter()
        .flat_map(|root| std::fs::read_dir(root).into_iter().flatten().flatten())
        .filter_map(|d| {
            let folder = d.file_name().to_string_lossy().into_owned();
            let id = disk::account_of_folder(&folder)?;
            let files = std::fs::read_dir(d.path())
                .into_iter()
                .flatten()
                .flatten()
                .map(|f| f.file_name().to_string_lossy().into_owned())
                .filter(|n| n.to_ascii_lowercase().ends_with(".sl2"))
                .collect();
            Some(LocalAccount { id: id.to_string(), folder: d.path().to_string_lossy().into_owned(), files })
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.files.len()));
    out
}

/// The newest save of `edition` under the saves root (other than `except`) whose regulation
/// belongs to that release.
fn other_save_format(edition: Edition, except: &Path) -> Option<(TargetFormat, PathBuf)> {
    disk::discover().into_iter().filter(|f| f.edition == Some(edition)).find_map(|f| {
        let p = PathBuf::from(&f.path);
        if doc_id(&p) == doc_id(except) {
            return None;
        }
        let (save, _) = read_save(&p).ok()?;
        let fmt = TargetFormat::from_save(&save).ok()?;
        (save.source_edition == edition && regulation_matches(edition, &fmt.regulation.info())).then_some((fmt, p))
    })
}

fn parse_account(text: &str) -> Result<u64, SaveError> {
    let id: u64 = text.trim().parse().map_err(|_| SaveError::new("badAccount", "A SteamID64 is a 17-digit number."))?;
    if id >> 32 != header::STEAM_ID_HIGH || id as u32 == 0 {
        return Err(SaveError::new("badAccount", "That is not an individual Steam account id."));
    }
    Ok(id)
}

struct Built {
    out: SaveFile,
    changes: Vec<Change>,
    format: FormatSource,
    accounts: AccountPlan,
}

/// Builds the save that would be written, without writing it.
fn build(source: &SaveFile, req: &ConversionRequest, base: Option<&SaveFile>) -> Result<Built, SaveError> {
    let path = PathBuf::from(&req.target_path);
    let mut changes = Vec::new();

    // The format the written file must have.
    let install = req.game_dir.as_deref().filter(|d| !d.is_empty()).map(|d| install_format(req.target_edition, Path::new(d)));
    let base_format = base
        .filter(|b| b.source_edition == req.target_edition)
        .and_then(|b| TargetFormat::from_save(b).ok())
        .filter(|f| regulation_matches(req.target_edition, &f.regulation.info()));
    let (fmt, format) = match (install, base_format) {
        (Some(Ok(f)), _) => {
            let info = f.regulation.info();
            (f, FormatSource { kind: "install", path: req.game_dir.clone().unwrap_or_default(), regulation: info })
        }
        (Some(Err(e)), _) => return Err(e),
        (None, Some(f)) => {
            let info = f.regulation.info();
            (f, FormatSource { kind: "targetFile", path: req.target_path.clone(), regulation: info })
        }
        (None, None) => match other_save_format(req.target_edition, &path) {
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
    let mut out = match (req.slot, base) {
        (None, Some(b)) => {
            let merged = transfer::merge(source, b)?;
            for s in 0..SLOT_COUNT {
                let before = character::summary(b, s);
                let after = character::summary(&merged, s);
                let (before, after) = (before.occupied.then_some(before.name), after.occupied.then_some(after.name));
                let same_data = b.block(character_entry(s)) == merged.block(character_entry(s))
                    && b.block(world_entry(s)) == merged.block(world_entry(s));
                if !same_data {
                    changes.push(Change::Slot { slot: s, before, after });
                }
            }
            changes.push(Change::KeptTargetSystem { path: req.target_path.clone() });
            merged
        }
        (None, None) => {
            changes.push(Change::NewFile { path: req.target_path.clone() });
            transfer::convert(source, req.target_edition)
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
    let reference = match (base, req.slot) {
        (Some(b), Some(_)) => b,
        _ => &out,
    };
    let before_reg = transfer::regulation_info(reference);
    let before_title = ds2save::save::chunks::find(reference.block(22), header::TITLE_CHUNK).map(|c| c.version);
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
    let folder = folder_account(&path);
    let manual = req.account.as_deref().filter(|a| !a.trim().is_empty()).map(parse_account).transpose()?;
    let (result, origin) = match (manual, folder) {
        (Some(id), _) => (Some(id), "manual"),
        (None, Some(id)) => (Some(id), "folder"),
        (None, None) => (current, if base.is_some() { "targetFile" } else { "source" }),
    };
    if let Some(id) = result.filter(|id| Some(*id) != current) {
        header::set_steam_id(out.block_mut(HEADER_ENTRY), id)?;
    }
    let source_account = header::steam_id(source.block(HEADER_ENTRY));
    if let Some(id) = result.filter(|id| Some(*id) != source_account) {
        changes.push(Change::Account { from: source_account.map(|a| a.to_string()), to: id.to_string(), origin });
    }
    let accounts = AccountPlan {
        source: source_account.map(|a| a.to_string()),
        target_file: base.and_then(|b| header::steam_id(b.block(HEADER_ENTRY))).map(|a| a.to_string()),
        target_folder: folder.map(|a| a.to_string()),
        result: result.map(|a| a.to_string()),
        origin,
        local: local_accounts(req.target_edition),
    };
    Ok(Built { out, changes, format, accounts })
}

/// A save written over itself for its own release is a repair (it gets the installed game's
/// format and its folder's account); written over itself as the other release it would
/// end up named for the wrong game.
fn same_file_other_release(req: &ConversionRequest, source: &SaveFile, path: &Path) -> bool {
    doc_id(path) == req.source_id && req.slot.is_none() && req.target_edition != source.source_edition
}

fn source_of(state: &AppState, id: &str) -> CmdResult<SaveFile> {
    state.with_doc(id, |doc| Ok(doc.save.clone()))
}

#[tauri::command]
pub async fn plan_conversion(state: State<'_, AppState>, req: ConversionRequest) -> CmdResult<ConversionPlan> {
    let source = source_of(&state, &req.source_id)?;
    let path = PathBuf::from(&req.target_path);
    let target_exists = path.exists();
    let mut target_error = None;
    let base = if target_exists {
        match read_save(&path) {
            Ok((t, _)) => Some(t),
            Err(e) => {
                target_error = Some(e);
                None
            }
        }
    } else {
        None
    };
    let target_open_and_dirty = state.is_dirty(&doc_id(&path));
    let mut plan = ConversionPlan {
        target_path: path.to_string_lossy().into_owned(),
        target_exists,
        target_edition: base.as_ref().map(|b| b.source_edition),
        target_slots: base.as_ref().map(|b| (0..SLOT_COUNT).map(|i| character::summary(b, i).into()).collect()),
        target_error,
        compatibility: base.as_ref().map(|b| transfer::compare(&source, b)),
        source_regulation: transfer::regulation_info(&source),
        format: None,
        accounts: AccountPlan {
            source: header::steam_id(source.block(HEADER_ENTRY)).map(|a| a.to_string()),
            target_file: None,
            target_folder: folder_account(&path).map(|a| a.to_string()),
            result: None,
            origin: "source",
            local: local_accounts(req.target_edition),
        },
        changes: Vec::new(),
        blocked: None,
        target_open_and_dirty,
    };
    if same_file_other_release(&req, &source, &path) {
        plan.blocked = Some(SaveError::new("sameFile", "The target is the source file itself."));
        return Ok(plan);
    }
    match build(&source, &req, base.as_ref()) {
        Ok(b) => {
            plan.format = Some(b.format);
            plan.accounts = b.accounts;
            plan.changes = b.changes;
        }
        Err(e) => plan.blocked = Some(e),
    }
    if target_open_and_dirty && plan.blocked.is_none() {
        plan.blocked = Some(SaveError::new("targetDirty", "The target is open here with unsaved changes; save or discard them first."));
    }
    Ok(plan)
}

#[tauri::command]
pub async fn run_conversion(state: State<'_, AppState>, req: ConversionRequest) -> CmdResult<ConversionReport> {
    crate::commands::ensure_game_closed()?;
    let source = source_of(&state, &req.source_id)?;
    let path = PathBuf::from(&req.target_path);
    if same_file_other_release(&req, &source, &path) {
        return Err(SaveError::new("sameFile", "The target is the source file itself."));
    }
    if state.is_dirty(&doc_id(&path)) {
        return Err(SaveError::new("targetDirty", "The target is open here with unsaved changes; save or discard them first."));
    }
    let base = if path.exists() { Some(read_save(&path)?.0) } else { None };
    let Built { out, mut changes, .. } = build(&source, &req, base.as_ref())?;
    let (backup, fingerprint) = write_checked(&path, &out, Reason::Transfer)?;
    if let Some(b) = &backup {
        changes.push(Change::Backup { path: b.to_string_lossy().into_owned() });
    }
    // A target that is open in the app now shows what was written.
    if let Ok((save, _)) = read_save(&path) {
        state.replace_if_open(&doc_id(&path), Doc::new(path.clone(), save, fingerprint));
    }
    Ok(ConversionReport {
        path: path.to_string_lossy().into_owned(),
        backup: backup.map(|b| b.to_string_lossy().into_owned()),
        edition: out.edition,
        changes,
    })
}

/// Where a save of `edition` should go: the same account's folder among the release's own
/// saves folders (on Windows that is the source's folder itself; on Linux the release's
/// Proton prefix), else the local account folder that holds the most saves.
#[tauri::command]
pub async fn default_target(path: String, edition: Edition) -> String {
    let p = PathBuf::from(&path);
    let local = local_accounts(edition);
    if let Some(id) = folder_account(&p) {
        if let Some(a) = local.iter().find(|a| a.id == id.to_string()) {
            return Path::new(&a.folder).join(edition.file_name()).to_string_lossy().into_owned();
        }
        if local.is_empty() {
            return p.with_file_name(edition.file_name()).to_string_lossy().into_owned();
        }
    }
    match local.first() {
        Some(a) => Path::new(&a.folder).join(edition.file_name()).to_string_lossy().into_owned(),
        None => p.with_file_name(edition.file_name()).to_string_lossy().into_owned(),
    }
}
