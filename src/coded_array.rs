//! CodedArray.cs and CodedArrayHeader.cs: one coded array of any type, refusing anything larger than the caller allows.

use crate::growing;
use crate::huffman_array::HuffmanArray;
use crate::multi_array::{self, Work};
use crate::refusal;
use crate::run_length;
use crate::tans_array::TansArray;

pub const MAX_DEPTH: i32 = 16;
pub const WORKING_LIMIT: i32 = 0x40000;

pub const STORED: i32 = 0;
pub const TANS: i32 = 1;
pub const HUFFMAN_THREE: i32 = 2;
pub const RUN_LENGTH: i32 = 3;
pub const HUFFMAN_SIX: i32 = 4;
pub const COMPOSITE: i32 = 5;

#[derive(Clone, Copy, Default)]
pub struct Header {
    pub kind: i32,
    pub header_length: i32,
    pub payload_length: i32,
    pub decoded_length: i32,
}

impl Header {
    pub fn try_read(src: &[u8], at: i32, end: i32) -> Option<Header> {
        let mut header = Header::default();
        if at >= end {
            return None;
        }
        let a = at as usize;
        let first = src[a] as i32;
        let short_form = (first & 0x80) != 0;
        header.kind = (first >> 4) & 7;
        if header.kind > COMPOSITE {
            return None;
        }

        if header.kind == STORED {
            return Self::read_stored(src, at, end, first, short_form, header);
        }

        if short_form {
            if end - at < 3 {
                return None;
            }
            let v = (first << 16) | ((src[a + 1] as i32) << 8) | src[a + 2] as i32;
            header.header_length = 3;
            header.payload_length = v & 0x3FF;
            header.decoded_length = header.payload_length + ((v >> 10) & 0x3FF) + 1;
            return Some(header);
        }

        if end - at < 5 {
            return None;
        }
        let w = ((src[a + 1] as u32) << 24) | ((src[a + 2] as u32) << 16) | ((src[a + 3] as u32) << 8) | src[a + 4] as u32;
        header.header_length = 5;
        header.payload_length = (w & 0x3FFFF) as i32;
        header.decoded_length = ((first & 0xF) << 14) + (w >> 18) as i32 + 1;
        if header.payload_length < header.decoded_length {
            Some(header)
        } else {
            None
        }
    }

    fn read_stored(src: &[u8], at: i32, end: i32, first: i32, short_form: bool, mut header: Header) -> Option<Header> {
        let a = at as usize;
        if short_form {
            if end - at < 2 {
                return None;
            }
            header.header_length = 2;
            header.decoded_length = ((first & 0xF) << 8) | src[a + 1] as i32;
        } else {
            if end - at < 3 {
                return None;
            }
            if (first & 0x0C) != 0 {
                return None;
            }
            header.header_length = 3;
            header.decoded_length = ((first & 0x3) << 16) | ((src[a + 1] as i32) << 8) | src[a + 2] as i32;
        }
        header.payload_length = header.decoded_length;
        Some(header)
    }
}

pub struct CodedArray {
    huffman: HuffmanArray,
    tans: TansArray,
    pub(crate) commands_by_depth: Vec<Vec<u8>>,
    pub(crate) multi_work: Vec<Work>,
}

impl CodedArray {
    pub fn new() -> Self {
        CodedArray {
            huffman: HuffmanArray::new(),
            tans: TansArray::new(),
            commands_by_depth: (0..=MAX_DEPTH).map(|_| Vec::new()).collect(),
            multi_work: (0..MAX_DEPTH + 2).map(|_| Work::default()).collect(),
        }
    }

