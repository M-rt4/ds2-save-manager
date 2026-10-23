//! Moving characters between saves and between the two releases. The releases share the
//! block layout, so converting a file means encrypting it again with the other key, and
//! moving a character means copying its character block, its world block and its title
//! record (in entry 0 and in the copy in entry 22).

use serde::Serialize;

use super::chunks;
use super::edition::Edition;
use super::error::{Result, SaveError};
use super::file::{character_entry, world_entry, SaveFile, HEADER_ENTRY, SLOT_COUNT, SUMMARY_ENTRY};
use super::header::{self, TITLE_HEAD_LEN};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionNote {
    pub entry: usize,
    pub chunk: u32,
    pub source: u32,
    pub target: u32,
}

/// Whether characters can move from `src` to `dst`, plus the chunk versions that differ
/// between them (which the game may or may not accept; reported, not enforced).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    pub compatible: bool,
    pub problems: Vec<String>,
    pub version_notes: Vec<VersionNote>,
    /// Entries outside the character data whose size differs; the target keeps its own.
    pub system_size_differences: Vec<usize>,
}

/// Entries compared for version differences: menu, first character, first world block and
/// the two system blocks.
const PROBE_ENTRIES: [usize; 5] = [HEADER_ENTRY, character_entry(0), world_entry(0), 21, SUMMARY_ENTRY];

pub fn compare(src: &SaveFile, dst: &SaveFile) -> Compatibility {
    let mut problems = Vec::new();
    if src.block_count() != dst.block_count() {
        problems.push(format!("Entry counts differ ({} vs {}).", src.block_count(), dst.block_count()));
    }
    for (label, entry) in [("character", character_entry(0)), ("world", world_entry(0))] {
        if entry >= src.block_count().min(dst.block_count()) {
            continue;
        }
        let (a, b) = (src.raw_block(entry), dst.raw_block(entry));
        if a.plain().len() != b.plain().len() || !chunks::same_shape(a.data(), b.data()) {
            problems.push(format!("The {label} blocks have different layouts."));
        }
    }
    if header::title_data(src.block(HEADER_ENTRY)).is_none() || header::title_data(dst.block(HEADER_ENTRY)).is_none() {
        problems.push("A title list is missing.".into());
    }

    let mut version_notes = Vec::new();
    let mut system_size_differences = Vec::new();
    for entry in PROBE_ENTRIES {
        if entry >= src.block_count().min(dst.block_count()) {
            continue;
        }
        let (a, b) = (src.block(entry), dst.block(entry));
        if a.len() != b.len() {
            system_size_differences.push(entry);
        }
        for (chunk, source, target) in chunks::version_differences(a, b) {
            version_notes.push(VersionNote { entry, chunk, source, target });
        }
    }
    system_size_differences.retain(|e| *e != character_entry(0) && *e != world_entry(0));
    Compatibility { compatible: problems.is_empty(), problems, version_notes, system_size_differences }
}

/// Copies the character in slot `from` of `src` over slot `to` of `dst`. An empty source
/// slot empties the target slot. Nothing is changed unless every part fits.
pub fn transfer(src: &SaveFile, from: usize, dst: &mut SaveFile, to: usize) -> Result<()> {
    if from >= SLOT_COUNT || to >= SLOT_COUNT {
        return Err(SaveError::invalid("Slots are 1 to 10."));
    }
    for (a, b) in [(character_entry(from), character_entry(to)), (world_entry(from), world_entry(to))] {
        let (x, y) = (src.raw_block(a), dst.raw_block(b));
        if x.plain().len() != y.plain().len() || !chunks::same_shape(x.data(), y.data()) {
            return Err(SaveError::incompatible("The two saves lay out their character data differently."));
        }
    }
    let source_record = header::record(src.block(HEADER_ENTRY), from)
        .map(|r| src.block(HEADER_ENTRY)[r].to_vec())
        .ok_or_else(|| SaveError::format("The source has no title list."))?;
    if header::record(dst.block(HEADER_ENTRY), to).is_none() {
        return Err(SaveError::format("The target has no title list."));
    }

    dst.replace_block(character_entry(to), src.raw_block(character_entry(from)))?;
    dst.replace_block(world_entry(to), src.raw_block(world_entry(from)))?;
    for e in [HEADER_ENTRY, SUMMARY_ENTRY] {
        if e >= dst.block_count() {
            continue;
        }
        // Prefer the matching copy of the source, fall back to its entry-0 record.
        let record = (e < src.block_count())
            .then(|| header::record(src.block(e), from).map(|r| src.block(e)[r].to_vec()))
            .flatten()
            .unwrap_or_else(|| source_record.clone());
        if let Some(r) = header::record(dst.block(e), to) {
            dst.block_mut(e)[r].copy_from_slice(&record);
        }
    }
    Ok(())
}

/// The whole of `src` re-encrypted for `target`.
pub fn convert(src: &SaveFile, target: Edition) -> SaveFile {
    let mut out = src.clone();
    out.edition = target;
    out
}

/// `base` (normally an existing save of the target release) with all ten slots replaced
/// by those of `src`. The base keeps its own account, options and system blocks.
pub fn merge(src: &SaveFile, base: &SaveFile) -> Result<SaveFile> {
    let check = compare(src, base);
    if !check.compatible {
        return Err(SaveError::incompatible(check.problems.join(" ")));
    }
    let mut out = base.clone();
    for slot in 0..SLOT_COUNT {
        transfer(src, slot, &mut out, slot)?;
    }
    // The title head says which slot "Continue" opens.
    let head = header::title_data(src.block(HEADER_ENTRY)).map(|r| src.block(HEADER_ENTRY)[r.start..r.start + TITLE_HEAD_LEN].to_vec());
    if let Some(head) = head {
        for e in [HEADER_ENTRY, SUMMARY_ENTRY] {
            if e < out.block_count() {
                if let Some(r) = header::title_data(out.block(e)) {
                    out.block_mut(e)[r.start..r.start + TITLE_HEAD_LEN].copy_from_slice(&head);
                }
            }
        }
    }
    Ok(out)
}
