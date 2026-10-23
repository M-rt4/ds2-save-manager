use serde::{Deserialize, Serialize};

/// The two PC releases of Dark Souls II. They share one save layout; each encrypts its
/// entries with its own AES-128 key (both published in TKGP's SoulsFormats `SFUtil`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Edition {
    /// The 2014 DX9 release, `DARKSII0000.sl2`.
    Original,
    /// Scholar of the First Sin (DX11), `DS2SOFS0000.sl2`.
    Scholar,
}

const ORIGINAL_KEY: [u8; 16] = [
    0xB7, 0xFD, 0x46, 0x3E, 0x4A, 0x9C, 0x11, 0x02, 0xDF, 0x17, 0x39, 0xE5, 0xF3, 0xB2, 0xA5, 0x0F,
];
const SCHOLAR_KEY: [u8; 16] = [
    0x59, 0x9F, 0x9B, 0x69, 0x96, 0x40, 0xA5, 0x52, 0x36, 0xEE, 0x2D, 0x70, 0x83, 0x5E, 0xC7, 0x44,
];

impl Edition {
    pub const ALL: [Edition; 2] = [Edition::Scholar, Edition::Original];

    pub fn key(self) -> &'static [u8; 16] {
        match self {
            Edition::Original => &ORIGINAL_KEY,
            Edition::Scholar => &SCHOLAR_KEY,
        }
    }

    /// The file name the game reads its first save from.
    pub fn file_name(self) -> &'static str {
        match self {
            Edition::Original => "DARKSII0000.sl2",
            Edition::Scholar => "DS2SOFS0000.sl2",
        }
    }

    pub fn other(self) -> Edition {
        match self {
            Edition::Original => Edition::Scholar,
            Edition::Scholar => Edition::Original,
        }
    }

    /// Guesses the edition from a file name, for listing saves without decrypting them.
    pub fn from_file_name(name: &str) -> Option<Edition> {
        let upper = name.to_ascii_uppercase();
        if upper.starts_with("DS2SOFS") {
            Some(Edition::Scholar)
        } else if upper.starts_with("DARKSII") {
            Some(Edition::Original)
        } else {
            None
        }
    }
}
