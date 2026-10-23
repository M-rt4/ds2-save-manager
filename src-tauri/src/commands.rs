//! The commands the UI calls. Open saves live here, decrypted, keyed by their path; edits
//! change them in memory and nothing touches the disk until `write_save` or
//! `run_conversion`, which back up first and refuse while the game runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ds2save::save::character::{self, Character, CharacterEdit, SlotSummary};
use ds2save::save::disk::{self, FoundSave};
use ds2save::save::file::{SaveFile, HEADER_ENTRY, SLOT_COUNT};
use ds2save::save::items::{catalog, ItemDef};
use ds2save::save::transfer::{self, Compatibility};
use ds2save::save::{chunks, header, maps, Edition, SaveError};
use serde::{Deserialize, Serialize};
use tauri::State;

type CmdResult<T> = Result<T, SaveError>;

struct Doc {
    path: PathBuf,
    save: SaveFile,
    fingerprint: [u8; 16],
    dirty: bool,
}

#[derive(Default)]
pub struct AppState {
    docs: Mutex<HashMap<String, Doc>>,
}

impl AppState {
    fn with_doc<T>(&self, id: &str, f: impl FnOnce(&mut Doc) -> CmdResult<T>) -> CmdResult<T> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(id).ok_or_else(|| SaveError::new("notOpen", "That save is not open."))?;
        f(doc)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    #[serde(flatten)]
    summary: SlotSummary,
    area: Option<&'static str>,
    map_label: String,
}

