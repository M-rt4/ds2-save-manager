//! Entry 21 of every save is a copy of the game's regulation (its parameter BND4), stored as
//! `[u32 compressed length][u32 plain length][zlib]` and zero-filled to the entry's fixed
//! capacity (2 MB in Scholar of the First Sin, 1 MB in the original release).
//!
//! The game refuses a save whose copy does not match the running game ("The game version and
//! save data version are different"). Two things decide it: the regulation's version text at
//! BND4 +0x18 (e.g. `02000100`) and a stamp at +0x34 that is the executable's file version
//! read as `a*100 + b*10 + c` (1.0.1 → 0x65). Apart from that stamp the copy is byte for byte
//! the installed game's `enc_regulation.bnd.dcx`, decrypted and decompressed.

use std::io::{Read, Write};
use std::path::Path;

use aes::Aes128;
use ctr::cipher::{KeyIvInit, StreamCipher};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::Serialize;

use super::bytes::{u32_at, u8_at};
use super::error::{Result, SaveError};

pub const ENTRY: usize = 21;
pub const VERSION_OFFSET: usize = 0x18;
pub const STAMP_OFFSET: usize = 0x34;

/// The AES-128 key of `enc_regulation.bnd.dcx` (from Atvaark/BinderTool).
const FILE_KEY: [u8; 16] = [
    0x40, 0x17, 0x81, 0x30, 0xDF, 0x0A, 0x94, 0x54, 0x33, 0x09, 0xE1, 0x71, 0xEC, 0xBF, 0x25, 0x4C,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegulationInfo {
    /// Regulation version text, e.g. `02000100`.
    pub version: String,
    /// Game version stamp (exe file version as a*100+b*10+c).
    pub stamp: u8,
    /// The game version the stamp stands for, e.g. `1.0.1`.
    pub game_version: String,
    pub params: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regulation {
    bnd: Vec<u8>,
}

impl Regulation {
    fn new(bnd: Vec<u8>) -> Result<Self> {
        if bnd.len() < 0x40 || &bnd[..4] != b"BND4" {
            return Err(SaveError::format("The regulation copy is not a BND4 archive."));
        }
        Ok(Self { bnd })
    }

    pub fn info(&self) -> RegulationInfo {
        let stamp = u8_at(&self.bnd, STAMP_OFFSET);
        RegulationInfo {
            version: String::from_utf8_lossy(&self.bnd[VERSION_OFFSET..VERSION_OFFSET + 8]).trim_end_matches('\0').to_string(),
            stamp,
            game_version: format!("{}.{}.{}", stamp / 100, stamp / 10 % 10, stamp % 10),
            params: u32_at(&self.bnd, 0x0C),
        }
    }

    pub fn set_stamp(&mut self, stamp: u8) {
        self.bnd[STAMP_OFFSET] = stamp;
    }

    /// Reads the copy a save carries in entry 21.
    pub fn from_entry(data: &[u8]) -> Result<Self> {
        let clen = u32_at(data, 0) as usize;
        let ulen = u32_at(data, 4) as usize;
        if clen == 0 || 8 + clen > data.len() {
            return Err(SaveError::format("Entry 21 holds no regulation copy."));
        }
        let mut bnd = Vec::with_capacity(ulen);
        ZlibDecoder::new(&data[8..8 + clen])
            .read_to_end(&mut bnd)
            .map_err(|e| SaveError::format(format!("The regulation copy does not decompress: {e}")))?;
        if bnd.len() != ulen {
            return Err(SaveError::format("The regulation copy has the wrong length."));
        }
        Self::new(bnd)
    }

    /// Builds entry 21 for a save whose entry holds `capacity` bytes.
    pub fn to_entry(&self, capacity: usize) -> Result<Vec<u8>> {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&self.bnd)?;
        let packed = enc.finish()?;
        if 8 + packed.len() > capacity {
            return Err(SaveError::incompatible(format!(
                "The regulation needs {} bytes; the save only has room for {capacity}.",
                8 + packed.len()
            )));
        }
        let mut out = vec![0u8; capacity];
        out[0..4].copy_from_slice(&(packed.len() as u32).to_le_bytes());
        out[4..8].copy_from_slice(&(self.bnd.len() as u32).to_le_bytes());
        out[8..8 + packed.len()].copy_from_slice(&packed);
        Ok(out)
    }

    /// The regulation of an installed game, stamped with its executable's version, as the game
    /// itself would write it into a save.
    pub fn from_install(game_dir: &Path) -> Result<Self> {
        let raw = std::fs::read(game_dir.join("enc_regulation.bnd.dcx"))?;
        let mut reg = Self::new(decrypt_dcx(&raw)?)?;
        let (a, b, c, _) = exe_version(&game_dir.join("DarkSoulsII.exe"))?;
        reg.set_stamp(stamp_for(a, b, c));
        Ok(reg)
    }
}

/// `enc_regulation.bnd.dcx`: AES-128-CTR over a DCX whose payload is zlib.
fn decrypt_dcx(raw: &[u8]) -> Result<Vec<u8>> {
    if raw.len() < 64 {
        return Err(SaveError::format("enc_regulation.bnd.dcx is too short."));
    }
    let mut iv = [0u8; 16];
    iv[0] = 0x80;
    iv[1..12].copy_from_slice(&raw[0..11]);
    iv[15] = 1;
    let mut data = raw[32..].to_vec();
    ctr::Ctr128BE::<Aes128>::new((&FILE_KEY).into(), (&iv).into()).apply_keystream(&mut data);
    if &data[..4] != b"DCX\0" {
        return Err(SaveError::format("enc_regulation.bnd.dcx did not decrypt."));
    }
    let dca = data
        .windows(4)
        .position(|w| w == b"DCA\0")
        .ok_or_else(|| SaveError::format("enc_regulation.bnd.dcx has no DCA block."))?;
    let mut bnd = Vec::new();
    ZlibDecoder::new(&data[dca + 8..])
        .read_to_end(&mut bnd)
        .map_err(|e| SaveError::format(format!("enc_regulation.bnd.dcx does not decompress: {e}")))?;
    Ok(bnd)
}

/// The file version in an executable's VS_FIXEDFILEINFO.
pub fn exe_version(exe: &Path) -> Result<(u16, u16, u16, u16)> {
    let data = std::fs::read(exe)?;
    let sig = 0xFEEF_04BDu32.to_le_bytes();
    let at = data
        .windows(4)
        .position(|w| w == sig)
        .ok_or_else(|| SaveError::format("The game executable carries no version."))?;
    let ms = u32_at(&data, at + 8);
    let ls = u32_at(&data, at + 12);
    Ok(((ms >> 16) as u16, ms as u16, (ls >> 16) as u16, ls as u16))
}

/// The stamp a game of this version writes (1.0.1 → 101).
pub fn stamp_for(a: u16, b: u16, c: u16) -> u8 {
    (a as u32 * 100 + b as u32 * 10 + c as u32).min(255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_round_trip_and_capacity() {
        let mut bnd = b"BND4".to_vec();
        bnd.resize(0x400, 7);
        bnd[0x18..0x20].copy_from_slice(b"02000100");
        let mut reg = Regulation::new(bnd).unwrap();
        reg.set_stamp(0x65);
        let entry = reg.to_entry(4096).unwrap();
        assert_eq!(entry.len(), 4096);
        let back = Regulation::from_entry(&entry).unwrap();
        assert_eq!(back, reg);
        assert_eq!(back.info().version, "02000100");
        assert_eq!(back.info().game_version, "1.0.1");
        assert!(reg.to_entry(16).is_err());
    }

    /// Checks the reader against an installed game when `DS2_GAME_DIR` points at one.
    #[test]
    fn installed_game_when_available() {
        let Some(dir) = std::env::var_os("DS2_GAME_DIR") else { return };
        let reg = Regulation::from_install(Path::new(&dir)).unwrap();
        let info = reg.info();
        eprintln!("{info:?}");
        assert_eq!(info.version.len(), 8);
        assert!(info.params > 100);
    }
}
