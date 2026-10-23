//! Dark Souls II `.sl2` saves: the BND4 container, its AES-encrypted entries and the
//! character data inside them. The original release and Scholar of the First Sin share
//! one layout; only the encryption key differs.

pub mod backups;
mod bnd4;
pub mod bytes;
pub mod character;
pub mod chunks;
pub mod conversion;
mod crypto;
pub mod disk;
pub mod document;
pub mod edition;
pub mod error;
pub mod file;
pub mod games;
pub mod header;
pub mod inventory;
pub mod inventory_changes;
pub mod items;
pub mod maps;
pub mod platform;
pub mod raw;
pub mod regulation;
pub mod transfer;

#[cfg(test)]
mod conversion_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod testdata;
#[cfg(test)]
mod tests;

pub use edition::Edition;
pub use error::{Result, SaveError};
pub use file::SaveFile;
