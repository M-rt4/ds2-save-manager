use super::bnd4::{self, Entry};
use super::bytes::u64_at;
use super::crypto;
use super::edition::Edition;
use super::error::{Result, SaveError};

/// Character slots shown in the game's Load Game menu.
pub const SLOT_COUNT: usize = 10;
/// Entry 0: menu data (title list of the ten slots, Steam account, options).
pub const HEADER_ENTRY: usize = 0;
/// Entry 22: a second copy of the title list.
pub const SUMMARY_ENTRY: usize = 22;
/// A DS2 save has 23 entries: header, ten character blocks, ten world blocks and two
/// system blocks.
pub const DS2_ENTRY_COUNT: usize = 23;

/// BND4 entry holding the character (status + inventory) of a 0-based slot.
pub const fn character_entry(slot: usize) -> usize {
    1 + slot
}

/// BND4 entry holding the world state (flags, bonfires, map progress) of a 0-based slot.
pub const fn world_entry(slot: usize) -> usize {
    1 + SLOT_COUNT + slot
}

/// One decrypted entry. `plain` keeps the length prefix and the original padding so that
/// re-encrypting it reproduces a blob of exactly the original size.
#[derive(Debug, Clone)]
pub struct Block {
    pub iv: [u8; 16],
    plain: Vec<u8>,
    len: usize,
    pub checksum_ok: bool,
}

impl Block {
    pub fn data(&self) -> &[u8] {
        &self.plain[4..4 + self.len]
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.plain[4..4 + self.len]
    }

    pub fn plain(&self) -> &[u8] {
        &self.plain
    }
}

/// A whole decrypted save. Edits happen on the decrypted blocks; `encode` encrypts them
/// again with the key of `edition`, which is all a conversion between releases changes.
#[derive(Debug, Clone)]
pub struct SaveFile {
    /// The release the file was read as.
    pub source_edition: Edition,
    /// The release the file will be written as.
    pub edition: Edition,
    raw: Vec<u8>,
    entries: Vec<Entry>,
    blocks: Vec<Block>,
}

impl SaveFile {
    pub fn parse(raw: Vec<u8>) -> Result<Self> {
        let entries = bnd4::parse(&raw)?;
        if entries.len() < SUMMARY_ENTRY {
            return Err(SaveError::new(
                "notDs2",
                format!("This save has {} entries; Dark Souls II saves have {DS2_ENTRY_COUNT}.", entries.len()),
            ));
        }
        let edition = detect(&raw, &entries).ok_or_else(|| {
            let code = if bnd4::has_ds2_signature(&raw) { "unknownKey" } else { "notDs2" };
            SaveError::new(code, "Neither the original nor the Scholar of the First Sin key opens this save.")
        })?;

        let mut blocks = Vec::with_capacity(entries.len());
        for e in &entries {
            let blob = &raw[e.offset..e.offset + e.size];
            let plain = crypto::decrypt(edition.key(), blob);
            let len = crypto::declared_len(&plain).ok_or_else(|| {
                SaveError::new("corrupt", format!("Entry {} ({}) does not decrypt cleanly.", e.index, e.name))
            })?;
            blocks.push(Block { iv: crypto::iv_of(blob), plain, len, checksum_ok: crypto::checksum_ok(blob) });
        }
        Ok(Self { source_edition: edition, edition, raw, entries, blocks })
    }

