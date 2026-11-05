//! UnrealPakDirectory.cs: the full directory index (every name and where its entry is encoded) and one encoded entry.

use super::encoded::Encoded;
use super::index;
use super::le;
use super::name::Name;

pub const SHORT: &str = "the directory index is cut short";
pub const BAD_COUNT: &str = "the directory index has a count below zero";

pub fn read(index: &[u8]) -> Result<Vec<Name>, &'static str> {
    if index.len() < 4 {
        return Err(SHORT);
    }
    let mut p = 0usize;
    let directories = le::i32_at(index, p);
    p += 4;
    if directories < 0 {
        return Err(BAD_COUNT);
    }

    let mut found = Vec::new();
    for _ in 0..directories {
        let directory = index::string(index, &mut p)?;
        if p + 4 > index.len() {
            return Err(SHORT);
        }
        let files = le::i32_at(index, p);
        p += 4;
        if files < 0 {
            return Err(BAD_COUNT);
        }

        for _ in 0..files {
            let name = index::string(index, &mut p)?;
            if p + 4 > index.len() {
                return Err(SHORT);
            }
            found.push(Name { path: join(&directory, &name), at: le::i32_at(index, p) });
            p += 4;
        }
    }

    Ok(found)
}

pub fn join(directory: &str, name: &str) -> String {
    let d = directory.replace('\\', "/");
    let mut d = d.trim_start_matches('/').to_string();
    if !d.is_empty() && !d.ends_with('/') {
        d.push('/');
    }
    d + name
}

pub fn entry(blob: &[u8], at: i32) -> Result<Encoded, &'static str> {
    if at < 0 || at as usize + 4 > blob.len() {
        return Err(SHORT);
    }
    let mut p = at as usize;
    let value = le::u32_at(blob, p);
    p += 4;

    let block_size = if value & 0x3F == 0x3F {
        if p + 4 > blob.len() {
            return Err(SHORT);
        }
        let size = le::u32_at(blob, p) as i64;
        p += 4;
        size
    } else {
        ((value & 0x3F) as i64) << 11
    };

    let method = (value >> 23) & 0x3F;
    let offset = number(blob, &mut p, value & (1 << 31) != 0).ok_or(SHORT)?;
    let size = number(blob, &mut p, value & (1 << 30) != 0).ok_or(SHORT)?;
    let stored = if method != 0 { number(blob, &mut p, value & (1 << 29) != 0).ok_or(SHORT)? } else { size };
    let blocks = ((value >> 6) & 0xFFFF) as i32;

    Ok(Encoded {
        offset,
        size,
        stored,
        method,
        encrypted: value & (1 << 22) != 0,
        blocks,
        block_size: if blocks == 1 { size } else { block_size },
    })
}

fn number(b: &[u8], p: &mut usize, small: bool) -> Option<i64> {
    if small {
        if *p + 4 > b.len() {
            return None;
        }
        let v = le::u32_at(b, *p) as i64;
        *p += 4;
        return Some(v);
    }
    if *p + 8 > b.len() {
        return None;
    }
    let v = le::i64_at(b, *p);
    *p += 8;
    Some(v)
}
