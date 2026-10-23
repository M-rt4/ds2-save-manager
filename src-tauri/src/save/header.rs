//! Entry 0 is the menu block. Its chunk 3 holds the owning Steam account; its chunk 4 is
//! the Load Game title list: a 16-byte head (last used slot, slot count, flags) and ten
//! 496-byte records, one per slot, with what the menu shows without loading the
//! character. Entry 22 carries a second chunk 4 with the same layout.

use std::ops::Range;

use super::bytes::{put_u16, u16_at, u32_at, utf16_at};
use super::chunks;
use super::error::{Result, SaveError};
use super::file::SLOT_COUNT;

pub const ACCOUNT_CHUNK: u32 = 3;
pub const TITLE_CHUNK: u32 = 4;
pub const TITLE_HEAD_LEN: usize = 16;
pub const RECORD_LEN: usize = 496;

/// The account is the SteamID64 as 16 lowercase hex digits (the save folder's name).
pub const STEAM_ID_OFFSET: usize = 53;
pub const STEAM_ID_LEN: usize = 16;
/// Universe 1, type 1, instance 1: the high half of every individual SteamID64.
pub const STEAM_ID_HIGH: u64 = 0x0110_0001;

/// Field offsets inside one title record.
pub mod rec {
    /// Nine u16 attributes, in the character block's order (see `character::ATTR_OFFSETS`).
    pub const ATTRS: usize = 372;
    pub const NAME: usize = 394;
    pub const NAME_CHARS: usize = 16;
    /// Seconds played, u32.
    pub const PLAY_TIME: usize = 460;
    /// Map of the last rest, u32 `AABBCCDD` read as `mAA_BB_CC_DD`.
    pub const MAP: usize = 464;
    pub const LEVEL: usize = 468;
    pub const CLASS: usize = 470;
}

/// Byte range of the whole title chunk data (head + ten records) in a block.
pub fn title_data(block: &[u8]) -> Option<Range<usize>> {
    let chunk = chunks::find(block, TITLE_CHUNK)?;
    (chunk.size >= TITLE_HEAD_LEN + SLOT_COUNT * RECORD_LEN).then(|| chunk.data())
}

pub fn record(block: &[u8], slot: usize) -> Option<Range<usize>> {
    let data = title_data(block)?;
    let start = data.start + TITLE_HEAD_LEN + slot * RECORD_LEN;
    (slot < SLOT_COUNT).then_some(start..start + RECORD_LEN)
}

/// The 0-based slot the game will offer to continue.
pub fn last_slot(block: &[u8]) -> Option<u32> {
    title_data(block).map(|d| u32_at(block, d.start))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleRecord {
    pub name: String,
    pub level: u16,
    pub class: u16,
    pub play_time: u32,
    pub map: u32,
}

pub fn read_record(block: &[u8], slot: usize) -> Option<TitleRecord> {
    let r = record(block, slot)?;
    let b = &block[r];
    Some(TitleRecord {
        name: utf16_at(b, rec::NAME, rec::NAME_CHARS),
        level: u16_at(b, rec::LEVEL),
        class: u16_at(b, rec::CLASS),
        play_time: u32_at(b, rec::PLAY_TIME),
        map: u32_at(b, rec::MAP),
    })
}

pub fn set_record_name(block: &mut [u8], slot: usize, name: &str) -> Result<()> {
    let r = record(block, slot).ok_or_else(|| SaveError::format("Title list not found."))?;
    super::bytes::put_utf16(&mut block[r], rec::NAME, rec::NAME_CHARS + 1, name);
    Ok(())
}

pub fn set_record_level(block: &mut [u8], slot: usize, level: u16) -> Result<()> {
    let r = record(block, slot).ok_or_else(|| SaveError::format("Title list not found."))?;
    put_u16(&mut block[r], rec::LEVEL, level);
    Ok(())
}

/// Shifts the menu's attribute figures by the same amount the character's changed, so
/// any equipment bonus the menu includes is kept.
pub fn shift_record_attrs(block: &mut [u8], slot: usize, deltas: &[i32; 9]) -> Result<()> {
    let r = record(block, slot).ok_or_else(|| SaveError::format("Title list not found."))?;
    let b = &mut block[r];
    for (i, d) in deltas.iter().enumerate() {
        let o = rec::ATTRS + i * 2;
        let v = (u16_at(b, o) as i32 + d).clamp(1, 99 + 40) as u16;
        put_u16(b, o, v);
    }
    Ok(())
}

/// The owning account as a SteamID64, read from the hex text the game writes.
pub fn steam_id(header: &[u8]) -> Option<u64> {
    let chunk = chunks::find(header, ACCOUNT_CHUNK)?;
    let range = STEAM_ID_OFFSET..STEAM_ID_OFFSET + STEAM_ID_LEN;
    if !(chunk.data().start <= range.start && range.end <= chunk.data().end) {
        return None;
    }
    let text = std::str::from_utf8(&header[range]).ok()?;
    let id = u64::from_str_radix(text, 16).ok()?;
    (id >> 32 == STEAM_ID_HIGH && id as u32 != 0).then_some(id)
}

pub fn set_steam_id(header: &mut [u8], id: u64) -> Result<()> {
    if id >> 32 != STEAM_ID_HIGH || id as u32 == 0 {
        return Err(SaveError::invalid("Not an individual SteamID64."));
    }
    if steam_id(header).is_none() {
        return Err(SaveError::format("This save has no readable account field."));
    }
    header[STEAM_ID_OFFSET..STEAM_ID_OFFSET + STEAM_ID_LEN].copy_from_slice(format!("{id:016x}").as_bytes());
    Ok(())
}

/// The save folder name the game uses for an account.
pub fn steam_folder(id: u64) -> String {
    format!("{id:016x}")
}
