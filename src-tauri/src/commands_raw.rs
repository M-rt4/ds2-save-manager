//! The commands behind the Raw data tab: an open save's decrypted blocks, read a page at
//! a time and changed byte by byte in memory. Nothing reaches the disk until `write_save`
//! (backup first, never while the game runs), which also recomputes the checksums.

use ds2save::save::raw::{self, Match};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::State;

use crate::commands::{AppState, CmdResult};

/// The most one read returns; the view asks only for the pages it shows.
const MAX_READ: usize = 64 * 1024;
/// The most matches one search returns.
const MAX_MATCHES: usize = 1000;

/// Up to 64 KB of a decrypted block, sent as raw bytes (an `ArrayBuffer` in the UI).
#[tauri::command]
pub async fn raw_read(state: State<'_, AppState>, id: String, index: usize, offset: usize, length: usize) -> CmdResult<Response> {
    state.with_doc(&id, |doc| Ok(Response::new(raw::read(&doc.save, index, offset, length.min(MAX_READ))?.to_vec())))
}

/// Overwrites bytes of a decrypted block in memory; returns whether any changed.
#[tauri::command]
pub async fn raw_write(state: State<'_, AppState>, id: String, index: usize, offset: usize, bytes: Vec<u8>) -> CmdResult<bool> {
    state.edit_save(&id, |save| raw::write(save, index, offset, &bytes))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMatches {
    matches: Vec<Match>,
    /// Whether the search stopped at `MAX_MATCHES`.
    more: bool,
}

/// Where a byte pattern occurs, in one block or (`index` = null) in all of them.
#[tauri::command]
pub async fn raw_find(state: State<'_, AppState>, id: String, index: Option<usize>, pattern: Vec<u8>) -> CmdResult<RawMatches> {
    state.with_doc(&id, |doc| {
        let (matches, more) = raw::find(&doc.save, index, &pattern, MAX_MATCHES)?;
        Ok(RawMatches { matches, more })
    })
}
