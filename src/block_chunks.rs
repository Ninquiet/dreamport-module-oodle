//! OodleBlockChunks.cs: the chunks of 128 KiB inside one compressed block payload.

use crate::coded_array::CodedArray;
use crate::decoder::copy;
use crate::kraken::KrakenChunk;
use crate::leviathan::LeviathanChunk;
use crate::mermaid::MermaidChunk;
use crate::refusal;

pub const CHUNK_SIZE: i32 = 0x20000;

pub const KRAKEN: i32 = 6;
pub const MERMAID: i32 = 10;
pub const LEVIATHAN: i32 = 12;

pub struct BlockChunks {
    arrays: CodedArray,
    kraken: Option<KrakenChunk>,
    mermaid: Option<MermaidChunk>,
    leviathan: Option<LeviathanChunk>,
}

impl BlockChunks {
    pub fn new() -> Self {
        BlockChunks { arrays: CodedArray::new(), kraken: None, mermaid: None, leviathan: None }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, codec: i32, src: &[u8], start: i32, end: i32, output: &mut [u8], block_start: i32, block_length: i32, window: i32) -> i32 {
        let mut at = start;
        let mut done = 0i32;
        while done < block_length {
            let chunk_length = CHUNK_SIZE.min(block_length - done);
            let chunk_start = block_start + done;
            if end - at < 3 {
                return refusal::CORRUPT;
            }
            let a = at as usize;
            let header = ((src[a] as i32) << 16) | ((src[a + 1] as i32) << 8) | src[a + 2] as i32;

            let used;
            if (header & 0x800000) == 0 {
                used = self.arrays.decode_exactly(src, at, end, output, chunk_start, chunk_length, 0);
                if used < 0 {
                    return used;
                }
            } else {
                let payload = header & 0x7FFFF;
                let mode = (header >> 19) & 0xF;
                let payload_start = at + 3;
                if payload > end - payload_start {
                    return refusal::CORRUPT;
                }
                let result = self.decode_lz(codec, src, payload_start, payload_start + payload, output, chunk_start, chunk_length, mode, window);
                if result < 0 {
                    return result;
                }
                used = 3 + payload;
            }

            at += used;
            done += chunk_length;
        }
        if at == end {
            0
        } else {
            refusal::CORRUPT
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_lz(&mut self, codec: i32, src: &[u8], start: i32, end: i32, output: &mut [u8], chunk_start: i32, chunk_length: i32, mode: i32, window: i32) -> i32 {
        let payload = end - start;
        if payload == chunk_length {
            if mode != 0 {
                return refusal::CORRUPT;
            }
            copy(src, start, output, chunk_start, payload);
            return 0;
        }
        if payload > chunk_length {
            return refusal::CORRUPT;
        }

        let arrays = &mut self.arrays;
        match codec {
            KRAKEN => self.kraken.get_or_insert_with(KrakenChunk::new).decode(arrays, src, start, end, output, chunk_start, chunk_length, mode, window),
            MERMAID => self.mermaid.get_or_insert_with(MermaidChunk::new).decode(arrays, src, start, end, output, chunk_start, chunk_length, mode, window),
            LEVIATHAN => self.leviathan.get_or_insert_with(LeviathanChunk::new).decode(arrays, src, start, end, output, chunk_start, chunk_length, mode, window),
            _ => refusal::UNSUPPORTED_CODEC,
        }
    }
}
