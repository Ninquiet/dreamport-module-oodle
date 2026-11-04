//! LzSteps.cs: the raw bytes at a window start, literal runs (raw or added to the byte one distance back), the distance check,
//! and matches that may overlap what they write.
//! Where the C# passes a span `chunk` (the output up to the chunk's end) and the whole output, this passes one slice that ends
//! at the chunk's end: the C# writes through `output` only inside the run, which its callers keep before the chunk's end.

use crate::refusal;

pub const SMALLEST_DISTANCE: i32 = 8;
const RAW_PREFIX: i32 = 8;
const SHORT_COPY: i32 = 4;
const PIECE: i32 = 8;
const MOST_PIECED: i32 = 32;

#[inline(always)]
fn read64(buffer: &[u8], at: i32) -> u64 {
    let a = at as usize;
    u64::from_le_bytes(buffer[a..a + 8].try_into().unwrap())
}

#[inline(always)]
fn write64(buffer: &mut [u8], at: i32, value: u64) {
    let a = at as usize;
    buffer[a..a + 8].copy_from_slice(&value.to_le_bytes());
}

/// TryCopyPieces with a source other than the destination: up to 32 bytes in pieces of 8, writing up to 7 bytes past the run.
#[inline(always)]
pub fn try_copy_pieces_from(source: &[u8], from: i32, destination: &mut [u8], at: i32, count: i32) -> bool {
    let rounded = (count + PIECE - 1) & -PIECE;
    if count > MOST_PIECED || from + rounded > source.len() as i32 || at + rounded > destination.len() as i32 {
        return false;
    }
    let mut i = 0;
    while i < rounded {
        write64(destination, at + i, read64(source, from + i));
        i += PIECE;
    }
    true
}

/// TryCopyPieces with the source inside the destination (a match): each piece is read whole before it is written.
#[inline(always)]
pub fn try_copy_pieces_within(buffer: &mut [u8], from: i32, at: i32, count: i32) -> bool {
    let rounded = (count + PIECE - 1) & -PIECE;
    let length = buffer.len() as i32;
    if count > MOST_PIECED || from + rounded > length || at + rounded > length {
        return false;
    }
    let mut i = 0;
    while i < rounded {
        let piece = read64(buffer, from + i);
        write64(buffer, at + i, piece);
        i += PIECE;
    }
    true
}

/// The chunk that starts at the window takes its first 8 bytes raw from the payload: bytes consumed (8 or 0), or the refusal.
pub fn take_raw_prefix(src: &[u8], at: i32, output: &mut [u8], chunk_start: i32, chunk_length: i32, window: i32) -> i32 {
    if chunk_start != window {
        return 0;
    }
    if chunk_length < RAW_PREFIX {
        return refusal::CORRUPT;
    }
    crate::decoder::copy(src, at, output, chunk_start, RAW_PREFIX);
    RAW_PREFIX
}

/// Copy16 from another buffer: 16 bytes as two pieces of 8. The caller makes sure both have room.
#[inline(always)]
pub fn copy16_from(source: &[u8], from: i32, destination: &mut [u8], at: i32) {
    write64(destination, at, read64(source, from));
    write64(destination, at + PIECE, read64(source, from + PIECE));
}

/// Copy16 inside one buffer: the second piece is read after the first is written.
#[inline(always)]
pub fn copy16_within(buffer: &mut [u8], from: i32, at: i32) {
    let first = read64(buffer, from);
    write64(buffer, at, first);
    let second = read64(buffer, from + PIECE);
    write64(buffer, at + PIECE, second);
}

