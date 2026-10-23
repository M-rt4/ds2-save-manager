//! Little-endian field access on decrypted save data. Reads past the end yield 0, so a
//! short or damaged block reads as empty instead of panicking; writes are bounds-checked
//! by the callers, which only write at offsets the layout check has already confirmed.

pub fn u8_at(b: &[u8], o: usize) -> u8 {
    b.get(o).copied().unwrap_or(0)
}

pub fn u16_at(b: &[u8], o: usize) -> u16 {
    b.get(o..o + 2).map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
}

pub fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

pub fn u64_at(b: &[u8], o: usize) -> u64 {
    b.get(o..o + 8).map_or(0, |s| {
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        u64::from_le_bytes(a)
    })
}

pub fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u32_at(b, o))
}

pub fn put_u16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn put_f32(b: &mut [u8], o: usize, v: f32) {
    put_u32(b, o, v.to_bits());
}

/// A NUL-terminated UTF-16LE string of at most `max_chars` characters.
pub fn utf16_at(b: &[u8], o: usize, max_chars: usize) -> String {
    let units: Vec<u16> = (0..max_chars)
        .map(|i| u16_at(b, o + i * 2))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Writes `s` as UTF-16LE into a field of `field_chars` characters, NUL-padding the rest.
pub fn put_utf16(b: &mut [u8], o: usize, field_chars: usize, s: &str) {
    let units: Vec<u16> = s.encode_utf16().take(field_chars).collect();
    for i in 0..field_chars {
        put_u16(b, o + i * 2, units.get(i).copied().unwrap_or(0));
    }
}
