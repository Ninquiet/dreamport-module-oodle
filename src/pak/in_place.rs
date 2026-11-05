//! PakInPlace.cs: where one entry's bytes lie in the pak, from its own record: kept as they lie, or blocks one
//! after another, named by the decoder that reads them back. Nothing is decoded here.

use super::encoded::Encoded;
use super::entry;
use super::footer::Footer;
use super::record::Record;
use super::record_head;
use super::unpack;
use super::Failed;
use crate::host::Container;

pub const GAPS: &str = "an entry's blocks do not follow one another, so it cannot be read in place";
pub const OUTSIDE: &str = "an entry's bytes do not lie inside the pak";
pub const BLOCK_TOO_BIG: &str = "an entry's block is larger than one read";

/// DreamportBlocks.Stored and DreamportDecoders' names.
pub const STORED: &str = "stored";
pub const ZLIB: &str = "zlib";
pub const OODLE: &str = "oodle";

/// DreamportRange's fields, as a reader declares them.
#[derive(Default)]
pub struct Range {
    pub offset: i64,
    pub length: i64,
    pub method: i32,
    pub size: i64,
    pub decoder: &'static str,
    pub block_size: i32,
    pub blocks: Vec<i32>,
}

pub fn locate(file: &Container, file_length: i64, footer: &Footer, encoded: &Encoded) -> Result<Range, Failed> {
    let record = record_head::read(file, encoded)?;
    if record.encrypted {
        return Err(unpack::ENCRYPTED.into());
    }

    let decoder = decoder(footer, record.method).ok_or(entry::UNKNOWN_METHOD)?;
    let mut range = Range { method: record.method as i32, decoder, ..Range::default() };
    if decoder == STORED {
        as_it_lies(&mut range, &record, encoded.offset);
    } else {
        in_blocks(&mut range, &record, encoded.offset)?;
    }

    if range.offset < 0 || range.offset > file_length.wrapping_sub(range.length) {
        return Err(OUTSIDE.into());
    }
    Ok(range)
}

fn decoder(footer: &Footer, method: u32) -> Option<&'static str> {
    if method == entry::STORED {
        return Some(STORED);
    }
    if method == footer.method_for("Zlib") {
        return Some(ZLIB);
    }
    if method == footer.method_for("Oodle") {
        return Some(OODLE);
    }
    None
}

fn as_it_lies(range: &mut Range, record: &Record, at: i64) {
    range.offset = at.wrapping_add(record.data_at);
    range.length = record.stored;
    range.size = record.stored;
}

/// Each block where the extracting reader reads it: the record's offsets, from the entry's own start.
fn in_blocks(range: &mut Range, record: &Record, at: i64) -> Result<(), &'static str> {
    let blocks = &record.blocks;
    if record.block_size > i32::MAX as u32 {
        return Err(BLOCK_TOO_BIG);
    }

    range.offset = at.wrapping_add(if blocks.is_empty() { record.data_at } else { blocks[0].start });
    range.size = record.unpacked;
    range.block_size = record.block_size as i32;
    range.blocks = Vec::with_capacity(blocks.len());

    for (i, block) in blocks.iter().enumerate() {
        if i > 0 && block.start != blocks[i - 1].end {
            return Err(GAPS);
        }
        let length = block.length();
        if length <= 0 || length > i32::MAX as i64 {
            return Err(OUTSIDE);
        }
        range.blocks.push(length as i32);
        range.length = range.length.wrapping_add(length);
    }
    Ok(())
}
