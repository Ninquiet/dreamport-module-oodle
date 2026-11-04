//! MermaidCommands.cs: runs a Mermaid chunk's commands half by half; the literal, near and length cursors and the recent distance carry from one half to the next.

use crate::lz_steps;
use crate::refusal;

/// The arrays the C# keeps as fields after Start: borrowed for the chunk.
pub struct Buffers<'a> {
    pub literals: &'a [u8],
    pub commands: &'a [u8],
    pub near: &'a [i32],
    pub source: &'a [u8],
}

pub struct MermaidCommands {
    delta: bool,
    window: i32,
    literal_count: i32,
    near_count: i32,
    length_at: i32,
    length_end: i32,
    literal_at: i32,
    near_at: i32,
    recent: i32,
}

impl MermaidCommands {
    pub fn new() -> Self {
        MermaidCommands { delta: false, window: 0, literal_count: 0, near_count: 0, length_at: 0, length_end: 0, literal_at: 0, near_at: 0, recent: 0 }
    }

    /// Starts a chunk: its literal count, near count, and its length bytes, which run from length_at to the payload's end.
    pub fn start(&mut self, window: i32, delta: bool, literal_count: i32, near_count: i32, length_at: i32, length_end: i32) {
        self.window = window;
        self.delta = delta;
        self.literal_count = literal_count;
        self.near_count = near_count;
        self.length_at = length_at;
        self.length_end = length_end;
        self.literal_at = 0;
        self.near_at = 0;
        self.recent = 8;
    }

    /// After the last half: the length bytes ended at the payload's end and every literal was used.
    pub fn used_up(&self) -> bool {
        self.length_at == self.length_end && self.literal_at == self.literal_count
    }

    #[allow(clippy::too_many_arguments)]
    pub fn half(&mut self, b: &Buffers, output: &mut [u8], mut p: i32, half_start: i32, half_end: i32, first_command: i32, end_command: i32, far: &[i32], far_count: i32) -> i32 {
        let (literals, commands, near) = (b.literals, b.commands, b.near);
        let (mut literal_at, literal_count, mut near_at, near_count, mut recent) = (self.literal_at, self.literal_count, self.near_at, self.near_count, self.recent);
        let delta = self.delta;
        let mut far_at = 0i32;
        let window = self.window;
        let half = &mut output[..half_end as usize];
        for i in first_command..end_command {
            let c = commands[i as usize] as i32;
            let literal_run;
            let length;
            if c >= 24 {
                literal_run = c & 7;
                if literal_run > half_end - p || literal_run > literal_count - literal_at {
                    return refusal::CORRUPT;
                }
                lz_steps::literal_run(half, p, literals, literal_at, literal_run, delta, recent);
                literal_at += literal_run;
                p += literal_run;
                if (c & 0x80) == 0 {
                    if near_at >= near_count {
                        return refusal::CORRUPT;
                    }
                    recent = near[near_at as usize];
                    near_at += 1;
                }
                let length = (c >> 3) & 15;
                let refused = lz_steps::check_distance(recent, p, window, false);
                if refused < 0 {
                    return refused;
                }
                if length > half_end - p {
                    return refusal::CORRUPT;
                }
                lz_steps::match_run(half, p, recent, length);
                p += length;
                continue;
            }

            if c == 0 {
                let n = self.next_length(b.source);
                if n < 0 {
                    return n;
                }
                literal_run = n + 64;
                if literal_run > half_end - p || literal_run > literal_count - literal_at {
                    return refusal::CORRUPT;
                }
                lz_steps::literal_run(half, p, literals, literal_at, literal_run, delta, recent);
                literal_at += literal_run;
                p += literal_run;
                continue;
            }

            if c == 1 {
                let n = self.next_length(b.source);
                if n < 0 {
                    return n;
                }
                if near_at >= near_count {
                    return refusal::CORRUPT;
                }
                recent = near[near_at as usize];
                near_at += 1;
                length = n + 91;
            } else {
                if c == 2 {
                    let n = self.next_length(b.source);
                    if n < 0 {
                        return n;
                    }
                    length = n + 29;
                } else {
                    length = c + 5;
                }
                if far_at >= far_count {
                    return refusal::CORRUPT;
                }
                recent = p - (half_start - far[far_at as usize]);
                far_at += 1;
            }
            let result = lz_steps::check_distance(recent, p, window, false);
            if result < 0 {
                return result;
            }
            if length > half_end - p {
                return refusal::CORRUPT;
            }
            lz_steps::match_run(half, p, recent, length);
            p += length;
        }

        let trailing = half_end - p;
        if trailing < 0 || trailing > literal_count - literal_at {
            return refusal::CORRUPT;
        }
        lz_steps::literals_to(half, p, trailing, literals, literal_at, delta, recent);
        self.literal_at = literal_at + trailing;
        self.near_at = near_at;
        self.recent = recent;
        0
    }

    fn next_length(&mut self, source: &[u8]) -> i32 {
        if self.length_at >= self.length_end {
            return refusal::CORRUPT;
        }
        let b = source[self.length_at as usize] as i32;
        self.length_at += 1;
        if b <= 251 {
            return b;
        }
        if self.length_end - self.length_at < 2 {
            return refusal::CORRUPT;
        }
        let a = self.length_at as usize;
        let w = source[a] as i32 | ((source[a + 1] as i32) << 8);
        self.length_at += 2;
        b + 4 * w
    }
}
