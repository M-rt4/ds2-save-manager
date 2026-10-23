//! Every decrypted DS2 block is a run of chunks, each `[u32 type][u32 version][u32 size]`
//! plus 20 reserved bytes, followed by `size` bytes of data. The chunk table is how two
//! saves are checked for layout compatibility before anything is copied between them.

use std::ops::Range;

use serde::Serialize;

use super::bytes::u32_at;

pub const CHUNK_HEADER_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub kind: u32,
    pub version: u32,
    /// Offset of the chunk header inside the block.
    pub offset: usize,
    /// Length of the chunk data, header excluded.
    pub size: usize,
}

impl Chunk {
    pub fn data(&self) -> Range<usize> {
        let start = self.offset + CHUNK_HEADER_LEN;
        start..start + self.size
    }
}

/// The chunk table of a block, or `None` when the chunks don't tile the block exactly
/// (an unknown layout, which every writer treats as "don't touch").
pub fn walk(block: &[u8]) -> Option<Vec<Chunk>> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < block.len() {
        if offset + CHUNK_HEADER_LEN > block.len() || chunks.len() > 64 {
            return None;
        }
        let size = u32_at(block, offset + 8) as usize;
        let end = offset.checked_add(CHUNK_HEADER_LEN)?.checked_add(size)?;
        if end > block.len() {
            return None;
        }
        chunks.push(Chunk {
            kind: u32_at(block, offset),
            version: u32_at(block, offset + 4),
            offset,
            size,
        });
        offset = end;
    }
    (!chunks.is_empty()).then_some(chunks)
}

pub fn find(block: &[u8], kind: u32) -> Option<Chunk> {
    walk(block)?.into_iter().find(|c| c.kind == kind)
}

/// Two blocks have the same shape when their chunk types and sizes line up; versions may
/// differ (that is reported separately, see `version_differences`).
pub fn same_shape(a: &[u8], b: &[u8]) -> bool {
    match (walk(a), walk(b)) {
        (Some(a), Some(b)) => {
            a.len() == b.len()
                && a.iter().zip(&b).all(|(x, y)| x.kind == y.kind && x.size == y.size && x.offset == y.offset)
        }
        _ => false,
    }
}

/// `(type, version in a, version in b)` for every chunk whose version differs.
pub fn version_differences(a: &[u8], b: &[u8]) -> Vec<(u32, u32, u32)> {
    match (walk(a), walk(b)) {
        (Some(a), Some(b)) => a
            .iter()
            .zip(&b)
            .filter(|(x, y)| x.kind == y.kind && x.version != y.version)
            .map(|(x, y)| (x.kind, x.version, y.version))
            .collect(),
        _ => Vec::new(),
    }
}
