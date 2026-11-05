//! BitConverter's little-endian reads; the caller has checked the bytes are there.

pub fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub fn i32_at(b: &[u8], at: usize) -> i32 {
    u32_at(b, at) as i32
}

pub fn i64_at(b: &[u8], at: usize) -> i64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    i64::from_le_bytes(v)
}
