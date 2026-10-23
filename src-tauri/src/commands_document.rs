//! The commands behind the readable view of the Raw data tab: an open save as a JSON
//! document, a preview of what an edited document would change, applying it in memory, and
//! reading and writing `.json` files. Writing the save itself stays with `write_save`.

use std::path::Path;

use ds2save::save::document::{self, FieldChange, Preview};
use ds2save::save::{disk, SaveError};
use tauri::State;

use crate::commands::{AppState, CmdResult};

/// Larger than any save's document by far.
const MAX_FILE: u64 = 32 * 1024 * 1024;

#[tauri::command]
pub async fn document_export(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    state.with_doc(&id, |doc| Ok(document::to_text(&document::export(&doc.save))))
}

#[tauri::command]
pub async fn document_preview(state: State<'_, AppState>, id: String, text: String) -> CmdResult<Preview> {
    state.with_doc(&id, |doc| Ok(document::preview(&doc.save, &text)))
}

/// Applies a document to the open save in memory; the save counts as edited when anything
/// was written.
#[tauri::command]
pub async fn document_apply(state: State<'_, AppState>, id: String, text: String) -> CmdResult<Vec<FieldChange>> {
    let mut changes = Vec::new();
    state.edit_save(&id, |save| {
        changes = document::apply(save, &text)?;
        Ok(changes.iter().any(|c| c.applies))
    })?;
    Ok(changes)
}

fn json_path(path: &str) -> CmdResult<&Path> {
    let p = Path::new(path);
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
        return Err(SaveError::new("notJson", "Documents are read from and written to .json files."));
    }
    Ok(p)
}

#[tauri::command]
pub async fn document_write_file(path: String, text: String) -> CmdResult<()> {
    let p = json_path(&path)?;
    disk::ensure_writable(p)?;
    std::fs::write(p, text)?;
    Ok(())
}

#[tauri::command]
pub async fn document_read_file(path: String) -> CmdResult<String> {
    let p = json_path(&path)?;
    if std::fs::metadata(p)?.len() > MAX_FILE {
        return Err(SaveError::new("notJson", "That file is far larger than any save's document."));
    }
    std::fs::read_to_string(p).map_err(|e| SaveError::new("notJson", format!("The file is not text: {e}")))
}
