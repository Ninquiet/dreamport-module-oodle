//! MultiArray.cs: several output arrays built by copying pieces of source arrays, an index and a length per piece.

use crate::coded_array::{CodedArray, Header, WORKING_LIMIT};
use crate::growing;
use crate::msb_bits::MsbBits;
use crate::refusal;

const MAX_BIT_COUNT: u8 = 16;
const MAX_SOURCES: usize = 0x3F;

pub struct Work {
    sources: Vec<u8>,
    source_end: [i32; MAX_SOURCES + 1],
    source_at: [i32; MAX_SOURCES],
    entries: Vec<u8>,
    widths: Vec<u8>,
    pieces: Vec<i32>,
}

impl Default for Work {
    fn default() -> Self {
        Work { sources: Vec::new(), source_end: [0; MAX_SOURCES + 1], source_at: [0; MAX_SOURCES], entries: Vec::new(), widths: Vec::new(), pieces: Vec::new() }
    }
}

/// Stream bytes used, or a refusal; bounds[k] is where output array k starts, relative to dst_at, and bounds[arrays] the total.
#[allow(clippy::too_many_arguments)]
pub fn decode(coded: &mut CodedArray, src: &[u8], at: i32, end: i32, arrays: i32, dst: &mut [u8], dst_at: i32, room: i32, bounds: &mut [i32], depth: i32) -> i32 {
    if end - at < 4 || (src[at as usize] & 0x80) == 0 {
        return refusal::CORRUPT;
    }
    let source_count = (src[at as usize] & 0x3F) as i32;
    let mut p = at + 1;
    let room = room.min(dst.len() as i32 - dst_at);

    if source_count == 0 {
        let mut written = 0i32;
        for k in 0..arrays as usize {
            bounds[k] = written;
            let mut decoded = 0;
            let used = coded.decode(src, p, end, dst, dst_at + written, room - written, depth + 1, &mut decoded);
            if used < 0 {
                return used;
            }
            p += used;
            written += decoded;
        }
        bounds[arrays as usize] = written;
        return p - at;
    }

    let mut work = std::mem::take(&mut coded.multi_work[depth as usize]);
    let result = decode_sources(coded, &mut work, src, at, p, end, source_count, arrays, dst, dst_at, room, bounds, depth);
    coded.multi_work[depth as usize] = work;
    result
}

#[allow(clippy::too_many_arguments)]
fn decode_sources(coded: &mut CodedArray, work: &mut Work, src: &[u8], at: i32, mut p: i32, end: i32, source_count: i32, arrays: i32,
    dst: &mut [u8], dst_at: i32, room: i32, bounds: &mut [i32], depth: i32) -> i32 {
    work.source_end[0] = 0;
    let mut total = 0i32;
    for s in 0..source_count as usize {
        let mut decoded = 0;
        let source_room = WORKING_LIMIT - total;
        growing::grow(&mut work.sources, total + CodedArray::room_for(src, p, end, source_room), total);
        let used = coded.decode(src, p, end, &mut work.sources, total, source_room, depth + 1, &mut decoded);
        if used < 0 {
            return used;
        }
        p += used;
        total += decoded;
        work.source_end[s + 1] = total;
    }

    if end - p < 3 {
        return refusal::CORRUPT;
    }
    let control = src[p as usize] as i32 | ((src[p as usize + 1] as i32) << 8);
    p += 2;
    let packed = (control & 0x8000) != 0;

    let peek = match Header::try_read(src, p, end) {
        Some(h) => h,
        None => return refusal::CORRUPT,
    };
    let entry_count = peek.decoded_length;
    if entry_count > total || entry_count - arrays < 1 {
        return refusal::CORRUPT;
    }

    growing::bytes(&mut work.entries, entry_count);
    let mut step = coded.decode_exactly(src, p, end, &mut work.entries, 0, entry_count, depth + 1);
    if step < 0 {
        return step;
    }
    p += step;

    let value_count = if packed { entry_count } else { entry_count - arrays };
    growing::bytes(&mut work.widths, value_count);
    if packed {
        for i in 0..entry_count as usize {
            work.widths[i] = work.entries[i] >> 4;
            work.entries[i] &= 0xF;
        }
    } else {
        step = coded.decode_exactly(src, p, end, &mut work.widths, 0, value_count, depth + 1);
        if step < 0 {
            return step;
        }
        p += step;
        for i in 0..value_count as usize {
            if work.widths[i] > MAX_BIT_COUNT {
                return refusal::CORRUPT;
            }
        }
    }

    let region_length = control & 0x3FFF;
    if region_length > end - p {
        return refusal::CORRUPT;
    }
    growing::ints(&mut work.pieces, value_count);
    read_piece_lengths(src, p, p + region_length, &work.widths, &mut work.pieces, value_count);
    p += region_length;

    if work.entries[(entry_count - 1) as usize] != 0 {
        return refusal::CORRUPT;
    }
    let ok = assemble(work, entry_count, value_count, packed, source_count, arrays, dst, dst_at, room, bounds);
    if ok {
        p - at
    } else {
        refusal::CORRUPT
    }
}

fn read_piece_lengths(src: &[u8], start: i32, end: i32, widths: &[u8], pieces: &mut [i32], count: i32) {
    let mut forward = MsbBits::new(src, start, end, false);
    let mut backward = MsbBits::new(src, start, end, true);
    for j in 0..count as usize {
        let reader = if (j & 1) == 0 { &mut forward } else { &mut backward };
        let width = widths[j] as i32;
        pieces[j] = (1i32 << width) + reader.read(width) as i32;
    }
}

#[allow(clippy::too_many_arguments)]
fn assemble(work: &mut Work, entry_count: i32, piece_count: i32, packed: bool, source_count: i32, arrays: i32, dst: &mut [u8], dst_at: i32, room: i32,
    bounds: &mut [i32]) -> bool {
    let entries = &work.entries;
    let pieces = &work.pieces;
    let sources = &work.sources;
    let source_end = &work.source_end;
    let source_at = &mut work.source_at;
    for s in 0..source_count as usize {
        source_at[s] = source_end[s];
    }

    let (mut e, mut v, mut written) = (0i32, 0i32, 0i32);
    for k in 0..arrays as usize {
        bounds[k] = written;
        loop {
            if e >= entry_count {
                return false;
            }
            let index = entries[e as usize] as i32;
            e += 1;
            if index == 0 {
                if packed {
                    let was = v;
                    v += 1;
                    if was >= piece_count {
                        return false;
                    }
                }
                break;
            }
            if index > source_count || v >= piece_count {
                return false;
            }
            let piece = pieces[v as usize];
            v += 1;
            let s = (index - 1) as usize;
            if piece > source_end[s + 1] - source_at[s] || piece > room - written {
                return false;
            }
            crate::decoder::copy(sources, source_at[s], dst, dst_at + written, piece);
            source_at[s] += piece;
            written += piece;
        }
    }
    bounds[arrays as usize] = written;

    if e != entry_count || v != piece_count {
        return false;
    }
    for s in 0..source_count as usize {
        if source_at[s] != source_end[s + 1] {
            return false;
        }
    }
    true
}