    /// The complete file, every entry encrypted with the key of `self.edition`. Entries keep
    /// their place; when one changed size (see `set_block_data`) the entries are laid out
    /// again back to back in index order, as the game writes them, and the entry table is
    /// updated to match.
    pub fn encode(&self) -> Vec<u8> {
        let blobs: Vec<Vec<u8>> = self.blocks.iter().map(|b| crypto::encrypt(self.edition.key(), &b.iv, &b.plain)).collect();
        if self.entries.iter().zip(&blobs).all(|(e, b)| e.size == b.len()) {
            let mut out = self.raw.clone();
            for (e, blob) in self.entries.iter().zip(&blobs) {
                out[e.offset..e.offset + e.size].copy_from_slice(blob);
            }
            return out;
        }
        let data_start = self.entries.iter().map(|e| e.offset).min().unwrap_or(self.raw.len());
        let entry_len = u64_at(&self.raw, 0x20) as usize;
        let mut out = self.raw[..data_start].to_vec();
        for (e, blob) in self.entries.iter().zip(&blobs) {
            let header = bnd4::HEADER_LEN + e.index * entry_len;
            let offset = out.len();
            out[header + 8..header + 16].copy_from_slice(&(blob.len() as u64).to_le_bytes());
            out[header + 16..header + 20].copy_from_slice(&(offset as u32).to_le_bytes());
            out.extend_from_slice(blob);
        }
        out
    }

    /// Replaces a block's data with `data` of any length; `encode` re-lays the container.
    pub fn set_block_data(&mut self, index: usize, data: &[u8]) {
        let block = &mut self.blocks[index];
        block.plain = crypto::wrap_payload(data);
        block.len = data.len();
    }

    /// Checks that `bytes`, this save as `encode` wrote it, reads back as this save: the same
    /// release, the same number of entries, and every entry's data byte for byte in its place.
    /// Entries whose checksum already failed when the file was read may still fail; no other
    /// may.
    pub fn verify_encoded(&self, bytes: &[u8]) -> Result<()> {
        let failed = || SaveError::new("verify", "The encoded save did not read back correctly; nothing was written.");
        let reread = SaveFile::parse(bytes.to_vec()).map_err(|_| failed())?;
        if reread.source_edition != self.edition
            || reread.block_count() != self.block_count()
            || (0..self.block_count()).any(|i| reread.block(i) != self.block(i))
            || reread.checksum_failures().len() > self.checksum_failures().len()
        {
            return Err(failed());
        }
        Ok(())
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn block(&self, index: usize) -> &[u8] {
        self.blocks[index].data()
    }

    pub fn block_mut(&mut self, index: usize) -> &mut [u8] {
        self.blocks[index].data_mut()
    }

    pub fn raw_block(&self, index: usize) -> &Block {
        &self.blocks[index]
    }

    /// Replaces a whole block (prefix and padding included) with one of the same size.
    pub fn replace_block(&mut self, index: usize, from: &Block) -> Result<()> {
        let to = &mut self.blocks[index];
        if to.plain.len() != from.plain.len() || to.len != from.len {
            return Err(SaveError::incompatible(format!(
                "Entry {index} differs in size ({} vs {} bytes).",
                to.len, from.len
            )));
        }
        to.plain.copy_from_slice(&from.plain);
        Ok(())
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Entries whose stored MD5 did not match when the file was read.
    pub fn checksum_failures(&self) -> Vec<usize> {
        self.blocks.iter().enumerate().filter(|(_, b)| !b.checksum_ok).map(|(i, _)| i).collect()
    }

    /// Raw file bytes as read, still encrypted with the source edition's key.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

/// Which release's key opens the save. Entry 0 and entry 1 must both decrypt to a
/// consistent length prefix, which a wrong key essentially never produces.
fn detect(raw: &[u8], entries: &[Entry]) -> Option<Edition> {
    Edition::ALL.into_iter().find(|ed| {
        entries.iter().take(2).all(|e| {
            let blob = &raw[e.offset..e.offset + e.size];
            crypto::declared_len(&crypto::decrypt(ed.key(), blob)).is_some()
        })
    })
}

#[cfg(test)]
impl SaveFile {
    /// Builds a save in memory from decrypted entry data, for tests.
    pub fn from_blocks(edition: Edition, datas: &[Vec<u8>]) -> Self {
        let raw = super::testdata::container(edition, datas);
        Self::parse(raw).expect("synthetic save parses")
    }
}
