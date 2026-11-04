//! TansWeights.cs: the tANS header, the table size and a weight for each used symbol, summing to the table size.

use crate::msb_bits::MsbBits;
use crate::table_bits::{self, MAX_VALUES};

pub struct TansWeights {
    pub size_bits: i32,
    pub weight: [i32; 256],
    values: [i32; MAX_VALUES],
    symbols: [u8; 256],
}

impl TansWeights {
    pub fn new() -> Self {
        TansWeights { size_bits: 0, weight: [0; 256], values: [0; MAX_VALUES], symbols: [0; 256] }
    }

    pub fn size(&self) -> i32 {
        1 << self.size_bits
    }

    pub fn read(&mut self, bits: &mut MsbBits) -> bool {
        self.weight.fill(0);
        if bits.read(1) != 0 {
            return false;
        }
        self.size_bits = bits.read(2) as i32 + 8;
        let ok = if bits.read(1) == 1 { self.read_coded(bits) } else { self.read_short_list(bits) };
        ok && !bits.overran()
    }

    fn read_coded(&mut self, bits: &mut MsbBits) -> bool {
        let extra = bits.read(3) as i32;
        let count = bits.read(8) as i32 + 1;
        if count < 2 {
            return false;
        }
        let ranges = table_bits::range_value_count(bits, count);
        if !table_bits::read_unary(bits, &mut self.values, count + ranges) {
            return false;
        }

        if !table_bits::used_symbols(bits, &self.values, count, ranges, count, &mut self.symbols) {
            return false;
        }

        let (mut average, mut sum) = (6i32, 0i32);
        let size = self.size();
        for i in 0..count as usize {
            let width = extra + self.values[i];
            if width > 15 {
                return false;
            }
            let t = bits.read(width) as i32 + (1 << width) - (1 << extra);
            let half = average >> 2;
            let v = if t <= 2 * half { half + table_bits::zigzag(t) } else { t };
            average += (if 2 * half < v { 2 * half } else { v }) - half;
            self.weight[self.symbols[i] as usize] = v + 1;
            sum += v + 1;
            if sum > size {
                return false;
            }
        }
        sum == size
    }

    fn read_short_list(&mut self, bits: &mut MsbBits) -> bool {
        let listed = bits.read(3) as i32 + 1;
        let width = bits.read(4) as i32;
        if width < 1 || width > self.size_bits {
            return false;
        }

        let (mut running, mut sum) = (0i32, 0i32);
        for _ in 0..listed {
            let symbol = bits.read(8) as usize;
            if self.weight[symbol] != 0 {
                return false;
            }
            running += bits.read(width) as i32;
            if running == 0 {
                return false;
            }
            self.weight[symbol] = running;
            sum += running;
        }

        let last = bits.read(8) as usize;
        let size = self.size();
        if self.weight[last] != 0 || sum >= size {
            return false;
        }
        let rest = size - sum;
        if rest < 2 || rest < running {
            return false;
        }
        self.weight[last] = rest;
        true
    }
}
