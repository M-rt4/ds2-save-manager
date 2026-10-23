//! The commands the UI calls. Open saves live here, decrypted, keyed by their path; edits
//! change them in memory and nothing touches the disk until `write_save` or
//! `run_conversion`, which back up first and refuse while the game runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ds2save::save::backups::Reason;
use ds2save::save::character::{self, Character, CharacterEdit, SlotSummary};
use ds2save::save::disk::{self, FoundSave};
use ds2save::save::file::{character_entry, SaveFile, HEADER_ENTRY, SLOT_COUNT};
use ds2save::save::items::{catalog, ItemDef};
use ds2save::save::{chunks, header, maps, Edition, SaveError};
use serde::Serialize;
use tauri::State;

pub(crate) type CmdResult<T> = Result<T, SaveError>;

pub(crate) struct Doc {
    path: PathBuf,
    pub(crate) save: SaveFile,
    fingerprint: [u8; 16],
    dirty: bool,
    /// The ten character blocks as the file on disk holds them (read, reloaded or last
    /// written); what the inventory's change marks compare against.
    saved_characters: Vec<Vec<u8>>,
}

#[derive(Default)]
pub struct AppState {
    docs: Mutex<HashMap<String, Doc>>,
}

impl Doc {
    pub(crate) fn new(path: PathBuf, save: SaveFile, fingerprint: [u8; 16]) -> Self {
        let saved_characters = characters_of(&save);
        Self { path, save, fingerprint, dirty: false, saved_characters }
    }

    /// A character block as the file on disk holds it.
    pub(crate) fn saved_character(&self, slot: usize) -> Option<&[u8]> {
        self.saved_characters.get(slot).map(Vec::as_slice)
    }
}

fn characters_of(save: &SaveFile) -> Vec<Vec<u8>> {
    (0..SLOT_COUNT).map(|s| save.block(character_entry(s)).to_vec()).collect()
}

impl AppState {
    pub(crate) fn is_dirty(&self, id: &str) -> bool {
        self.docs.lock().unwrap().get(id).is_some_and(|d| d.dirty)
    }

    /// Swaps in a freshly read copy of a save that is open in the app.
    pub(crate) fn replace_if_open(&self, id: &str, doc: Doc) {
        if let Some(open) = self.docs.lock().unwrap().get_mut(id) {
            *open = doc;
        }
    }

    pub(crate) fn with_doc<T>(&self, id: &str, f: impl FnOnce(&mut Doc) -> CmdResult<T>) -> CmdResult<T> {
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
    /// Set when this build may not write the file back: a development build and a save inside
    /// the game's real save folders. Edits in memory still work; saving does not.
    read_only: bool,
    read_only_reason: Option<SaveError>,
    size: usize,
    entries: Vec<EntryInfo>,
}

fn view(id: &str, doc: &Doc) -> SaveView {
    let save = &doc.save;
    let read_only_reason = disk::ensure_writable(&doc.path).err();
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
        read_only: read_only_reason.is_some(),
        read_only_reason,
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

pub(crate) fn read_save(path: &Path) -> CmdResult<(SaveFile, [u8; 16])> {
    let bytes = std::fs::read(path)?;
    let fingerprint = disk::fingerprint(&bytes);
    Ok((SaveFile::parse(bytes)?, fingerprint))
}

pub(crate) fn doc_id(path: &Path) -> String {
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
pub async fn game_running() -> CmdResult<bool> {
    disk::game_running()
}

#[tauri::command]
pub async fn open_save(state: State<'_, AppState>, path: String) -> CmdResult<SaveView> {
    let path = PathBuf::from(path);
    let id = doc_id(&path);
    // Opened again while it holds unsaved edits: reading the file over them would lose them
    // without a word. Throwing them away is reload_save's job, asked for in so many words.
    if let Some(doc) = state.docs.lock().unwrap().get(&id).filter(|d| d.dirty) {
        return Ok(view(&id, doc));
    }
    let (save, fingerprint) = read_save(&path)?;
    let doc = Doc::new(path, save, fingerprint);
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
        *doc = Doc::new(doc.path.clone(), save, fingerprint);
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

/// Refuses while the game runs, and when it can't be told whether it does.
pub(crate) fn ensure_game_closed() -> CmdResult<()> {
    if disk::game_running()? {
        return Err(SaveError::new("gameRunning", "Dark Souls II is running. Quit to the desktop first."));
    }
    Ok(())
}

/// Backs up whatever is at `path`, writes `save` there, and reads it back to prove it.
pub(crate) fn write_checked(path: &Path, save: &SaveFile, reason: Reason) -> CmdResult<(Option<PathBuf>, [u8; 16])> {
    let bytes = save.encode();
    save.verify_encoded(&bytes)?;
    let backup = disk::backup(path, reason)?;
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
        let (backup, fingerprint) = write_checked(&doc.path, &doc.save, Reason::Save)?;
        doc.fingerprint = fingerprint;
        doc.dirty = false;
        doc.saved_characters = characters_of(&doc.save);
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

impl AppState {
    /// Reads `path` again into the open save it belongs to, if one is open, dropping its
    /// unsaved edits. Used after something else rewrote the file (restoring a backup).
    pub fn reload_if_open(&self, path: &Path) -> CmdResult<()> {
        let mut docs = self.docs.lock().unwrap();
        if let Some(doc) = docs.get_mut(&doc_id(path)) {
            let (save, fingerprint) = read_save(path)?;
            *doc = Doc::new(doc.path.clone(), save, fingerprint);
        }
        Ok(())
    }

    /// Runs `f` on the decrypted file of an open save; when it reports a change, the save
    /// counts as edited until it is written.
    pub(crate) fn edit_save(&self, id: &str, f: impl FnOnce(&mut SaveFile) -> CmdResult<bool>) -> CmdResult<bool> {
        self.with_doc(id, |doc| {
            let changed = f(&mut doc.save)?;
            doc.dirty |= changed;
            Ok(changed)
        })
    }

    /// Whether an open save's file now differs from what was read (the game saved it since).
    /// A file that can't be read any more counts as unchanged; the copy in memory is all there is.
    pub(crate) fn changed_on_disk(&self, id: &str) -> bool {
        let Some((path, fingerprint)) = self.docs.lock().unwrap().get(id).map(|d| (d.path.clone(), d.fingerprint)) else {
            return false;
        };
        std::fs::read(path).is_ok_and(|bytes| disk::fingerprint(&bytes) != fingerprint)
    }
}
