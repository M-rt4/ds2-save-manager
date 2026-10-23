//! The BND4 archive an `.sl2` file is. Only the entry table is read; entries are always
//! rewritten in place at their original size, so the container itself never changes.

use super::bytes::{u32_at, u64_at, u8_at};
use super::crypto::ENTRY_HEADER_LEN;
use super::error::{Result, SaveError};

pub const HEADER_LEN: usize = 0x40;
/// The version string DS2 stamps into its saves (both releases).
pub const DS2_SIGNATURE: &[u8; 8] = b"14e503cb";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub index: usize,
    pub offset: usize,
    pub size: usize,
    pub name: String,
}

pub fn parse(data: &[u8]) -> Result<Vec<Entry>> {
    if data.len() < HEADER_LEN || &data[..4] != b"BND4" {
        return Err(SaveError::new("notSave", "Not a BND4 / .sl2 file."));
    }
    let count = u32_at(data, 0x0C) as usize;
    if count == 0 || count > 64 {
        return Err(SaveError::format(format!("Implausible entry count {count}.")));
    }
    let entry_len = u64_at(data, 0x20) as usize;
    if !(24..=64).contains(&entry_len) {
        return Err(SaveError::format(format!("Unknown entry header size {entry_len}.")));
    }
    let unicode = u8_at(data, 0x30) == 1;

    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let base = HEADER_LEN + index * entry_len;
        if base + entry_len > data.len() {
            return Err(SaveError::format(format!("Entry header {index} is truncated.")));
        }
        let size = u64_at(data, base + 8) as usize;
        let offset = u32_at(data, base + 16) as usize;
        let name_offset = u32_at(data, base + 20) as usize;
        let fits = offset.checked_add(size).is_some_and(|end| end <= data.len());
        if !fits || size <= ENTRY_HEADER_LEN || (size - ENTRY_HEADER_LEN) % 16 != 0 {
            return Err(SaveError::format(format!(
                "Entry {index} is malformed (offset {offset}, size {size})."
            )));
        }
        entries.push(Entry { index, offset, size, name: read_name(data, name_offset, unicode) });
    }

    let mut by_offset: Vec<&Entry> = entries.iter().collect();
    by_offset.sort_by_key(|e| e.offset);
    if by_offset.windows(2).any(|w| w[0].offset + w[0].size > w[1].offset) {
        return Err(SaveError::format("Entries overlap."));
    }
    Ok(entries)
}

pub fn has_ds2_signature(data: &[u8]) -> bool {
    data.get(0x18..0x20) == Some(&DS2_SIGNATURE[..])
}

fn read_name(data: &[u8], offset: usize, unicode: bool) -> String {
    if offset == 0 || offset >= data.len() {
        return String::new();
    }
    if unicode {
        super::bytes::utf16_at(data, offset, 64)
    } else {
        data[offset..].iter().take_while(|&&b| b != 0).take(64).map(|&b| b as char).collect()
    }
}
