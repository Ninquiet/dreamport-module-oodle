//! DreamportOodleDecoder.cs: blocks of 256 KiB, each stored, filled or compressed.

use crate::block_chunks::{BlockChunks, KRAKEN, LEVIATHAN, MERMAID};
use crate::refusal;

pub const BLOCK_SIZE: i32 = 0x40000;
const FILL_MARKER: i32 = 0x3FFFF;
const RESET_FLAG: i32 = 0x80;

pub fn decompress(chunks: &mut BlockChunks, compressed: &[u8], raw: &mut [u8], raw_length: i32) -> i64 {
    if raw_length < 0 || raw_length as usize > raw.len() {
        return refusal::BAD_ARGUMENTS as i64;
    }
    let result = decode_blocks(chunks, compressed, raw, raw_length);
    if result >= 0 {
        return result;
    }
    raw[..raw_length as usize].fill(0);
    result
}

fn decode_blocks(chunks: &mut BlockChunks, compressed: &[u8], raw: &mut [u8], raw_length: i32) -> i64 {
    let (mut at, end, mut window) = (0i32, compressed.len() as i32, 0i32);
    let mut block_start = 0i32;
    while block_start < raw_length {
        let block_length = BLOCK_SIZE.min(raw_length - block_start);
        if end - at >= 1 && (compressed[at as usize] as i32 & RESET_FLAG) != 0 {
            window = block_start;
        }
        let used = decode_block(chunks, compressed, at, end, raw, block_start, block_length, window);
        if used < 0 {
            return used as i64;
        }
        at += used;
        block_start += block_length;
    }
    raw_length as i64
}

#[allow(clippy::too_many_arguments)]
fn decode_block(chunks: &mut BlockChunks, src: &[u8], at: i32, end: i32, output: &mut [u8], block_start: i32, block_length: i32, window: i32) -> i32 {
    if end - at < 2 {
        return refusal::CORRUPT;
    }
    let flags = src[at as usize] as i32;
    let codec_byte = src[at as usize + 1] as i32;
    if (flags & 0x0F) != 0x0C || (flags & 0x30) != 0 {
        return refusal::CORRUPT;
    }
    let codec = codec_byte & 0x7F;
    let checksums = (codec_byte & 0x80) != 0;
    if codec != KRAKEN && codec != MERMAID && codec != LEVIATHAN {
        return refusal::UNSUPPORTED_CODEC;
    }
    let mut p = at + 2;

    if (flags & 0x40) != 0 {
        if end - p < block_length {
            return refusal::CORRUPT;
        }
        copy(src, p, output, block_start, block_length);
        return p + block_length - at;
    }

    if end - p < 3 {
        return refusal::CORRUPT;
    }
    let header = ((src[p as usize] as i32) << 16) | ((src[p as usize + 1] as i32) << 8) | src[p as usize + 2] as i32;
    p += 3;
    let size_field = header & 0x3FFFF;

    if size_field == FILL_MARKER {
        if (header >> 18) != 1 || end - p < 1 {
            return refusal::CORRUPT;
        }
        let fill = src[p as usize];
        p += 1;
        output[block_start as usize..(block_start + block_length) as usize].fill(fill);
        return p - at;
    }

    let payload = size_field + 1;
    if checksums {
        if end - p < 3 {
            return refusal::CORRUPT;
        }
        p += 3;
    }
    if payload > end - p || payload > block_length {
        return refusal::CORRUPT;
    }

    if payload == block_length {
        copy(src, p, output, block_start, block_length);
    } else {
        let result = chunks.decode(codec, src, p, p + payload, output, block_start, block_length, window);
        if result < 0 {
            return result;
        }
    }
    p + payload - at
}

pub fn copy(src: &[u8], from: i32, dst: &mut [u8], at: i32, count: i32) {
    dst[at as usize..(at + count) as usize].copy_from_slice(&src[from as usize..(from + count) as usize]);
}
