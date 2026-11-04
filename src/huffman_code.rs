//! HuffmanCode.cs: a complete canonical code with lengths up to 11, as a lookup indexed by the next 11 bits of an LSB stream.
//! Each entry is the symbol, plus its length times 256.

use crate::huffman_table::MAX_LENGTH;

const WIDTH: i32 = MAX_LENGTH;
pub const SIZE: usize = 1 << WIDTH;
static REVERSED: [u16; SIZE] = build_reversed();

pub struct HuffmanCode {
    pub entries: [u16; SIZE],
    by_length: [i32; WIDTH as usize + 2],
    next: [i32; WIDTH as usize + 2],
    order: [i32; 256],
}

impl HuffmanCode {
    pub fn new() -> Self {
        HuffmanCode { entries: [0; SIZE], by_length: [0; WIDTH as usize + 2], next: [0; WIDTH as usize + 2], order: [0; 256] }
    }

    /// False when the lengths do not make a complete code. Built length by length: the table so far is doubled, then each code of l bits takes its one slot.
    pub fn build(&mut self, symbols: &[u8], lengths: &[i32], count: i32) -> bool {
        let by_length = &mut self.by_length;
        let next = &mut self.next;
        let order = &mut self.order;
        by_length.fill(0);
        for i in 0..count as usize {
            by_length[lengths[i] as usize] += 1;
        }

        let mut filled = 0i32;
        for l in 1..=WIDTH {
            filled += by_length[l as usize] << (WIDTH - l);
        }
        if filled != SIZE as i32 {
            return false;
        }

        next[1] = 0;
        for l in 1..=WIDTH as usize {
            next[l + 1] = next[l] + by_length[l];
        }
        for i in 0..count as usize {
            let l = lengths[i] as usize;
            order[next[l] as usize] = i as i32;
            next[l] += 1;
        }

        let entries = &mut self.entries;
        let (mut code, mut k) = (0i32, 0i32);
        for l in 1..=WIDTH {
            let half = 1usize << (l - 1);
            entries.copy_within(0..half, half);
            let end = k + by_length[l as usize];
            while k < end {
                let i = order[k as usize] as usize;
                entries[REVERSED[code as usize] as usize] = symbols[i] as u16 | ((l as u16) << 8);
                k += 1;
                code += 1 << (WIDTH - l);
            }
        }
        true
    }
}

const fn build_reversed() -> [u16; SIZE] {
    let mut table = [0u16; SIZE];
    let mut i = 0;
    while i < SIZE {
        let mut r = 0usize;
        let mut b = 0;
        while b < WIDTH as usize {
            if (i & (1 << b)) != 0 {
                r |= 1 << (WIDTH as usize - 1 - b);
            }
            b += 1;
        }
        table[i] = r as u16;
        i += 1;
    }
    table
}
