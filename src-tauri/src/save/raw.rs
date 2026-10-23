//! Raw access to a save's decrypted blocks, for the Raw data tab: reading a window of
//! bytes, overwriting bytes in place (a block never changes size) and finding a byte
//! pattern. `SaveFile::encode` recomputes the checksums when the file is written; nothing
//! here checks what the bytes mean.

use serde::Serialize;

use super::error::{Result, SaveError};
use super::file::SaveFile;

fn block(save: &SaveFile, index: usize) -> Result<&[u8]> {
    if index >= save.block_count() {
        return Err(SaveError::invalid(format!("This save has no block {index}.")));
    }
    Ok(save.block(index))
}

/// Up to `len` bytes of block `index` from `offset`; empty past its end.
pub fn read(save: &SaveFile, index: usize, offset: usize, len: usize) -> Result<&[u8]> {
    let data = block(save, index)?;
    let start = offset.min(data.len());
    Ok(&data[start..start + len.min(data.len() - start)])
}

/// Overwrites bytes of block `index` at `offset`, refused whole when they would run past
/// the block's end. Returns whether any byte changed.
pub fn write(save: &mut SaveFile, index: usize, offset: usize, bytes: &[u8]) -> Result<bool> {
    let len = block(save, index)?.len();
    if bytes.is_empty() {
        return Err(SaveError::invalid("Nothing to write."));
    }
    let end = offset.checked_add(bytes.len()).filter(|&end| end <= len).ok_or_else(|| {
        SaveError::new("rawOutOfRange", format!("Block {index} is {len} bytes; the write would run past its end."))
    })?;
    let target = &mut save.block_mut(index)[offset..end];
    if target == bytes {
        return Ok(false);
    }
    target.copy_from_slice(bytes);
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub index: usize,
    pub offset: usize,
}

/// Where `pattern` occurs, in block `index` or in every block when it is `None`: the
/// first `max` matches, and whether there are more.
pub fn find(save: &SaveFile, index: Option<usize>, pattern: &[u8], max: usize) -> Result<(Vec<Match>, bool)> {
    if pattern.is_empty() {
        return Err(SaveError::invalid("Search for at least one byte."));
    }
    let indices = match index {
        Some(i) => block(save, i).map(|_| i..i + 1)?,
        None => 0..save.block_count(),
    };
    let mut matches = Vec::new();
    for i in indices {
        for (offset, window) in save.block(i).windows(pattern.len()).enumerate() {
            if window == pattern {
                if matches.len() == max {
                    return Ok((matches, true));
                }
                matches.push(Match { index: i, offset });
            }
        }
    }
    Ok((matches, false))
}

#[cfg(test)]
mod tests {
    use super::super::character::{self, off};
    use super::super::file::{character_entry, HEADER_ENTRY};
    use super::super::testdata;
    use super::super::Edition;
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn writes_bytes_that_survive_encoding() {
        let mut s = SaveFile::from_blocks(Edition::Scholar, &testdata::save_blocks(&[(0, "Nele", 37)], 64));
        let entry = character_entry(0);
        let others: Vec<Vec<u8>> = (0..s.block_count()).filter(|&i| i != entry).map(|i| s.block(i).to_vec()).collect();

        assert_eq!(read(&s, entry, off::NAME, 8).unwrap(), utf16("Nele"));
        assert!(write(&mut s, entry, off::NAME, &utf16("Pate")).unwrap());
        assert!(!write(&mut s, entry, off::NAME, &utf16("Pate")).unwrap(), "the same bytes again change nothing");
        assert_eq!(character::read(&s, 0).name, "Pate");

        let reread = SaveFile::parse(s.encode()).unwrap();
        assert!(reread.checksum_failures().is_empty());
        assert_eq!(read(&reread, entry, off::NAME, 8).unwrap(), utf16("Pate"));
        let after: Vec<Vec<u8>> = (0..reread.block_count()).filter(|&i| i != entry).map(|i| reread.block(i).to_vec()).collect();
        assert_eq!(after, others);
    }

    #[test]
    fn refuses_writes_outside_a_block() {
        let mut s = SaveFile::from_blocks(Edition::Original, &testdata::save_blocks(&[(0, "Nele", 37)], 64));
        let before = s.encode();
        let len = s.block(HEADER_ENTRY).len();
        assert_eq!(write(&mut s, HEADER_ENTRY, len - 1, &[1, 2]).unwrap_err().code, "rawOutOfRange");
        assert_eq!(write(&mut s, HEADER_ENTRY, usize::MAX, &[1]).unwrap_err().code, "rawOutOfRange");
        assert!(write(&mut s, 99, 0, &[1]).is_err());
        assert!(write(&mut s, HEADER_ENTRY, 0, &[]).is_err());
        assert_eq!(s.encode(), before);
        assert!(write(&mut s, HEADER_ENTRY, len - 1, &[0xAB]).unwrap());

        assert_eq!(read(&s, HEADER_ENTRY, len - 2, 100).unwrap().len(), 2);
        assert!(read(&s, HEADER_ENTRY, len + 5, 100).unwrap().is_empty());
    }

    #[test]
    fn finds_patterns_in_one_block_or_all() {
        let s = SaveFile::from_blocks(Edition::Scholar, &testdata::save_blocks(&[(0, "Nele", 37), (3, "Nele", 12)], 64));
        let name = utf16("Nele");
        let (all, more) = find(&s, None, &name, 100).unwrap();
        assert!(!more);
        // Each character's own block, plus the two title lists (entries 0 and 22).
        let blocks: Vec<usize> = all.iter().map(|m| m.index).collect();
        assert_eq!(blocks, [0, 0, 1, 4, 22, 22]);
        assert!(all.contains(&Match { index: 1, offset: off::NAME }));

        let (one, _) = find(&s, Some(4), &name, 100).unwrap();
        assert_eq!(one, [Match { index: 4, offset: off::NAME }]);
        let (first, more) = find(&s, None, &name, 2).unwrap();
        assert_eq!((first.len(), more), (2, true));
        assert!(find(&s, Some(0), &[], 10).is_err());
        assert!(find(&s, Some(23), &name, 10).is_err());
    }
}
