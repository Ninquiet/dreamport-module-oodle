//! UnrealPakIndex.cs: the primary index (version 10 and later), its mount point, sub-indexes and encoded entries.

use super::footer::ascii;
use super::le;

pub const SHORT: &str = "the index is cut short";
pub const BAD_MOUNT: &str = "the index's mount point is not a string this build can read";
pub const BAD_COUNT: &str = "the index claims more entries than it could hold";
pub const NO_NAMES: &str = "this pak carries no directory index, so it has no file names";
pub const OUTSIDE: &str = "the index points a sub-index outside the file";
pub const BAD_ENCODED: &str = "the index gives a length below zero for its entries";
pub const OLD_ENTRIES: &str = "this pak keeps some entries in a form this reader does not read";

pub const OLDEST: u32 = 10;

/// The start of `Older(version)`; the version number cannot cross the ABI, so the start stands for it.
pub const OLDER_PAK: &str = "this pak's index is an older format than this reader opens";

pub struct Index {
    pub mount_point: String,
    pub has_path_hash_index: bool,
    pub path_hash_index_offset: i64,
    pub full_directory_index_offset: i64,
    pub full_directory_index_size: i64,
    pub encoded: Vec<u8>,
}

pub fn read(index: &[u8], file_length: i64, version: u32) -> Result<Index, &'static str> {
    if version < OLDEST {
        return Err(OLDER_PAK);
    }
    if index.len() < 4 {
        return Err(SHORT);
    }

    let mut p = 0usize;
    let mount_point = string(index, &mut p)?;

    if !room(index, p, 4 + 8 + 4) {
        return Err(SHORT);
    }
    let entry_count = le::i32_at(index, p);
    p += 4 + 8;
    if entry_count < 0 {
        return Err(BAD_COUNT);
    }

    let has_path_hash_index = le::i32_at(index, p) != 0;
    p += 4;
    let (mut path_hash_index_offset, mut path_hash_index_size) = (0, 0);
    if has_path_hash_index {
        if !room(index, p, 8 + 8 + 20) {
            return Err(SHORT);
        }
        path_hash_index_offset = le::i64_at(index, p);
        path_hash_index_size = le::i64_at(index, p + 8);
        p += 8 + 8 + 20;
    }

    if !room(index, p, 4) {
        return Err(SHORT);
    }
    let has_full_directory_index = le::i32_at(index, p) != 0;
    p += 4;
    if !has_full_directory_index {
        return Err(NO_NAMES);
    }
    if !room(index, p, 8 + 8 + 20) {
        return Err(SHORT);
    }
    let full_directory_index_offset = le::i64_at(index, p);
    let full_directory_index_size = le::i64_at(index, p + 8);
    p += 8 + 8 + 20;

    if !room(index, p, 4) {
        return Err(SHORT);
    }
    let encoded = le::i32_at(index, p);
    p += 4;
    if encoded < 0 {
        return Err(BAD_ENCODED);
    }
    let encoded = encoded as usize;
    if !room(index, p, encoded) {
        return Err(SHORT);
    }
    let encoded_bytes = index[p..p + encoded].to_vec();
    p += encoded;

    if !room(index, p, 4) {
        return Err(SHORT);
    }
    if le::i32_at(index, p) != 0 {
        return Err(OLD_ENTRIES);
    }

    if outside(full_directory_index_offset, full_directory_index_size, file_length)
        || (has_path_hash_index && outside(path_hash_index_offset, path_hash_index_size, file_length))
    {
        return Err(OUTSIDE);
    }

    Ok(Index {
        mount_point,
        has_path_hash_index,
        path_hash_index_offset,
        full_directory_index_offset,
        full_directory_index_size,
        encoded: encoded_bytes,
    })
}

fn outside(at: i64, size: i64, file_length: i64) -> bool {
    at < 0 || size < 0 || at.wrapping_add(size) > file_length
}

fn room(b: &[u8], at: usize, want: usize) -> bool {
    at as u64 + want as u64 <= b.len() as u64
}

/// Unreal's string: a length, then ASCII bytes when positive or UTF-16 when negative, trailing zeros dropped.
pub fn string(b: &[u8], at: &mut usize) -> Result<String, &'static str> {
    if !room(b, *at, 4) {
        return Err(SHORT);
    }
    let length = le::i32_at(b, *at);
    *at += 4;

    if length == 0 {
        return Ok(String::new());
    }

    if length > 0 {
        let length = length as usize;
        if !room(b, *at, length) {
            return Err(SHORT);
        }
        let mut end = length;
        while end > 0 && b[*at + end - 1] == 0 {
            end -= 1;
        }
        let made = ascii(&b[*at..*at + end]);
        *at += length;
        return Ok(made);
    }

    let chars = -(length as i64);
    if chars > (i32::MAX / 2) as i64 || !room(b, *at, chars as usize * 2) {
        return Err(BAD_MOUNT);
    }
    let chars = chars as usize;
    let mut stop = chars;
    while stop > 0 && b[*at + (stop - 1) * 2] == 0 && b[*at + (stop - 1) * 2 + 1] == 0 {
        stop -= 1;
    }
    let units: Vec<u16> = (0..stop).map(|i| u16::from_le_bytes([b[*at + i * 2], b[*at + i * 2 + 1]])).collect();
    *at += chars * 2;
    Ok(String::from_utf16_lossy(&units))
}
