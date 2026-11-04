//! LzTail.cs: the bit-packed end of a Kraken or Leviathan chunk: every distance's extra bits and the long lengths,
//! read by two readers that meet, then the length list.

use crate::coded_array::CodedArray;
use crate::growing;
use crate::msb_bits::MsbBits;
use crate::refusal;

pub const NO_REGION: i32 = -1;
const PLAIN_OFFSETS: i32 = 0;
const MAX_LONG_LENGTHS: usize = 508;

pub struct LzTail {
    offset_codes: Vec<u8>,
    low_parts: Vec<u8>,
    length_bytes: Vec<u8>,
    offset_count: i32,
    length_byte_count: i32,
    scale: i32,
    pub distances: Vec<i32>,
    pub lengths: Vec<i32>,
    long_lengths: [i32; MAX_LONG_LENGTHS],
    pub distance_count: i32,
    pub length_count: i32,
}

impl LzTail {
    pub fn new() -> Self {
        LzTail {
            offset_codes: Vec::new(),
            low_parts: Vec::new(),
            length_bytes: Vec::new(),
            offset_count: 0,
            length_byte_count: 0,
            scale: 0,
            distances: Vec::new(),
            lengths: Vec::new(),
            long_lengths: [0; MAX_LONG_LENGTHS],
            distance_count: 0,
            length_count: 0,
        }
    }

    /// The arrays the tail's values come with: an optional scale byte, the offset codes, their low parts when scaled, then the length bytes. Bytes used, or the refusal.
    pub fn read_arrays(&mut self, arrays: &mut CodedArray, src: &[u8], start: i32, end: i32, offset_max: i32, length_max: i32) -> i32 {
        let mut at = start;
        self.scale = PLAIN_OFFSETS;
        if src[at as usize] >= 0x80 {
            self.scale = src[at as usize] as i32 - 127;
            at += 1;
        }
        let mut used = arrays.decode_into(src, at, end, offset_max, &mut self.offset_codes, &mut self.offset_count);
        if used < 0 {
            return used;
        }
        at += used;
        if self.scale > 1 {
            let mut low_count = 0;
            used = arrays.decode_into(src, at, end, offset_max, &mut self.low_parts, &mut low_count);
            if used < 0 {
                return used;
            }
            if low_count != self.offset_count {
                return refusal::CORRUPT;
            }
            at += used;
        }

        used = arrays.decode_into(src, at, end, length_max, &mut self.length_bytes, &mut self.length_byte_count);
        if used < 0 {
            return used;
        }
        at + used - start
    }

    /// Reads the tail of [start, end) for the arrays read last. A region length of -1 means the long lengths and their count are in the tail;
    /// otherwise the long lengths fill the last region_length bytes and their count is the number of 255s among the length bytes. Zero, or the refusal.
    pub fn read(&mut self, src: &[u8], start: i32, end: i32, region_length: i32) -> i32 {
        let offset_count = self.offset_count;
        let length_byte_count = self.length_byte_count;
        let scale = self.scale;
        let scaled = scale > 1;
        let in_tail = region_length < 0;
        let tail_end = if in_tail { end } else { end - region_length };
        let mut x = MsbBits::new(src, start, tail_end, false);
        let mut y = MsbBits::new(src, start, tail_end, true);

        let long_count: i32;
        if in_tail {
            let zeros = y.leading_zeros();
            if zeros > 18 {
                return refusal::CORRUPT;
            }
            y.skip(zeros);
            long_count = y.read(zeros + 1) as i32 - 1;
            if long_count > MAX_LONG_LENGTHS as i32 {
                return refusal::CORRUPT;
            }
        } else {
            let mut count = 0i32;
            for i in 0..length_byte_count as usize {
                if self.length_bytes[i] == 255 {
                    count += 1;
                }
            }
            if count > MAX_LONG_LENGTHS as i32 {
                return refusal::CORRUPT;
            }
            long_count = count;
        }

        growing::ints(&mut self.distances, offset_count);
        self.distance_count = offset_count;
        for i in 0..offset_count as usize {
            let reader = if (i & 1) == 0 { &mut x } else { &mut y };
            let code = self.offset_codes[i] as i32;
            let distance = if scale == PLAIN_OFFSETS {
                plain_distance(reader, code)
            } else {
                scaled_distance(reader, code, scale, if scaled { self.low_parts[i] as i32 } else { 0 })
            };
            if distance < 0 || distance > i32::MAX as i64 {
                return refusal::CORRUPT;
            }
            self.distances[i] = distance as i32;
        }

        if in_tail && !read_long_lengths(&mut self.long_lengths, &mut x, &mut y, long_count) {
            return refusal::CORRUPT;
        }
        if x.bytes_touched() + y.bytes_touched() != (tail_end - start) as i64 {
            return refusal::CORRUPT;
        }
        if !in_tail {
            let mut forward = MsbBits::new(src, tail_end, end, false);
            let mut backward = MsbBits::new(src, tail_end, end, true);
            if !read_long_lengths(&mut self.long_lengths, &mut forward, &mut backward, long_count) {
                return refusal::CORRUPT;
            }
            if forward.bytes_touched() + backward.bytes_touched() != region_length as i64 {
                return refusal::CORRUPT;
            }
        }

        growing::ints(&mut self.lengths, length_byte_count);
        self.length_count = length_byte_count;
        let mut used = 0i32;
        for i in 0..length_byte_count as usize {
            let mut v = self.length_bytes[i] as i32;
            if v == 255 {
                if used >= long_count {
                    return refusal::CORRUPT;
                }
                v += self.long_lengths[used as usize];
                used += 1;
            }
            self.lengths[i] = v + 3;
        }
        if used == long_count {
            0
        } else {
            refusal::CORRUPT
        }
    }
}

fn read_long_lengths<'a>(long_lengths: &mut [i32; MAX_LONG_LENGTHS], even: &mut MsbBits<'a>, odd: &mut MsbBits<'a>, count: i32) -> bool {
    for j in 0..count as usize {
        let reader = if (j & 1) == 0 { &mut *even } else { &mut *odd };
        let z = reader.leading_zeros();
        if z > 12 {
            return false;
        }
        reader.skip(z);
        long_lengths[j] = reader.read(z + 7) as i32 - 64;
    }
    true
}

fn plain_distance(reader: &mut MsbBits, code: i32) -> i64 {
    if code < 0xF0 {
        let n = (code >> 4) + 4;
        let v = reader.read(n) as i64;
        return ((1i64 << n) + v) * 16 + (code & 15) as i64 - 248;
    }
    let wide = code - 0xF0 + 4;
    let high = reader.read(wide) as i64;
    let low = reader.read(12) as i64;
    0x7EFF00i64 + ((1i64 << wide) + high) * 4096 + low
}

fn scaled_distance(reader: &mut MsbBits, code: i32, scale: i32, low: i32) -> i64 {
    let e = code >> 3;
    if e > 26 {
        return -1;
    }
    let v = reader.read_long(e);
    let t = (8i64 + (code & 7) as i64) * (1i64 << e) + v - 8;
    if scale == 1 {
        t
    } else {
        scale as i64 * t + low as i64
    }
}
