//! UnrealPakRecord.cs: the record in front of an entry's bytes, with its blocks.

use super::block::Block;
use super::le;

pub const FIXED_BYTES: usize = 48;

pub const SHORT: &str = "the entry record is cut short";
pub const TOO_MANY_BLOCKS: &str = "the entry record claims more blocks than its bytes hold";
pub const NEGATIVE_LENGTH: &str = "the entry record has a length below zero";
pub const BLOCKS_BACKWARDS: &str = "a block ends before it starts";

pub struct Record {
    pub stored: i64,
    pub unpacked: i64,
    pub method: u32,
    pub blocks: Vec<Block>,
    pub encrypted: bool,
    pub block_size: u32,
    pub data_at: i64,
}

pub fn read(bytes: &[u8], at: usize) -> Result<Record, &'static str> {
    let len = bytes.len();
    if at + FIXED_BYTES > len {
        return Err(SHORT);
    }

    let mut p = at + 8;
    let stored = le::i64_at(bytes, p);
    let unpacked = le::i64_at(bytes, p + 8);
    let method = le::u32_at(bytes, p + 16);
    p += 8 + 8 + 4 + 20;

    if stored < 0 || unpacked < 0 {
        return Err(NEGATIVE_LENGTH);
    }

    let mut blocks = Vec::new();
    if method != 0 {
        if p + 4 > len {
            return Err(SHORT);
        }
        let count = le::u32_at(bytes, p) as usize;
        p += 4;
        if count > (len - p) / 16 {
            return Err(TOO_MANY_BLOCKS);
        }
        blocks.reserve_exact(count);
        for _ in 0..count {
            blocks.push(Block { start: le::i64_at(bytes, p), end: le::i64_at(bytes, p + 8) });
            p += 16;
        }
    }

    if p + 5 > len {
        return Err(SHORT);
    }
    let encrypted = bytes[p] != 0;
    let block_size = le::u32_at(bytes, p + 1);
    p += 5;
    let data_at = p as i64;

    if !blocks.is_empty() && blocks[0].start < data_at {
        for b in blocks.iter_mut() {
            b.start = b.start.wrapping_add(at as i64);
            b.end = b.end.wrapping_add(at as i64);
        }
    }

    if blocks.iter().any(|b| b.end < b.start) {
        return Err(BLOCKS_BACKWARDS);
    }

    Ok(Record { stored, unpacked, method, blocks, encrypted, block_size, data_at })
}
