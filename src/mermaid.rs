//! MermaidChunk.cs: a Mermaid or Selkie LZ chunk: literal and command arrays, near distances, far values per half and the length bytes.

use crate::coded_array::CodedArray;
use crate::growing;
use crate::lz_steps;
use crate::mermaid_commands::{Buffers, MermaidCommands};
use crate::refusal;

pub const HALF_SIZE: i32 = 0x10000;
const MIN_PAYLOAD: i32 = 10;
const ESCAPE: i32 = 4095;

pub struct MermaidChunk {
    run: MermaidCommands,
    literals: Vec<u8>,
    commands: Vec<u8>,
    high: Vec<u8>,
    low: Vec<u8>,
    near: Vec<i32>,
    first_far: Vec<i32>,
    second_far: Vec<i32>,
}

impl MermaidChunk {
    pub fn new() -> Self {
        MermaidChunk {
            run: MermaidCommands::new(),
            literals: Vec::new(),
            commands: Vec::new(),
            high: Vec::new(),
            low: Vec::new(),
            near: Vec::new(),
            first_far: Vec::new(),
            second_far: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, arrays: &mut CodedArray, src: &[u8], start: i32, end: i32, output: &mut [u8], chunk_start: i32, chunk_length: i32, mode: i32, window: i32) -> i32 {
        if mode > 1 || end - start < MIN_PAYLOAD {
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

        let (mut literal_count, mut command_count, mut near_count) = (0, 0, 0);
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

        let mut first_half_commands = command_count;
        if chunk_length > HALF_SIZE {
            if end - at < 2 {
                return refusal::CORRUPT;
            }
            first_half_commands = read_le16(src, at);
            at += 2;
            if first_half_commands > command_count {
                return refusal::CORRUPT;
            }
        }

        used = self.read_near(arrays, src, at, end, chunk_length, &mut near_count);
        if used < 0 {
            return used;
        }
        at += used;

        let (mut first_far_count, mut second_far_count) = (0, 0);
        used = self.read_far(src, at, end, chunk_start - window, &mut first_far_count, &mut second_far_count);
        if used < 0 {
            return used;
        }
        at += used;
        let run = &mut self.run;
        run.start(window, mode == 0, literal_count, near_count, at, end);
        let buffers = Buffers { literals: &self.literals, commands: &self.commands, near: &self.near, source: src };

        let first_end = chunk_start + if chunk_length < HALF_SIZE { chunk_length } else { HALF_SIZE };
        let mut result = run.half(&buffers, output, p, chunk_start, first_end, 0, first_half_commands, &self.first_far, first_far_count);
        if result < 0 {
            return result;
        }
        if chunk_length > HALF_SIZE {
            result = run.half(&buffers, output, chunk_start + HALF_SIZE, chunk_start + HALF_SIZE, chunk_start + chunk_length, first_half_commands, command_count, &self.second_far, second_far_count);
            if result < 0 {
                return result;
            }
        }
        if run.used_up() {
            0
        } else {
            refusal::CORRUPT
        }
    }

    fn read_near(&mut self, arrays: &mut CodedArray, src: &[u8], at: i32, end: i32, chunk_length: i32, near_count: &mut i32) -> i32 {
        *near_count = 0;
        if end - at < 2 {
            return refusal::CORRUPT;
        }
        let count = read_le16(src, at);
        let mut p = at + 2;
        if count != 0xFFFF {
            if 2 * count > end - p {
                return refusal::CORRUPT;
            }
            growing::ints(&mut self.near, count);
            for i in 0..count {
                self.near[i as usize] = read_le16(src, p + 2 * i);
            }
            *near_count = count;
            return 2 + 2 * count;
        }

        let (mut high_count, mut low_count) = (0, 0);
        let mut used = arrays.decode_into(src, p, end, chunk_length / 2, &mut self.high, &mut high_count);
        if used < 0 {
            return used;
        }
        p += used;
        used = arrays.decode_into(src, p, end, chunk_length / 2, &mut self.low, &mut low_count);
        if used < 0 {
            return used;
        }
        p += used;
        if high_count != low_count {
            return refusal::CORRUPT;
        }
        growing::ints(&mut self.near, high_count);
        for i in 0..high_count as usize {
            self.near[i] = self.low[i] as i32 + 256 * self.high[i] as i32;
        }
        *near_count = high_count;
        p - at
    }

    fn read_far(&mut self, src: &[u8], at: i32, end: i32, first_limit: i32, first_count: &mut i32, second_count: &mut i32) -> i32 {
        *first_count = 0;
        *second_count = 0;
        if end - at < 3 {
            return refusal::CORRUPT;
        }
        let a = at as usize;
        let packed = src[a] as i32 | ((src[a + 1] as i32) << 8) | ((src[a + 2] as i32) << 16);
        let mut p = at + 3;
        if packed == 0 {
            return 3;
        }

        *first_count = packed >> 12;
        *second_count = packed & 0xFFF;
        if *first_count == ESCAPE {
            if end - p < 2 {
                return refusal::CORRUPT;
            }
            *first_count = read_le16(src, p);
            p += 2;
        }
        if *second_count == ESCAPE {
            if end - p < 2 {
                return refusal::CORRUPT;
            }
            *second_count = read_le16(src, p);
            p += 2;
        }

        growing::ints(&mut self.first_far, *first_count);
        p = read_far_values(src, p, end, first_limit, &mut self.first_far, *first_count);
        if p < 0 {
            return p;
        }
        growing::ints(&mut self.second_far, *second_count);
        let second_limit = (i32::MAX as i64).min(first_limit as i64 + HALF_SIZE as i64) as i32;
        p = read_far_values(src, p, end, second_limit, &mut self.second_far, *second_count);
        if p < 0 {
            p
        } else {
            p - at
        }
    }
}

fn read_far_values(src: &[u8], mut p: i32, end: i32, limit: i32, values: &mut [i32], count: i32) -> i32 {
    for i in 0..count as usize {
        if end - p < 3 {
            return refusal::CORRUPT;
        }
        let a = p as usize;
        let mut v = src[a] as i32 | ((src[a + 1] as i32) << 8) | ((src[a + 2] as i32) << 16);
        p += 3;
        if limit >= 0xBFFFFF && v >= 0xC00000 {
            if p >= end {
                return refusal::CORRUPT;
            }
            v += (src[p as usize] as i32) << 22;
            p += 1;
        }
        if v > limit {
            return refusal::CORRUPT;
        }
        values[i] = v;
    }
    p
}

fn read_le16(src: &[u8], at: i32) -> i32 {
    src[at as usize] as i32 | ((src[at as usize + 1] as i32) << 8)
}
