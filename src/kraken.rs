//! KrakenChunk.cs: literals, commands, offsets and lengths as coded arrays, the bit-packed tail, then the commands run.

use crate::coded_array::CodedArray;
use crate::lz_steps;
use crate::lz_tail::{LzTail, NO_REGION};
use crate::refusal;

const MIN_PAYLOAD: i32 = 13;
const LARGEST_REGION: i32 = 459;

pub struct KrakenChunk {
    tail: LzTail,
    literals: Vec<u8>,
    commands: Vec<u8>,
}

impl KrakenChunk {
    pub fn new() -> Self {
        KrakenChunk { tail: LzTail::new(), literals: Vec::new(), commands: Vec::new() }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, arrays: &mut CodedArray, src: &[u8], start: i32, end: i32, output: &mut [u8], chunk_start: i32, chunk_length: i32, mode: i32, window: i32) -> i32 {
        if mode > 1 {
            return refusal::CORRUPT;
        }
        if end - start < MIN_PAYLOAD {
            return refusal::CORRUPT;
        }

        let mut at = start;
        let mut p = chunk_start;
        let prefix = lz_steps::take_raw_prefix(src, at, output, chunk_start, chunk_length, window);
        if prefix < 0 {
            return prefix;
        }
        at += prefix;
        p += prefix;
        let mut region_length = NO_REGION;
        if (src[at as usize] & 0x80) != 0 {
            let flag = read_flag(src, &mut at, end, &mut region_length);
            if flag < 0 {
                return flag;
            }
        }

        let (mut literal_count, mut command_count) = (0, 0);
        let mut used = arrays.decode_into(src, at, end, chunk_length, &mut self.literals, &mut literal_count);
        if used < 0 {
            return used;
        }
        at += used;
        used = arrays.decode_into(src, at, end, chunk_length, &mut self.commands, &mut command_count);
        if used < 0 {
            return used;
        }
        at += used;

        if end - at < 3 {
            return refusal::CORRUPT;
        }
        used = self.tail.read_arrays(arrays, src, at, end, command_count, chunk_length / 4);
        if used < 0 {
            return used;
        }
        at += used;

        if region_length > end - at {
            return refusal::CORRUPT;
        }
        let tail = self.tail.read(src, at, end, region_length);
        if tail < 0 {
            return tail;
        }

        run(output, p, window, chunk_start + chunk_length, mode == 0, &self.literals, literal_count, &self.commands, command_count, &self.tail)
    }
}

/// The flag announcing a long-length region of K bytes at the payload's end: 10Xkkkkk, and when X is set a second byte g, K = 32 + k + 32 g; no valid chunk has K above 459.
fn read_flag(src: &[u8], at: &mut i32, end: i32, region_length: &mut i32) -> i32 {
    *region_length = NO_REGION;
    let first = src[*at as usize] as i32;
    *at += 1;
    if (first & 0x40) != 0 {
        return refusal::CORRUPT;
    }
    let k = first & 0x1F;
    if (first & 0x20) == 0 {
        *region_length = k;
        return 0;
    }
    if *at >= end {
        return refusal::CORRUPT;
    }
    *region_length = 32 + k + 32 * src[*at as usize] as i32;
    *at += 1;
    if *region_length > LARGEST_REGION {
        refusal::CORRUPT
    } else {
        0
    }
}

#[allow(clippy::too_many_arguments)]
fn run(output: &mut [u8], mut p: i32, window: i32, chunk_end: i32, delta: bool, literals: &[u8], literal_count: i32, commands: &[u8], command_count: i32, tail: &LzTail) -> i32 {
    let chunk = &mut output[..chunk_end as usize];
    let (mut r0, mut r1, mut r2, mut last) = (8i32, 8i32, 8i32, 8i32);
    let (mut literal_at, mut distance_at, mut length_at) = (0i32, 0i32, 0i32);
    let lengths = &tail.lengths;
    let distances = &tail.distances;
    let (length_count, distance_count) = (tail.length_count, tail.distance_count);
    let literal_room = literals.len() as i32;

    for i in 0..command_count as usize {
        let command = commands[i] as i32;
        let mut literal_run = command & 3;
        if literal_run == 3 {
            if length_at >= length_count {
                return refusal::CORRUPT;
            }
            literal_run = lengths[length_at as usize];
            length_at += 1;
        }
        let mut match_length = (command >> 2) & 15;
        if match_length == 15 {
            if length_at >= length_count {
                return refusal::CORRUPT;
            }
            match_length = 14 + lengths[length_at as usize];
            length_at += 1;
        } else {
            match_length += 2;
        }

        if literal_run > literal_count - literal_at || literal_run > chunk_end - p {
            return refusal::CORRUPT;
        }
        if !delta && literal_run <= 16 && chunk_end - p >= 16 && literal_room - literal_at >= 16 {
            lz_steps::copy16_from(literals, literal_at, chunk, p);
        } else {
            let pieced = if delta {
                lz_steps::try_add_pieces(literals, literal_at, chunk, p, last, literal_run)
            } else {
                lz_steps::try_copy_pieces_from(literals, literal_at, chunk, p, literal_run)
            };
            if !pieced {
                lz_steps::literals_to(chunk, p, literal_run, literals, literal_at, delta, last);
            }
        }
        literal_at += literal_run;
        p += literal_run;

        let distance;
        match command >> 6 {
            0 => distance = r0,
            1 => {
                distance = r1;
                r1 = r0;
                r0 = distance;
            }
            2 => {
                distance = r2;
                r2 = r1;
                r1 = r0;
                r0 = distance;
            }
            _ => {
                if distance_at >= distance_count {
                    return refusal::CORRUPT;
                }
                distance = distances[distance_at as usize];
                distance_at += 1;
                r2 = r1;
                r1 = r0;
                r0 = distance;
            }
        }
        last = distance;

        let refused = lz_steps::check_distance(distance, p, window, false);
        if refused < 0 {
            return refused;
        }
        if match_length > chunk_end - p {
            return refusal::CORRUPT;
        }
        if match_length <= 16 && chunk_end - p >= 16 {
            lz_steps::copy16_within(chunk, p - distance, p);
        } else if !lz_steps::try_copy_pieces_within(chunk, p - distance, p, match_length) {
            lz_steps::match_copy(chunk, p, distance, match_length);
        }
        p += match_length;
    }

    if distance_at != distance_count || length_at != length_count {
        return refusal::CORRUPT;
    }
    let trailing = chunk_end - p;
    if trailing != literal_count - literal_at {
        return refusal::CORRUPT;
    }
    lz_steps::literals_to(chunk, p, trailing, literals, literal_at, delta, last);
    0
}
