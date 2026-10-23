//! Carrying saves and characters to a release (and an account) on this machine. The save that
//! gets written is built by `save::conversion`; this side finds what the target game expects
//! (its install, the target file, another local save), plans, and writes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use ds2save::save::backups::Reason;
use ds2save::save::character;
use ds2save::save::conversion::{self, AccountPlan, Built, Change, FormatSource, Formats, LocalAccount, Target};
use ds2save::save::disk;
use ds2save::save::file::{SaveFile, HEADER_ENTRY, SLOT_COUNT};
use ds2save::save::regulation::RegulationInfo;
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

/// The files of an install a format is read from, with their modification times: a game
/// updated while the app runs is read again.
type InstallKey = (PathBuf, Edition, Option<SystemTime>, Option<SystemTime>);

/// Installed-game formats; reading one decrypts 9 MB and scans a 30 MB exe.
static INSTALL_FORMATS: Mutex<Option<HashMap<InstallKey, TargetFormat>>> = Mutex::new(None);

fn install_format(edition: Edition, dir: &Path) -> Result<TargetFormat, SaveError> {
    let modified = |name: &str| std::fs::metadata(dir.join(name)).and_then(|m| m.modified()).ok();
    let key = (dir.to_path_buf(), edition, modified("DarkSoulsII.exe"), modified("enc_regulation.bnd.dcx"));
    if let Some(f) = INSTALL_FORMATS.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
        return Ok(f.clone());
    }
    let fmt = TargetFormat::from_install(edition, dir)?;
    if !conversion::trusted(edition, &fmt) {
        return Err(SaveError::new("wrongGame", format!("{} is not a {edition:?} install.", dir.display())));
    }
    INSTALL_FORMATS.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, fmt.clone());
    Ok(fmt)
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

/// The newest save of `edition` under the saves root (other than `except`) that carries that
/// release's format.
fn other_save_format(edition: Edition, except: &Path) -> Option<(TargetFormat, PathBuf)> {
    disk::discover().into_iter().filter(|f| f.edition == Some(edition)).find_map(|f| {
        let p = PathBuf::from(&f.path);
        if doc_id(&p) == doc_id(except) {
            return None;
        }
        let (save, _) = read_save(&p).ok()?;
        let fmt = TargetFormat::from_save(&save).ok()?;
        (save.source_edition == edition && conversion::trusted(edition, &fmt)).then_some((fmt, p))
    })
}

fn source_of(state: &AppState, id: &str) -> CmdResult<SaveFile> {
    state.with_doc(id, |doc| Ok(doc.save.clone()))
}

/// The game saved the source after it was opened here: what would be carried over is old.
fn source_changed() -> SaveError {
    SaveError::new(
        "sourceChanged",
        "The source file changed on disk after it was opened (the game saved?). Open it again first.",
    )
}

fn target_dirty() -> SaveError {
    SaveError::new("targetDirty", "The target is open here with unsaved changes; save or discard them first.")
}

fn target<'a>(req: &'a ConversionRequest, path: &'a Path) -> Target<'a> {
    Target {
        slot: req.slot,
        edition: req.target_edition,
        path,
        account: req.account.as_deref(),
        same_file: doc_id(path) == req.source_id,
    }
}

fn build(source: &SaveFile, req: &ConversionRequest, path: &Path, base: Option<&SaveFile>) -> CmdResult<Built> {
    let install = req
        .game_dir
        .as_deref()
        .filter(|d| !d.is_empty())
        .map(|d| (install_format(req.target_edition, Path::new(d)), d.to_string()));
    let mut other_save = || other_save_format(req.target_edition, path);
    let mut built = conversion::build(source, &target(req, path), base, Formats { install, other_save: &mut other_save })?;
    built.accounts.local = local_accounts(req.target_edition);
    Ok(built)
}

#[tauri::command]
pub async fn plan_conversion(state: State<'_, AppState>, req: ConversionRequest) -> CmdResult<ConversionPlan> {
    let source = source_of(&state, &req.source_id)?;
    let path = PathBuf::from(&req.target_path);
    let target_exists = path.exists();
    let (base, target_error) = match conversion::read_target(&path) {
        Ok(base) => (base, None),
        Err(e) => (None, Some(e)),
    };
    let target_open_and_dirty = state.is_dirty(&doc_id(&path));
    let mut plan = ConversionPlan {
        target_path: path.to_string_lossy().into_owned(),
        target_exists,
        target_edition: base.as_ref().map(|b| b.source_edition),
        target_slots: base.as_ref().map(|b| (0..SLOT_COUNT).map(|i| character::summary(b, i).into()).collect()),
        target_error: target_error.clone(),
        compatibility: base.as_ref().map(|b| transfer::compare(&source, b)),
        source_regulation: transfer::regulation_info(&source),
        format: None,
        accounts: AccountPlan {
            source: header::steam_id(source.block(HEADER_ENTRY)).map(|a| a.to_string()),
            target_file: None,
            target_folder: conversion::folder_account(&path).map(|a| a.to_string()),
            result: None,
            origin: "source",
            local: local_accounts(req.target_edition),
        },
        changes: Vec::new(),
        blocked: None,
        target_open_and_dirty,
    };
    plan.blocked = if state.changed_on_disk(&req.source_id) {
        Some(source_changed())
    } else if let Err(e) = conversion::check_same_file(&source, &target(&req, &path)) {
        Some(e)
    } else if let Some(e) = target_error {
        Some(e)
    } else {
        match build(&source, &req, &path, base.as_ref()) {
            Ok(b) => {
                plan.format = Some(b.format);
                plan.accounts = b.accounts;
                plan.changes = b.changes;
                target_open_and_dirty.then(target_dirty)
            }
            Err(e) => Some(e),
        }
    };
    Ok(plan)
}

#[tauri::command]
pub async fn run_conversion(state: State<'_, AppState>, req: ConversionRequest) -> CmdResult<ConversionReport> {
    crate::commands::ensure_game_closed()?;
    let source = source_of(&state, &req.source_id)?;
    let path = PathBuf::from(&req.target_path);
    if state.changed_on_disk(&req.source_id) {
        return Err(source_changed());
    }
    conversion::check_same_file(&source, &target(&req, &path))?;
    if state.is_dirty(&doc_id(&path)) {
        return Err(target_dirty());
    }
    let base = conversion::read_target(&path)?;
    let Built { out, mut changes, .. } = build(&source, &req, &path, base.as_ref())?;
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
    if let Some(id) = conversion::folder_account(&p) {
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
