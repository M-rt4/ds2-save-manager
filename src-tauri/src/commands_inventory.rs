//! The inventory's change marks: what each record holds now against what the file on disk
//! holds, and putting one record back as it is on disk.

use ds2save::save::file::{character_entry, SLOT_COUNT};
use ds2save::save::inventory::RECORD_LEN;
use ds2save::save::inventory_changes::{self, InventoryChanges};
use ds2save::save::{raw, SaveError};
use tauri::State;

use crate::commands::{AppState, CmdResult};

fn check_slot(slot: usize) -> CmdResult<()> {
    if slot >= SLOT_COUNT {
        return Err(SaveError::invalid("Slots are 1 to 10."));
    }
    Ok(())
}

#[tauri::command]
pub async fn inventory_changes(state: State<'_, AppState>, id: String, slot: usize) -> CmdResult<InventoryChanges> {
    check_slot(slot)?;
    state.with_doc(&id, |doc| {
        let saved = doc.saved_character(slot).unwrap_or_default();
        Ok(inventory_changes::compare(saved, doc.save.block(character_entry(slot))))
    })
}

/// Puts one inventory record back as the file on disk holds it, whatever changed it.
#[tauri::command]
pub async fn inventory_revert(state: State<'_, AppState>, id: String, slot: usize, offset: usize) -> CmdResult<bool> {
    check_slot(slot)?;
    let saved = state.with_doc(&id, |doc| {
        let block = doc.saved_character(slot).unwrap_or_default();
        if !inventory_changes::record_offsets(block.len()).any(|o| o == offset) {
            return Err(SaveError::invalid("No inventory record at that position."));
        }
        Ok(block[offset..offset + RECORD_LEN].to_vec())
    })?;
    state.edit_save(&id, |save| raw::write(save, character_entry(slot), offset, &saved))
}
