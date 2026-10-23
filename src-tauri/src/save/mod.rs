//! Dark Souls II `.sl2` saves: the BND4 container, its AES-encrypted entries and the
//! character data inside them. The original release and Scholar of the First Sin share
//! one layout; only the encryption key differs.

pub mod backups;
mod bnd4;
pub mod bytes;
pub mod character;
pub mod chunks;
mod crypto;
pub mod disk;
pub mod edition;
pub mod error;
pub mod file;
pub mod header;
pub mod inventory;
pub mod items;
pub mod maps;
pub mod transfer;

#[cfg(test)]
mod testdata;
#[cfg(test)]
mod tests;

pub use edition::Edition;
pub use error::{Result, SaveError};
pub use file::SaveFile;
