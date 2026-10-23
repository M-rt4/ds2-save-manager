//! Synthetic saves for tests: a real BND4 container with the DS2 entry table and blocks
//! that carry the real chunk layout, with only the fields the tests read filled in.

use super::bytes::{put_u16, put_u32, put_utf16};
use super::character::{off, ATTR_OFFSETS};
use super::chunks::CHUNK_HEADER_LEN;
use super::crypto::{encrypt, wrap_payload};
use super::edition::Edition;
use super::header::{rec, RECORD_LEN, STEAM_ID_OFFSET, TITLE_HEAD_LEN};

pub const STEAM: u64 = 0x0110_0001_3f6e_c4a6;

fn chunked(layout: &[(u32, u32, usize)]) -> Vec<u8> {
    let mut b = Vec::new();
    for &(kind, version, size) in layout {
        let start = b.len();
        b.resize(start + CHUNK_HEADER_LEN + size, 0);
        put_u32(&mut b, start, kind);
        put_u32(&mut b, start + 4, version);
        put_u32(&mut b, start + 8, size as u32);
    }
    b
}

pub fn header_block(names: &[(usize, &str, u16)], title_version: u32) -> Vec<u8> {
    let mut b = chunked(&[(3, 0x66, 812), (4, title_version, 4976), (0xD, 0x102, 904)]);
    b[STEAM_ID_OFFSET..STEAM_ID_OFFSET + 16].copy_from_slice(format!("{STEAM:016x}").as_bytes());
    fill_titles(&mut b, 844 + CHUNK_HEADER_LEN, names);
    b
}

pub fn summary_block(names: &[(usize, &str, u16)]) -> Vec<u8> {
    let mut b = chunked(&[(4, 0x6E, 4976), (0xD, 0x102, 8192)]);
    fill_titles(&mut b, CHUNK_HEADER_LEN, names);
    b
}

fn fill_titles(b: &mut [u8], data: usize, names: &[(usize, &str, u16)]) {
    put_u32(b, data, names.first().map_or(0, |n| n.0 as u32));
    put_u32(b, data + 4, 10);
    for &(slot, name, level) in names {
        let r = data + TITLE_HEAD_LEN + slot * RECORD_LEN;
        put_utf16(b, r + rec::NAME, rec::NAME_CHARS + 1, name);
        put_u16(b, r + rec::LEVEL, level);
        put_u32(b, r + rec::PLAY_TIME, 3600 + slot as u32);
        put_u32(b, r + rec::MAP, 10_040_000);
        for i in 0..9 {
            put_u16(b, r + rec::ATTRS + i * 2, 10);
        }
    }
}

pub fn character_block(name: &str, attrs: [u16; 9]) -> Vec<u8> {
    let mut b = chunked(&[
        (0x0, 0x6F, 860),
        (0x1, 0x6D, 1136),
        (0x5, 0x65, 4004),
        (0x6, 0x6F, 1564),
        (0xA, 0x6C, 65724),
        (0xC, 0x66, 5012),
        (0xE, 0x65, 1540),
        (0xD, 0x102, 31196),
    ]);
    if !name.is_empty() {
        put_utf16(&mut b, off::NAME, off::NAME_CHARS + 1, name);
        for (o, v) in ATTR_OFFSETS.iter().zip(attrs) {
            put_u16(&mut b, *o, v);
        }
        let level: i32 = attrs.iter().map(|&v| v as i32).sum::<i32>() - 53;
        put_u16(&mut b, off::LEVEL, level as u16);
        put_u32(&mut b, off::SOULS, 1234);
        put_u16(&mut b, off::JOURNEY, 1);
        put_u32(&mut b, off::CLASS, 2);
        // Estus Flask 1 (charges 3/3), a +5 fire Longsword, 10 Titanite Shards.
        let mut item = |slot: usize, id: u32, plus8: [u8; 4], plus12: [u8; 4]| {
            let o = super::inventory::MAIN.start + slot * 16;
            put_u32(&mut b, o, id);
            b[o + 8..o + 12].copy_from_slice(&plus8);
            b[o + 12..o + 16].copy_from_slice(&plus12);
        };
        item(0, 60_155_000, [1, 0, 3, 3], [0; 4]);
        item(1, 1_220_000, 40f32.to_le_bytes(), [5, 1, 0, 0]);
        item(3, 60_970_000, [10, 0, 0, 0], [0; 4]);
    }
    b
}

pub fn world_block(marker: u8) -> Vec<u8> {
    let mut b = chunked(&[(2, 0x67, 2048), (7, 0x65, 512), (0xD, 0x102, 1024)]);
    b[100] = marker;
    b
}

/// A complete save: header, ten characters (given by `names`), ten worlds, two system
/// blocks.
pub fn save_blocks(names: &[(usize, &str, u16)], system_len: usize) -> Vec<Vec<u8>> {
    let mut blocks = vec![header_block(names, 0x6D)];
    for slot in 0..10 {
        let name = names.iter().find(|n| n.0 == slot).map_or("", |n| n.1);
        blocks.push(character_block(name, [10, 10, 10, 10, 10, 10, 10, 10, 10 + slot as u16]));
    }
    for slot in 0..10 {
        blocks.push(world_block(slot as u8 + 1));
    }
    blocks.push(vec![0x5A; system_len]);
    blocks.push(summary_block(names));
    blocks
}

/// A BND4 container like the game writes, entries encrypted with the edition's key.
pub fn container(edition: Edition, datas: &[Vec<u8>]) -> Vec<u8> {
    let count = datas.len();
    let names_at = 0x40 + 32 * count;
    let mut names = Vec::new();
    let mut name_offsets = Vec::new();
    for i in 0..count {
        name_offsets.push(names_at + names.len());
        for u in format!("USERDATA{i:02}").encode_utf16().chain([0]) {
            names.extend_from_slice(&u.to_le_bytes());
        }
    }
    let data_at = (names_at + names.len()).next_multiple_of(16);
    let blobs: Vec<Vec<u8>> = datas
        .iter()
        .enumerate()
        .map(|(i, d)| encrypt(edition.key(), &[i as u8 + 1; 16], &wrap_payload(d)))
        .collect();

    let mut out = vec![0u8; data_at];
    out[..4].copy_from_slice(b"BND4");
    put_u32(&mut out, 0x0C, count as u32);
    put_u32(&mut out, 0x10, 0x40);
    out[0x18..0x20].copy_from_slice(b"14e503cb");
    put_u32(&mut out, 0x20, 32);
    put_u32(&mut out, 0x28, data_at as u32);
    out[0x30] = 1;
    out[0x31] = 0x20;
    let mut offset = data_at;
    for (i, blob) in blobs.iter().enumerate() {
        let h = 0x40 + 32 * i;
        out[h] = 0x50;
        put_u32(&mut out, h + 4, u32::MAX);
        put_u32(&mut out, h + 8, blob.len() as u32);
        put_u32(&mut out, h + 16, offset as u32);
        put_u32(&mut out, h + 20, name_offsets[i] as u32);
        offset += blob.len();
    }
    out[names_at..names_at + names.len()].copy_from_slice(&names);
    for blob in blobs {
        out.extend_from_slice(&blob);
    }
    out
}
