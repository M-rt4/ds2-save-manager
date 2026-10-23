//! Each BND4 entry of a DS2 save is `[MD5 16][IV 16][AES-128-CBC ciphertext]`. The MD5
//! covers IV + ciphertext. The plaintext is `[u32 length][data][padding]`, padded up to
//! the next 16-byte boundary.

use aes::Aes128;
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use md5::{Digest, Md5};

use super::bytes::u32_at;

pub const ENTRY_HEADER_LEN: usize = 32;

pub fn md5(data: &[u8]) -> [u8; 16] {
    Md5::digest(data).into()
}

/// Whether the entry's stored MD5 matches its IV and ciphertext.
pub fn checksum_ok(blob: &[u8]) -> bool {
    blob.len() >= ENTRY_HEADER_LEN && md5(&blob[16..]) == blob[..16]
}

pub fn iv_of(blob: &[u8]) -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&blob[16..32]);
    iv
}

/// Decrypts an entry's ciphertext. The caller guarantees whole 16-byte blocks.
pub fn decrypt(key: &[u8; 16], blob: &[u8]) -> Vec<u8> {
    let iv = iv_of(blob);
    let mut buf = blob[ENTRY_HEADER_LEN..].to_vec();
    cbc::Decryptor::<Aes128>::new(key.into(), (&iv).into())
        .decrypt_padded_mut::<NoPadding>(&mut buf)
        .expect("ciphertext is a whole number of blocks");
    buf
}

/// Builds a complete entry blob (`MD5 + IV + ciphertext`) for a plaintext.
pub fn encrypt(key: &[u8; 16], iv: &[u8; 16], plain: &[u8]) -> Vec<u8> {
    let mut ct = plain.to_vec();
    let len = ct.len();
    cbc::Encryptor::<Aes128>::new(key.into(), iv.into())
        .encrypt_padded_mut::<NoPadding>(&mut ct, len)
        .expect("plaintext is a whole number of blocks");
    let mut blob = Vec::with_capacity(ENTRY_HEADER_LEN + ct.len());
    blob.extend_from_slice(&[0u8; 16]);
    blob.extend_from_slice(iv);
    blob.extend_from_slice(&ct);
    let sum = md5(&blob[16..]);
    blob[..16].copy_from_slice(&sum);
    blob
}

/// The data length a plaintext declares, if it is consistent with the block: the
/// length plus its 4-byte prefix must fill the plaintext up to the last, partial block.
/// A wrong key decrypts to noise whose "length" fails this almost surely, so it doubles
/// as the key check.
pub fn declared_len(plain: &[u8]) -> Option<usize> {
    let len = u32_at(plain, 0) as usize;
    let used = len.checked_add(4)?;
    (len > 0 && used <= plain.len() && plain.len() - used < 16).then_some(len)
}

/// Pads `data` the way the game does: length prefix, data, then `n` bytes of value `n`.
#[cfg(test)]
pub fn wrap_payload(data: &[u8]) -> Vec<u8> {
    let pad = (16 - (data.len() + 4) % 16) % 16;
    let mut plain = Vec::with_capacity(data.len() + 4 + pad);
    plain.extend_from_slice(&(data.len() as u32).to_le_bytes());
    plain.extend_from_slice(data);
    plain.extend(std::iter::repeat_n(pad as u8, pad));
    plain
}