    /// Stream bytes used by the array, or a refusal; decoded receives its decoded length.
    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, src: &[u8], at: i32, end: i32, dst: &mut [u8], dst_at: i32, max: i32, depth: i32, decoded: &mut i32) -> i32 {
        *decoded = 0;
        if depth > MAX_DEPTH {
            return refusal::CORRUPT;
        }

        let header = match Header::try_read(src, at, end) {
            Some(h) => h,
            None => return refusal::CORRUPT,
        };
        let payload_start = at + header.header_length;
        let payload_end = payload_start + header.payload_length;
        if header.payload_length > end - payload_start {
            return refusal::CORRUPT;
        }
        if header.decoded_length > max || header.decoded_length > dst.len() as i32 - dst_at {
            return refusal::CORRUPT;
        }

        let result = self.decode_payload(header, src, payload_start, payload_end, dst, dst_at, depth);
        if result < 0 {
            return result;
        }

        *decoded = header.decoded_length;
        payload_end - at
    }

    /// Decodes an array that must produce exactly the given length.
    #[allow(clippy::too_many_arguments)]
    pub fn decode_exactly(&mut self, src: &[u8], at: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32, depth: i32) -> i32 {
        let mut decoded = 0;
        let used = self.decode(src, at, end, dst, dst_at, length, depth, &mut decoded);
        if used < 0 {
            return used;
        }
        if decoded == length {
            used
        } else {
            refusal::CORRUPT
        }
    }

    /// Decodes into the start of a reused buffer, grown to the caller's maximum (at most the working limit).
    pub fn decode_into(&mut self, src: &[u8], at: i32, end: i32, max: i32, buffer: &mut Vec<u8>, length: &mut i32) -> i32 {
        let room = 0.max(max.min(WORKING_LIMIT));
        growing::bytes(buffer, room);
        self.decode(src, at, end, buffer, 0, room, 0, length)
    }

    /// The room a nested array at [at, end) needs: its decoded length from its header, at most max; 0 when the header cannot be read.
    pub fn room_for(src: &[u8], at: i32, end: i32, max: i32) -> i32 {
        match Header::try_read(src, at, end) {
            Some(h) => 0.max(h.decoded_length.min(max)),
            None => 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn multi_decode(&mut self, src: &[u8], at: i32, end: i32, arrays: i32, dst: &mut [u8], dst_at: i32, room: i32, bounds: &mut [i32], depth: i32) -> i32 {
        multi_array::decode(self, src, at, end, arrays, dst, dst_at, room, bounds, depth)
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_payload(&mut self, header: Header, src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, depth: i32) -> i32 {
        let length = header.decoded_length;
        match header.kind {
            STORED => {
                crate::decoder::copy(src, start, dst, dst_at, length);
                0
            }
            TANS => self.tans.decode(src, start, end, dst, dst_at, length),
            HUFFMAN_THREE | HUFFMAN_SIX => self.huffman.decode(src, start, end, dst, dst_at, length, header.kind == HUFFMAN_SIX),
            RUN_LENGTH => run_length::decode(self, src, start, end, dst, dst_at, length, depth),
            _ => self.decode_composite(src, start, end, dst, dst_at, length, depth),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_composite(&mut self, src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32, depth: i32) -> i32 {
        if end - start < 6 || (src[start as usize] & 0x7F) < 2 {
            return refusal::CORRUPT;
        }
        if (src[start as usize] & 0x80) != 0 {
            let mut bounds = [0i32; 2];
            let used = self.multi_decode(src, start, end, 1, dst, dst_at, length, &mut bounds, depth + 1);
            if used < 0 {
                return used;
            }
            return if used == end - start && bounds[1] == length { 0 } else { refusal::CORRUPT };
        }
        self.concatenate(src, start, end, dst, dst_at, length, depth)
    }

    #[allow(clippy::too_many_arguments)]
    fn concatenate(&mut self, src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32, depth: i32) -> i32 {
        let count = src[start as usize] as i32;
        let mut at = start + 1;
        let mut written = 0i32;
        for _ in 0..count {
            let mut decoded = 0;
            let used = self.decode(src, at, end, dst, dst_at + written, length - written, depth + 1, &mut decoded);
            if used < 0 {
                return used;
            }
            at += used;
            written += decoded;
        }
        if at == end && written == length {
            0
        } else {
            refusal::CORRUPT
        }
    }
}
