//! TansArray.cs: the weights, the state table built from them, and five states fed by two readers that meet.
//! Each table entry packs the symbol (bits 0-7), the bits to read (8-11) and the base of the next state (12 and up).

use crate::msb_bits::MsbBits;
use crate::refusal;
use crate::table_bits;
use crate::tans_weights::TansWeights;

const STATES: i32 = 5;
const MAX_SIZE: usize = 1 << 11;
const FIVE_READS: i32 = STATES * 11;

pub struct TansArray {
    weights: TansWeights,
    entries: [i32; MAX_SIZE],
    group_end: [i32; 4],
}

impl TansArray {
    pub fn new() -> Self {
        TansArray { weights: TansWeights::new(), entries: [0; MAX_SIZE], group_end: [0; 4] }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32) -> i32 {
        if end - start < 8 || length < STATES {
            return refusal::CORRUPT;
        }

        let mut header = MsbBits::new(src, start, end, false);
        if !self.weights.read(&mut header) {
            return refusal::CORRUPT;
        }
        let data_start = start + header.bytes_touched() as i32;
        if data_start >= end {
            return refusal::CORRUPT;
        }

        self.build_table();
        let entries = &self.entries;

        let (mut fw, mut bw) = (0u64, 0u64);
        let (mut fh, mut bh, mut fn_, mut bn) = (0i32, 0i32, data_start, end);
        fill_forward(src, end, &mut fw, &mut fh, &mut fn_);
        fill_backward(src, data_start, &mut bw, &mut bh, &mut bn);
        let bits = self.weights.size_bits;
        let mask = (1u64 << bits) - 1;
        let mut s0 = (fw & mask) as i32;
        let mut s2 = ((fw >> bits) & mask) as i32;
        let mut s4 = ((fw >> (2 * bits)) & mask) as i32;
        fw >>= 3 * bits;
        fh -= 3 * bits;
        let mut s1 = (bw & mask) as i32;
        let mut s3 = ((bw >> bits) & mask) as i32;
        bw >>= 2 * bits;
        bh -= 2 * bits;

        let mut o = dst_at;
        let coded = dst_at + length - STATES;
        while coded - o >= 2 * STATES {
            if fh < FIVE_READS {
                fill_forward(src, end, &mut fw, &mut fh, &mut fn_);
            }
            s0 = step(entries, s0, dst, o, &mut fw, &mut fh);
            s1 = step(entries, s1, dst, o + 1, &mut fw, &mut fh);
            s2 = step(entries, s2, dst, o + 2, &mut fw, &mut fh);
            s3 = step(entries, s3, dst, o + 3, &mut fw, &mut fh);
            s4 = step(entries, s4, dst, o + 4, &mut fw, &mut fh);
            if bh < FIVE_READS {
                fill_backward(src, data_start, &mut bw, &mut bh, &mut bn);
            }
            s0 = step(entries, s0, dst, o + 5, &mut bw, &mut bh);
            s1 = step(entries, s1, dst, o + 6, &mut bw, &mut bh);
            s2 = step(entries, s2, dst, o + 7, &mut bw, &mut bh);
            s3 = step(entries, s3, dst, o + 8, &mut bw, &mut bh);
            s4 = step(entries, s4, dst, o + 9, &mut bw, &mut bh);
            o += 2 * STATES;
        }

        let mut k = 0i32;
        while o < coded {
            let forward = k < STATES;
            if forward {
                fill_forward(src, end, &mut fw, &mut fh, &mut fn_);
            } else {
                fill_backward(src, data_start, &mut bw, &mut bh, &mut bn);
            }
            let (window, held) = if forward { (&mut fw, &mut fh) } else { (&mut bw, &mut bh) };
            match k % STATES {
                0 => s0 = step(entries, s0, dst, o, window, held),
                1 => s1 = step(entries, s1, dst, o, window, held),
                2 => s2 = step(entries, s2, dst, o, window, held),
                3 => s3 = step(entries, s3, dst, o, window, held),
                _ => s4 = step(entries, s4, dst, o, window, held),
            }
            k += 1;
            o += 1;
        }

        if (s0 | s1 | s2 | s3 | s4) >= 256 {
            return refusal::CORRUPT;
        }
        let at = o as usize;
        dst[at] = s0 as u8;
        dst[at + 1] = s1 as u8;
        dst[at + 2] = s2 as u8;
        dst[at + 3] = s3 as u8;
        dst[at + 4] = s4 as u8;

        let touched = bytes_touched((fn_ - data_start) as i64, fh) + bytes_touched((end - bn) as i64, bh);
        if touched == (end - data_start) as i64 {
            0
        } else {
            refusal::CORRUPT
        }
    }

    fn build_table(&mut self) {
        let weights = &self.weights;
        let entries = &mut self.entries;
        let (size, bits) = (weights.size(), weights.size_bits);
        let mut singles = 0i32;
        for s in 0..256 {
            if weights.weight[s] == 1 {
                singles += 1;
            }
        }

        let spread = size - singles;
        let mut at = spread;
        for s in 0..256i32 {
            if weights.weight[s as usize] != 1 {
                continue;
            }
            entries[at as usize] = s | (bits << 8);
            at += 1;
        }

        let group_end = &mut self.group_end;
        let mut group_start = 0i32;
        for j in 0..4i32 {
            group_end[j as usize] = group_start;
            group_start += spread / 4 + if j < spread % 4 { 1 } else { 0 };
        }

        let mut walked = 0i32;
        for s in 0..256i32 {
            let w = weights.weight[s as usize];
            if w < 2 {
                continue;
            }
            let mut x = w;
            for j in 0..4usize {
                let in_group = count_with_remainder(walked, walked + w, j as i32);
                for _ in 0..in_group {
                    let drop = bits - (table_bits::bit_length(x) - 1);
                    entries[group_end[j] as usize] = s | (drop << 8) | (((x << drop) - size) << 12);
                    group_end[j] += 1;
                    x += 1;
                }
            }
            walked += w;
        }
    }
}

#[inline(always)]
fn fill_forward(src: &[u8], end: i32, window: &mut u64, held: &mut i32, next: &mut i32) {
    while *held <= 56 {
        let value = if *next < end { src[*next as usize] } else { 0 };
        *window |= (value as u64).wrapping_shl(*held as u32);
        *held += 8;
        *next += 1;
    }
}

#[inline(always)]
fn fill_backward(src: &[u8], start: i32, window: &mut u64, held: &mut i32, next: &mut i32) {
    while *held <= 56 {
        *next -= 1;
        let value = if *next >= start { src[*next as usize] } else { 0 };
        *window |= (value as u64).wrapping_shl(*held as u32);
        *held += 8;
    }
}

#[inline(always)]
fn step(entries: &[i32; MAX_SIZE], state: i32, dst: &mut [u8], o: i32, window: &mut u64, held: &mut i32) -> i32 {
    let entry = entries[state as usize];
    dst[o as usize] = entry as u8;
    let count = (entry >> 8) & 15;
    let next = (entry >> 12) + (*window & (1u64.wrapping_shl(count as u32).wrapping_sub(1))) as i32;
    *window = window.wrapping_shr(count as u32);
    *held -= count;
    next
}

fn bytes_touched(loaded: i64, held: i32) -> i64 {
    (loaded * 8 - held as i64 + 7) >> 3
}

fn count_with_remainder(from: i32, to: i32, remainder: i32) -> i32 {
    let first = from + ((remainder - from % 4) + 4) % 4;
    if first >= to {
        0
    } else {
        (to - 1 - first) / 4 + 1
    }
}