/// Delta literals eight at a time: each output byte is its literal plus the byte one distance back, with no carry between bytes.
#[inline(always)]
pub fn try_add_pieces(literals: &[u8], from: i32, destination: &mut [u8], at: i32, distance: i32, count: i32) -> bool {
    const LOW7: u64 = 0x7F7F7F7F7F7F7F7F;
    const HIGH: u64 = 0x8080808080808080;
    let rounded = (count + PIECE - 1) & -PIECE;
    if distance < PIECE || from + rounded > literals.len() as i32 || at + rounded > destination.len() as i32 {
        return false;
    }
    let mut i = 0;
    while i < rounded {
        let a = read64(literals, from + i);
        let b = read64(destination, at + i - distance);
        write64(destination, at + i, ((a & LOW7).wrapping_add(b & LOW7)) ^ ((a ^ b) & HIGH));
        i += PIECE;
    }
    true
}

/// A literal run, by the cheapest copy that fits: 16 bytes at once for a raw run of up to 16, pieces of 8 for up to 32 or a delta run, otherwise byte by byte.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn literal_run(chunk: &mut [u8], at: i32, literals: &[u8], from: i32, count: i32, delta: bool, distance: i32) {
    if !delta && count <= 16 && chunk.len() as i32 - at >= 16 && literals.len() as i32 - from >= 16 {
        copy16_from(literals, from, chunk, at);
        return;
    }
    let pieced = if delta { try_add_pieces(literals, from, chunk, at, distance, count) } else { try_copy_pieces_from(literals, from, chunk, at, count) };
    if !pieced {
        literals_to(chunk, at, count, literals, from, delta, distance);
    }
}

/// A match, by the cheapest copy that fits: at a distance of 8 or more, 16 bytes at once for up to 16, pieces of 8 for up to 32; otherwise match_copy.
#[inline(always)]
pub fn match_run(chunk: &mut [u8], at: i32, distance: i32, count: i32) {
    if distance >= PIECE {
        if count <= 16 && chunk.len() as i32 - at >= 16 {
            copy16_within(chunk, at - distance, at);
            return;
        }
        if try_copy_pieces_within(chunk, at - distance, at, count) {
            return;
        }
    }
    match_copy(chunk, at, distance, count);
}

#[inline(always)]
pub fn literal(output: &mut [u8], at: i32, value: u8, delta: bool, distance: i32) {
    output[at as usize] = if delta { value.wrapping_add(output[(at - distance) as usize]) } else { value };
}

/// LzSteps.Literals.
#[allow(clippy::too_many_arguments)]
pub fn literals_to(output: &mut [u8], at: i32, count: i32, literals: &[u8], from: i32, delta: bool, distance: i32) {
    if !delta {
        if count > SHORT_COPY {
            crate::decoder::copy(literals, from, output, at, count);
        } else {
            for i in 0..count {
                output[(at + i) as usize] = literals[(from + i) as usize];
            }
        }
        return;
    }
    for i in 0..count {
        literal(output, at + i, literals[(from + i) as usize], true, distance);
    }
}

/// Zero when the match may be copied, or the refusal: a copy may not start before the window. Distances 1 to 7 are refused with their own code unless allowed.
#[inline(always)]
pub fn check_distance(distance: i32, at: i32, window: i32, below_eight_allowed: bool) -> i32 {
    if distance <= 0 || distance > at - window {
        return refusal::CORRUPT;
    }
    if distance < SMALLEST_DISTANCE && !below_eight_allowed {
        refusal::DISTANCE_BELOW_EIGHT
    } else {
        0
    }
}

/// LzSteps.Match: copies count bytes from distance back. When they overlap, each copy takes the whole repeated period written so far, which doubles every time.
pub fn match_copy(output: &mut [u8], at: i32, distance: i32, count: i32) {
    let from = at - distance;
    if count <= SHORT_COPY {
        for i in 0..count {
            output[(at + i) as usize] = output[(from + i) as usize];
        }
        return;
    }
    if distance >= count {
        copy_within(output, from, at, count);
        return;
    }
    let mut copied = 0;
    while copied < count {
        let n = (count - copied).min(distance + copied);
        copy_within(output, from, at + copied, n);
        copied += n;
    }
}

#[inline(always)]
fn copy_within(buffer: &mut [u8], from: i32, at: i32, count: i32) {
    buffer.copy_within(from as usize..(from + count) as usize, at as usize);
}
