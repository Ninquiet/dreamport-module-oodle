//! LeviathanChunk.cs: offsets, length bytes, one or several literal arrays, one or eight command arrays, then Kraken's tail.

use crate::coded_array::{CodedArray, WORKING_LIMIT};
use crate::growing;
use crate::leviathan_commands::{LeviathanCommands, MOST_ARRAYS};
use crate::lz_steps;
use crate::lz_tail::{LzTail, NO_REGION};
use crate::refusal;

const MIN_PAYLOAD: i32 = 13;
const EIGHT_COMMAND_ARRAYS: u8 = 0x83;
const LITERAL_ARRAYS: [i32; 6] = [1, 1, 2, 4, 16, 16];

pub struct LeviathanChunk {
    tail: LzTail,
    run: LeviathanCommands,
    literal_bounds: [i32; MOST_ARRAYS + 1],
    command_bounds: [i32; MOST_ARRAYS + 1],
    literals: Vec<u8>,
    commands: Vec<u8>,
}

impl LeviathanChunk {
    pub fn new() -> Self {
        LeviathanChunk {
            tail: LzTail::new(),
            run: LeviathanCommands::new(),
            literal_bounds: [0; MOST_ARRAYS + 1],
            command_bounds: [0; MOST_ARRAYS + 1],
            literals: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, arrays: &mut CodedArray, src: &[u8], start: i32, end: i32, output: &mut [u8], chunk_start: i32, chunk_length: i32, mode: i32, window: i32) -> i32 {
        if mode > 5 || end - start < MIN_PAYLOAD {
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

        if at >= end {
            return refusal::CORRUPT;
        }
        let mut used = self.tail.read_arrays(arrays, src, at, end, chunk_length / 3, chunk_length / 5);
        if used < 0 {
            return used;
        }
        at += used;

        let literal_arrays = LITERAL_ARRAYS[mode as usize];
        used = read_arrays(arrays, src, at, end, literal_arrays, chunk_length, &mut self.literals, &mut self.literal_bounds);
        if used < 0 {
            return used;
        }
        at += used;

        if at >= end {
            return refusal::CORRUPT;
        }
        let mut command_arrays = 1;
        if src[at as usize] >= 0x80 {
            if src[at as usize] != EIGHT_COMMAND_ARRAYS {
                return refusal::CORRUPT;
            }
            at += 1;
            command_arrays = 8;
        }
        used = read_arrays(arrays, src, at, end, command_arrays, chunk_length, &mut self.commands, &mut self.command_bounds);
        if used < 0 {
            return used;
        }
        at += used;

        let tail = self.tail.read(src, at, end, NO_REGION);
        if tail < 0 {
            return tail;
        }
        let run = &mut self.run;
        run.start(window, chunk_start, chunk_start + chunk_length, mode, literal_arrays, command_arrays);
        run.run(output, p, &self.tail, &self.literals, &self.literal_bounds, &self.commands, &self.command_bounds)
    }
}

#[allow(clippy::too_many_arguments)]
fn read_arrays(arrays: &mut CodedArray, src: &[u8], at: i32, end: i32, count: i32, chunk_length: i32, buffer: &mut Vec<u8>, bounds: &mut [i32]) -> i32 {
    bounds[0] = 0;
    if count == 1 {
        let mut length = 0;
        let used = arrays.decode_into(src, at, end, chunk_length, buffer, &mut length);
        bounds[1] = length;
        return used;
    }
    growing::bytes(buffer, WORKING_LIMIT);
    arrays.multi_decode(src, at, end, count, buffer, 0, WORKING_LIMIT, bounds, 0)
}