impl From<SlotSummary> for SlotView {
    fn from(summary: SlotSummary) -> Self {
        Self { area: maps::area(summary.map), map_label: maps::label(summary.map), summary }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInfo {
    index: usize,
    name: String,
    size: usize,
    chunks: Vec<chunks::Chunk>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveView {
    id: String,
    path: String,
    file_name: String,
    folder: String,
    edition: Edition,
    source_edition: Edition,
    account: Option<String>,
    folder_account: Option<String>,
    last_slot: Option<u32>,
    slots: Vec<SlotView>,
    checksum_failures: Vec<usize>,
    dirty: bool,
    size: usize,
    entries: Vec<EntryInfo>,
}

fn view(id: &str, doc: &Doc) -> SaveView {
    let save = &doc.save;
    let folder = doc.path.parent().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    SaveView {
        id: id.to_string(),
        path: doc.path.to_string_lossy().into_owned(),
        file_name: doc.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
        folder_account: disk::account_of_folder(&folder).map(|a| a.to_string()),
        folder,
        edition: save.edition,
        source_edition: save.source_edition,
        account: header::steam_id(save.block(HEADER_ENTRY)).map(|a| a.to_string()),
        last_slot: header::last_slot(save.block(HEADER_ENTRY)),
        slots: (0..SLOT_COUNT).map(|i| character::summary(save, i).into()).collect(),
        checksum_failures: save.checksum_failures(),
        dirty: doc.dirty,
        size: save.raw().len(),
        entries: save
            .entries()
            .iter()
            .map(|e| EntryInfo {
                index: e.index,
                name: e.name.clone(),
                size: save.block(e.index).len(),
                chunks: chunks::walk(save.block(e.index)).unwrap_or_default(),
            })
            .collect(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterView {
    #[serde(flatten)]
    character: Character,
    area: Option<&'static str>,
    map_label: String,
}

fn character_view(save: &SaveFile, slot: usize) -> CmdResult<CharacterView> {
    if slot >= SLOT_COUNT {
        return Err(SaveError::invalid("Slots are 1 to 10."));
    }
    let character = character::read(save, slot);
    Ok(CharacterView { area: maps::area(character.map), map_label: maps::label(character.map), character })
}

fn read_save(path: &Path) -> CmdResult<(SaveFile, [u8; 16])> {
    let bytes = std::fs::read(path)?;
    let fingerprint = disk::fingerprint(&bytes);
    Ok((SaveFile::parse(bytes)?, fingerprint))
}

fn doc_id(path: &Path) -> String {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn list_saves() -> Vec<FoundSave> {
    disk::discover()
}

#[tauri::command]
pub async fn saves_root() -> Option<String> {
    disk::saves_root().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn game_running() -> bool {
    disk::game_running()
}

#[tauri::command]
pub async fn open_save(state: State<'_, AppState>, path: String) -> CmdResult<SaveView> {
    let path = PathBuf::from(path);
    let (save, fingerprint) = read_save(&path)?;
    let id = doc_id(&path);
    let doc = Doc { path, save, fingerprint, dirty: false };
    let v = view(&id, &doc);
    state.docs.lock().unwrap().insert(id, doc);
    Ok(v)
}

#[tauri::command]
pub async fn get_save(state: State<'_, AppState>, id: String) -> CmdResult<SaveView> {
    state.with_doc(&id, |doc| Ok(view(&id, doc)))
}

#[tauri::command]
pub async fn close_save(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.docs.lock().unwrap().remove(&id);
    Ok(())
}

/// Throws away unsaved edits and reads the file again.
#[tauri::command]
pub async fn reload_save(state: State<'_, AppState>, id: String) -> CmdResult<SaveView> {
    state.with_doc(&id, |doc| {
        let (save, fingerprint) = read_save(&doc.path)?;
        *doc = Doc { path: doc.path.clone(), save, fingerprint, dirty: false };
        Ok(view(&id, doc))
    })
}

#[tauri::command]
pub async fn get_character(state: State<'_, AppState>, id: String, slot: usize) -> CmdResult<CharacterView> {
    state.with_doc(&id, |doc| character_view(&doc.save, slot))
}

#[tauri::command]
pub async fn edit_character(
    state: State<'_, AppState>,
    id: String,
    slot: usize,
    edit: CharacterEdit,
) -> CmdResult<CharacterView> {
    state.with_doc(&id, |doc| {
        character::apply(&mut doc.save, slot, &edit)?;
        doc.dirty = true;
        character_view(&doc.save, slot)
    })
}

#[tauri::command]
pub async fn item_catalog() -> Vec<ItemDef> {
    catalog().items.clone()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteReport {
    path: String,
    backup: Option<String>,
    edition: Edition,
}

fn ensure_game_closed() -> CmdResult<()> {
    if disk::game_running() {
        return Err(SaveError::new("gameRunning", "Dark Souls II is running. Quit to the desktop first."));
    }
    Ok(())
}

/// Backs up whatever is at `path`, writes `save` there, and reads it back to prove it.
fn write_checked(path: &Path, save: &SaveFile) -> CmdResult<(Option<PathBuf>, [u8; 16])> {
    let bytes = save.encode();
    let reread = SaveFile::parse(bytes.clone())?;
    if reread.source_edition != save.edition || reread.checksum_failures().len() > save.checksum_failures().len() {
        return Err(SaveError::new("verify", "The encoded save did not read back correctly; nothing was written."));
    }
    let backup = disk::backup(path)?;
    disk::write_atomic(path, &bytes)?;
    Ok((backup, disk::fingerprint(&bytes)))
}

#[tauri::command]
pub async fn write_save(state: State<'_, AppState>, id: String) -> CmdResult<WriteReport> {
    ensure_game_closed()?;
    state.with_doc(&id, |doc| {
        if doc.path.exists() {
            let current = disk::fingerprint(&std::fs::read(&doc.path)?);
            if current != doc.fingerprint {
                return Err(SaveError::new(
                    "changedOnDisk",
                    "The file changed on disk since it was opened (the game saved?). Reload it first.",
                ));
            }
        }
        let (backup, fingerprint) = write_checked(&doc.path, &doc.save)?;
        doc.fingerprint = fingerprint;
        doc.dirty = false;
        Ok(WriteReport {
            path: doc.path.to_string_lossy().into_owned(),
            backup: backup.map(|b| b.to_string_lossy().into_owned()),
            edition: doc.save.edition,
        })
    })
}

/// Points the save at another Steam account (decimal SteamID64).
#[tauri::command]
pub async fn set_account(state: State<'_, AppState>, id: String, account: String) -> CmdResult<SaveView> {
    let account: u64 = account.trim().parse().map_err(|_| SaveError::invalid("A SteamID64 is a 17-digit number."))?;
    state.with_doc(&id, |doc| {
        header::set_steam_id(doc.save.block_mut(HEADER_ENTRY), account)?;
        doc.dirty = true;
        Ok(view(&id, doc))
    })
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConversionMode {
    /// Write the whole source, re-encrypted, as the target file.
    Replace,
    /// Keep the existing target file and replace its ten slots with the source's.
    Merge,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionRequest {
    source_id: String,
    target_edition: Edition,
    target_path: String,
    mode: ConversionMode,
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
    /// The account the written file will name, when it differs from the source's.
    account_change: Option<(String, String)>,
    target_open_and_dirty: bool,
}

fn target_account(path: &Path) -> Option<u64> {
    path.parent().and_then(|p| p.file_name()).and_then(|f| disk::account_of_folder(&f.to_string_lossy()))
}

#[tauri::command]
pub async fn plan_conversion(state: State<'_, AppState>, req: ConversionRequest) -> CmdResult<ConversionPlan> {
    let source = state.with_doc(&req.source_id, |doc| Ok(doc.save.clone()))?;
    let path = PathBuf::from(&req.target_path);
    let target_exists = path.exists();
    let (mut target_edition, mut target_slots, mut target_error, mut compatibility) = (None, None, None, None);
    if target_exists {
        match read_save(&path) {
            Ok((t, _)) => {
                target_edition = Some(t.source_edition);
                target_slots = Some((0..SLOT_COUNT).map(|i| character::summary(&t, i).into()).collect());
                compatibility = Some(transfer::compare(&source, &t));
            }
            Err(e) => target_error = Some(e),
        }
    }
    let from = header::steam_id(source.block(HEADER_ENTRY));
    let to = match req.mode {
        ConversionMode::Merge => None,
        ConversionMode::Replace => target_account(&path),
    };
    let account_change = match (from, to) {
        (Some(a), Some(b)) if a != b => Some((a.to_string(), b.to_string())),
        _ => None,
    };
    let target_open_and_dirty = state.docs.lock().unwrap().get(&doc_id(&path)).is_some_and(|d| d.dirty);
    Ok(ConversionPlan {
        target_path: path.to_string_lossy().into_owned(),
        target_exists,
        target_edition,
        target_slots,
        target_error,
        compatibility,
        account_change,
        target_open_and_dirty,
    })
}

#[tauri::command]
pub async fn run_conversion(state: State<'_, AppState>, req: ConversionRequest) -> CmdResult<WriteReport> {
    ensure_game_closed()?;
    let source = state.with_doc(&req.source_id, |doc| Ok(doc.save.clone()))?;
    let path = PathBuf::from(&req.target_path);
    if doc_id(&path) == req.source_id && req.target_edition == source.source_edition {
        return Err(SaveError::invalid("The target is the source file itself."));
    }
    let out = match req.mode {
        ConversionMode::Merge => {
            let (base, _) = read_save(&path)?;
            transfer::merge(&source, &base)?
        }
        ConversionMode::Replace => {
            let mut out = transfer::convert(&source, req.target_edition);
            if let Some(account) = target_account(&path) {
                if header::steam_id(out.block(HEADER_ENTRY)).is_some_and(|a| a != account) {
                    header::set_steam_id(out.block_mut(HEADER_ENTRY), account)?;
                }
            }
            out
        }
    };
    let (backup, fingerprint) = write_checked(&path, &out)?;

    // A target that is open in the app now shows what was written.
    let id = doc_id(&path);
    if let Some(doc) = state.docs.lock().unwrap().get_mut(&id) {
        if let Ok((save, _)) = read_save(&path) {
            *doc = Doc { path: path.clone(), save, fingerprint, dirty: false };
        }
    }
    Ok(WriteReport {
        path: path.to_string_lossy().into_owned(),
        backup: backup.map(|b| b.to_string_lossy().into_owned()),
        edition: out.edition,
    })
}

#[tauri::command]
pub async fn compare_saves(state: State<'_, AppState>, source_id: String, target_id: String) -> CmdResult<Compatibility> {
    let docs = state.docs.lock().unwrap();
    let get = |id: &str| docs.get(id).ok_or_else(|| SaveError::new("notOpen", "That save is not open."));
    Ok(transfer::compare(&get(&source_id)?.save, &get(&target_id)?.save))
}

/// Copies one character into a slot of another open save (or of the same save). The
/// target is only changed in memory; `write_save` stores it.
#[tauri::command]
pub async fn transfer_character(
    state: State<'_, AppState>,
    source_id: String,
    from_slot: usize,
    target_id: String,
    to_slot: usize,
) -> CmdResult<SaveView> {
    let mut docs = state.docs.lock().unwrap();
    let source = docs.get(&source_id).ok_or_else(|| SaveError::new("notOpen", "That save is not open."))?.save.clone();
    let target = docs.get_mut(&target_id).ok_or_else(|| SaveError::new("notOpen", "That save is not open."))?;
    if !character::summary(&source, from_slot.min(SLOT_COUNT - 1)).occupied {
        return Err(SaveError::invalid("The source slot has no character."));
    }
    transfer::transfer(&source, from_slot, &mut target.save, to_slot)?;
    target.dirty = true;
    Ok(view(&target_id, target))
}

/// The default target for converting a save: the other release's file in the same folder.
#[tauri::command]
pub async fn default_target(path: String, edition: Edition) -> String {
    let p = PathBuf::from(path);
    p.with_file_name(edition.file_name()).to_string_lossy().into_owned()
}
