//! LeviathanCommands.cs: seven recent distances, the length list taken from both ends, six literal modes.

use crate::lz_steps;
use crate::lz_tail::LzTail;
use crate::refusal;

const RECENT: usize = 7;
pub const MOST_ARRAYS: usize = 16;

pub struct LeviathanCommands {
    chunk_start: i32,
    chunk_end: i32,
    mode: i32,
    literal_arrays: i32,
    command_arrays: i32,
    recent: [i32; RECENT],
    last: i32,
    window: i32,
    delta: bool,
    literal_at: [i32; MOST_ARRAYS + 1],
    command_at: [i32; MOST_ARRAYS + 1],
}

impl LeviathanCommands {
    pub fn new() -> Self {
        LeviathanCommands {
            chunk_start: 0,
            chunk_end: 0,
            mode: 0,
            literal_arrays: 0,
            command_arrays: 0,
            recent: [0; RECENT],
            last: 0,
            window: 0,
            delta: false,
            literal_at: [0; MOST_ARRAYS + 1],
            command_at: [0; MOST_ARRAYS + 1],
        }
    }

    /// Starts a chunk; the bounds passed to run give where array k starts in its buffer, bounds[count] where the last one ends.
    pub fn start(&mut self, window: i32, chunk_start: i32, chunk_end: i32, mode: i32, literal_arrays: i32, command_arrays: i32) {
        self.window = window;
        self.chunk_start = chunk_start;
        self.chunk_end = chunk_end;
        self.mode = mode;
        self.delta = mode != 1 && mode != 4;
        self.literal_arrays = literal_arrays;
        self.command_arrays = command_arrays;
        self.recent = [8; RECENT];
        self.last = 8;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run(&mut self, output: &mut [u8], mut p: i32, tail: &LzTail, literals: &[u8], literal_bounds: &[i32], commands: &[u8], command_bounds: &[i32]) -> i32 {
        let literal_arrays = self.literal_arrays as usize;
        self.literal_at[..literal_arrays + 1].copy_from_slice(&literal_bounds[..literal_arrays + 1]);
        let arrays = self.command_arrays;
        self.command_at[..arrays as usize + 1].copy_from_slice(&command_bounds[..arrays as usize + 1]);
        let total = command_bounds[arrays as usize];
        let lengths = &tail.lengths;
        let distances = &tail.distances;
        let distance_count = tail.distance_count;
        let (mut front, mut back, mut distance_at) = (0i32, tail.length_count, 0i32);
        let chunk_end = self.chunk_end;
        let chunk = &mut output[..chunk_end as usize];
        let (chunk_start, window) = (self.chunk_start, self.window);
        let mut last = self.last;
        let one_literal_array = self.literal_arrays == 1;
        let delta = self.delta;
        let mut literal_at = self.literal_at[0];
        let literal_end = literal_bounds[1];

        for _ in 0..total {
            let j = if arrays == 1 { 0 } else { ((p - chunk_start) & 7) as usize };
            let at = self.command_at[j];
            if at >= command_bounds[j + 1] {
                return refusal::CORRUPT;
            }
            let c = commands[at as usize] as i32;
            self.command_at[j] = at + 1;

            let mut literal_run = (c >> 3) & 3;
            if literal_run == 3 {
                if front >= back {
                    return refusal::CORRUPT;
                }
                literal_run = lengths[front as usize];
                front += 1;
            }
            let mut match_length = c & 7;
            if match_length == 7 {
                if front >= back {
                    return refusal::CORRUPT;
                }
                back -= 1;
                match_length = 6 + lengths[back as usize];
            } else {
                match_length += 2;
            }

            if one_literal_array {
                if literal_run > chunk_end - p || literal_run > literal_end - literal_at {
                    return refusal::CORRUPT;
                }
                lz_steps::literal_run(chunk, p, literals, literal_at, literal_run, delta, last);
                literal_at += literal_run;
                p += literal_run;
            } else {
                self.last = last;
                let written = self.write_literals(chunk, literals, literal_bounds, &mut p, literal_run);
                if written < 0 {
                    return written;
                }
            }

            let slot = (c >> 5) as usize;
            let distance;
            let recent = &mut self.recent;
            if slot < RECENT {
                distance = recent[slot];
                for k in (1..=slot).rev() {
                    recent[k] = recent[k - 1];
                }
            } else {
                if distance_at >= distance_count {
                    return refusal::CORRUPT;
                }
                distance = distances[distance_at as usize];
                distance_at += 1;
                for k in (1..RECENT).rev() {
                    recent[k] = recent[k - 1];
                }
            }
            recent[0] = distance;
            last = distance;

            let refused = lz_steps::check_distance(distance, p, window, true);
            if refused < 0 {
                return refused;
            }
            if match_length > chunk_end - p {
                return refusal::CORRUPT;
            }
            lz_steps::match_run(chunk, p, distance, match_length);
            p += match_length;
        }

        if distance_at != distance_count || front != back {
            return refusal::CORRUPT;
        }
        if one_literal_array {
            self.literal_at[0] = literal_at;
        }
        self.last = last;
        let rest = chunk_end - p;
        self.write_literals(chunk, literals, literal_bounds, &mut p, rest)
    }

    fn write_literals(&mut self, output: &mut [u8], literals: &[u8], literal_bounds: &[i32], p: &mut i32, count: i32) -> i32 {
        if count > self.chunk_end - *p {
            return refusal::CORRUPT;
        }
        if self.literal_arrays == 1 {
            let from = self.literal_at[0];
            if count > literal_bounds[1] - from {
                return refusal::CORRUPT;
            }
            lz_steps::literals_to(output, *p, count, literals, from, self.delta, self.last);
            self.literal_at[0] = from + count;
            *p += count;
            return 0;
        }
        for i in 0..count {
            let array = self.array_for(output, *p, i);
            if self.literal_at[array] >= literal_bounds[array + 1] {
                return refusal::CORRUPT;
            }
            let value = literals[self.literal_at[array] as usize];
            self.literal_at[array] += 1;
            lz_steps::literal(output, *p, value, self.delta, self.last);
            *p += 1;
        }
        0
    }

    fn array_for(&self, output: &[u8], p: i32, index_in_run: i32) -> usize {
        match self.mode {
            2 => {
                if index_in_run == 0 {
                    1
                } else {
                    0
                }
            }
            3 => ((p - self.chunk_start) & 3) as usize,
            4 => (output[(p - 1) as usize] >> 4) as usize,
            5 => ((p - self.chunk_start) & 15) as usize,
            _ => 0,
        }
    }
}
