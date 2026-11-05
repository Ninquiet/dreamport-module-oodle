//! PakRecordHead.cs: an entry's own record, held to what the pak's index declared.

use super::encoded::Encoded;
use super::entry;
use super::record::{self, Record};
use super::unpack;
use super::Failed;
use crate::host::Container;

pub fn read(file: &Container, encoded: &Encoded) -> Result<Record, Failed> {
    let head = file.at(encoded.offset, room(encoded))?.ok_or(record::SHORT)?;
    let record = record::read(&head, 0)?;

    if record.unpacked > encoded.size || (record.method == entry::STORED && record.stored > encoded.size) {
        return Err(unpack::LARGER.into());
    }
    Ok(record)
}

fn room(encoded: &Encoded) -> i64 {
    let blocks = encoded.blocks.max(0) as i64;
    (record::FIXED_BYTES as i64 + 4 + blocks * 16 + 5).min(i32::MAX as i64)
}
