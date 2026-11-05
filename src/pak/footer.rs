//! UnrealPakFooter.cs: the footer at a pak's end, found by its magic in one of three lengths.

use super::le;
use super::Failed;
use crate::host::Container;

pub const MAGIC: u32 = 0x5A6F12E1;
pub const NAME_BYTES: usize = 32;

const NAMES_BEFORE_8: usize = 0;
const NAMES_FROM_8: usize = 4;
const NAMES_FROM_11: usize = 5;
const FIXED_BYTES: usize = 61;

const MAGIC_AT: usize = 17;
const ENCRYPTED_AT: usize = 16;
const VERSION_AT: usize = 21;
const INDEX_OFFSET_AT: usize = 25;
const INDEX_SIZE_AT: usize = 33;

pub const NOT_A_PAK: &str = "this file does not end like a pak";
pub const SHORT: &str = "the file is too short to hold a pak footer";
pub const BAD_INDEX: &str = "the footer points its index outside the file";

pub struct Footer {
    pub version: u32,
    pub index_offset: i64,
    pub index_size: i64,
    pub encrypted: bool,
    pub methods: Vec<String>,
}

pub fn read(file: &Container, file_length: i64) -> Result<Footer, Failed> {
    if file_length <= 0 || file_length < footer_bytes(NAMES_BEFORE_8) as i64 {
        return Err(SHORT.into());
    }

    for names in [NAMES_FROM_11, NAMES_FROM_8, NAMES_BEFORE_8] {
        let size = footer_bytes(names);
        let at = file_length - size as i64;
        if at < 0 {
            continue;
        }

        let tail = match file.at(at, size as i64)? {
            Some(tail) => tail,
            None => continue,
        };
        if le::u32_at(&tail, MAGIC_AT) != MAGIC {
            continue;
        }

        let footer = parse(&tail, names);
        if footer.index_offset < 0 || footer.index_size < 0 || footer.index_offset.wrapping_add(footer.index_size) > file_length {
            return Err(BAD_INDEX.into());
        }
        return Ok(footer);
    }

    Err(NOT_A_PAK.into())
}

fn footer_bytes(names: usize) -> usize {
    FIXED_BYTES + names * NAME_BYTES
}

fn parse(tail: &[u8], names: usize) -> Footer {
    let methods = (0..names).map(|i| name_at(tail, FIXED_BYTES + i * NAME_BYTES)).filter(|n| !n.is_empty()).collect();
    Footer {
        encrypted: tail[ENCRYPTED_AT] != 0,
        version: le::u32_at(tail, VERSION_AT),
        index_offset: le::i64_at(tail, INDEX_OFFSET_AT),
        index_size: le::i64_at(tail, INDEX_SIZE_AT),
        methods,
    }
}

fn name_at(tail: &[u8], at: usize) -> String {
    if at + NAME_BYTES > tail.len() {
        return String::new();
    }
    let slot = &tail[at..at + NAME_BYTES];
    let end = slot.iter().position(|&b| b == 0).unwrap_or(NAME_BYTES);
    ascii(&slot[..end]).trim().to_string()
}

/// Encoding.ASCII.GetString: a byte above 0x7F reads as '?'.
pub fn ascii(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| if b < 0x80 { b as char } else { '?' }).collect()
}

impl Footer {
    pub fn method_for(&self, name: &str) -> u32 {
        if name.is_empty() {
            return 0;
        }
        match self.methods.iter().position(|m| m.eq_ignore_ascii_case(name)) {
            Some(i) => (i + 1) as u32,
            None => 0,
        }
    }
}
