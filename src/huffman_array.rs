//! HuffmanArray.cs: the table, then three streams or two sets of three.

use crate::huffman_code::HuffmanCode;
use crate::huffman_streams::decode_three;
use crate::huffman_table;
use crate::msb_bits::MsbBits;
use crate::refusal;
use crate::table_bits::MAX_VALUES;

pub struct HuffmanArray {
    symbols: [u8; 256],
    lengths: [i32; 256],
    values: [i32; MAX_VALUES],
    code: HuffmanCode,
}

impl HuffmanArray {
    pub fn new() -> Self {
        HuffmanArray { symbols: [0; 256], lengths: [0; 256], values: [0; MAX_VALUES], code: HuffmanCode::new() }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode(&mut self, src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32, six_streams: bool) -> i32 {
        let mut table = MsbBits::new(src, start, end, false);
        let count = huffman_table::read(&mut table, &mut self.symbols, &mut self.lengths, &mut self.values);
        if count < 1 || table.overran() {
            return refusal::CORRUPT;
        }
        let at = start + table.bytes_touched() as i32;

        if count == 1 {
            if at != end {
                return refusal::CORRUPT;
            }
            dst[dst_at as usize..(dst_at + length) as usize].fill(self.symbols[0]);
            return 0;
        }

        if !self.code.build(&self.symbols, &self.lengths, count) {
            return refusal::CORRUPT;
        }

        let decoded = if six_streams {
            self.decode_six(src, at, end, dst, dst_at, length)
        } else {
            decode_three(&self.code.entries, src, at, end, dst, dst_at, length, 0)
        };
        if decoded {
            0
        } else {
            refusal::CORRUPT
        }
    }

    /// Six streams: a 3-byte size of the first set of three, then the two sets, the first for the first half of the output (one longer when the length is odd).
    fn decode_six(&self, src: &[u8], at: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32) -> bool {
        if end - at < 6 {
            return false;
        }
        let a = at as usize;
        let first_size = src[a] as i32 | ((src[a + 1] as i32) << 8) | ((src[a + 2] as i32) << 16);
        let first_start = at + 3;
        if first_size > end - first_start {
            return false;
        }
        let first_end = first_start + first_size;
        let first_half = (length + 1) / 2;
        decode_three(&self.code.entries, src, first_start, first_end, dst, dst_at, first_half, 2)
            && decode_three(&self.code.entries, src, first_end, end, dst, dst_at + first_half, length - first_half, 2)
    }
}
