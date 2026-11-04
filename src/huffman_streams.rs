//! HuffmanStreams.cs: three LSB streams decoded in turn with one code: the first stream forward, then the shared region backward,
//! then the shared region forward. Past its region a stream reads zeros.

use crate::huffman_table::MAX_LENGTH;

const MASK: u64 = (1 << MAX_LENGTH) - 1;
const THREE_SYMBOLS: i32 = 3 * MAX_LENGTH;

#[allow(clippy::too_many_arguments)]
pub fn decode_three(code: &[u16], src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32, min_shared: i32) -> bool {
    if end - start < 3 {
        return false;
    }
    let first_size = src[start as usize] as i32 | ((src[start as usize + 1] as i32) << 8);
    let first_start = start + 2;
    if first_size + min_shared > end - first_start {
        return false;
    }
    let shared_start = first_start + first_size;

    let (mut w0, mut w1, mut w2) = (0u64, 0u64, 0u64);
    let (mut h0, mut h1, mut h2) = (0i32, 0i32, 0i32);
    let (mut n0, mut n1, mut n2) = (first_start, end, shared_start);
    let mut o = dst_at;
    let stop = dst_at + length;

    while stop - o >= 9 {
        if h0 < THREE_SYMBOLS {
            refill_forward(src, shared_start, &mut w0, &mut h0, &mut n0);
        }
        if h1 < THREE_SYMBOLS {
            refill_backward(src, shared_start, &mut w1, &mut h1, &mut n1);
        }
        if h2 < THREE_SYMBOLS {
            refill_forward(src, end, &mut w2, &mut h2, &mut n2);
        }

        for _ in 0..3 {
            let at = o as usize;
            let mut e = code[(w0 & MASK) as usize] as i32;
            dst[at] = e as u8;
            w0 = w0.wrapping_shr((e >> 8) as u32);
            h0 -= e >> 8;
            e = code[(w1 & MASK) as usize] as i32;
            dst[at + 1] = e as u8;
            w1 = w1.wrapping_shr((e >> 8) as u32);
            h1 -= e >> 8;
            e = code[(w2 & MASK) as usize] as i32;
            dst[at + 2] = e as u8;
            w2 = w2.wrapping_shr((e >> 8) as u32);
            h2 -= e >> 8;
            o += 3;
        }
    }

    let mut lane = 0;
    while o < stop {
        let e;
        if lane == 0 {
            if h0 <= 56 {
                refill_forward(src, shared_start, &mut w0, &mut h0, &mut n0);
            }
            e = code[(w0 & MASK) as usize] as i32;
            w0 = w0.wrapping_shr((e >> 8) as u32);
            h0 -= e >> 8;
        } else if lane == 1 {
            if h1 <= 56 {
                refill_backward(src, shared_start, &mut w1, &mut h1, &mut n1);
            }
            e = code[(w1 & MASK) as usize] as i32;
            w1 = w1.wrapping_shr((e >> 8) as u32);
            h1 -= e >> 8;
        } else {
            if h2 <= 56 {
                refill_forward(src, end, &mut w2, &mut h2, &mut n2);
            }
            e = code[(w2 & MASK) as usize] as i32;
            w2 = w2.wrapping_shr((e >> 8) as u32);
            h2 -= e >> 8;
        }
        dst[o as usize] = e as u8;
        o += 1;
        lane = if lane == 2 { 0 } else { lane + 1 };
    }

    let first_touched = bytes_touched((n0 - first_start) as i64, h0);
    let shared_touched = bytes_touched((end - n1) as i64, h1) + bytes_touched((n2 - shared_start) as i64, h2);
    first_touched == first_size as i64 && shared_touched == (end - shared_start) as i64
}

/// Tops up a forward lane: one 8-byte read keeping the whole bytes that fit, or byte by byte near the region's end and zeros past it.
#[inline(always)]
fn refill_forward(src: &[u8], limit: i32, window: &mut u64, held: &mut i32, next: &mut i32) {
    if limit - *next >= 8 {
        let at = *next as usize;
        *window |= u64::from_le_bytes(src[at..at + 8].try_into().unwrap()).wrapping_shl(*held as u32);
        *next += (63 - *held) >> 3;
        *held |= 56;
        return;
    }
    while *held <= 56 {
        let value = if *next < limit { src[*next as usize] } else { 0 };
        *window |= (value as u64).wrapping_shl(*held as u32);
        *held += 8;
        *next += 1;
    }
}

/// Tops up the backward lane, reading toward the region's start (the byte before next comes first).
#[inline(always)]
fn refill_backward(src: &[u8], limit: i32, window: &mut u64, held: &mut i32, next: &mut i32) {
    if *next - limit >= 8 {
        let at = (*next - 8) as usize;
        *window |= u64::from_be_bytes(src[at..at + 8].try_into().unwrap()).wrapping_shl(*held as u32);
        *next -= (63 - *held) >> 3;
        *held |= 56;
        return;
    }
    while *held <= 56 {
        *next -= 1;
        let value = if *next >= limit { src[*next as usize] } else { 0 };
        *window |= (value as u64).wrapping_shl(*held as u32);
        *held += 8;
    }
}

fn bytes_touched(loaded: i64, held: i32) -> i64 {
    (loaded * 8 - held as i64 + 7) >> 3
}
