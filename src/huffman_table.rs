//! HuffmanTableReader.cs: a Huffman code-length table in any of its three forms.

use crate::msb_bits::MsbBits;
use crate::table_bits;

pub const MAX_LENGTH: i32 = 11;
const RUN_LIMITS: [i32; 4] = [20, 10, 5, 2];

/// Number of used symbols written to symbols and lengths, or -1.
pub fn read(bits: &mut MsbBits, symbols: &mut [u8], lengths: &mut [i32], values: &mut [i32]) -> i32 {
    if bits.read(1) == 0 {
        return if bits.read(1) == 0 { read_sparse(bits, symbols, lengths) } else { read_runs(bits, symbols, lengths) };
    }
    if bits.read(1) != 0 {
        return -1;
    }
    read_unary_form(bits, symbols, lengths, values)
}

fn read_sparse(bits: &mut MsbBits, symbols: &mut [u8], lengths: &mut [i32]) -> i32 {
    let count = bits.read(8) as i32;
    if count == 0 {
        return -1;
    }
    if count == 1 {
        symbols[0] = bits.read(8) as u8;
        lengths[0] = 0;
        return 1;
    }
    let width = bits.read(3) as i32;
    if width > 4 {
        return -1;
    }
    for i in 0..count as usize {
        symbols[i] = bits.read(8) as u8;
        lengths[i] = bits.read(width) as i32 + 1;
        if lengths[i] > MAX_LENGTH {
            return -1;
        }
    }
    count
}

fn read_runs(bits: &mut MsbBits, symbols: &mut [u8], lengths: &mut [i32]) -> i32 {
    let forced = bits.read(2) as i32;
    let limit = RUN_LIMITS[forced as usize];
    let mut average = 32i32;
    let mut used = bits.read(1) == 1;
    let (mut current, mut count) = (0i32, 0i32);

    loop {
        if !used {
            let skip = read_run(bits);
            if skip < 0 {
                return -1;
            }
            current += skip;
            if current >= 256 {
                break;
            }
        }

        let run = read_run(bits);
        if run < 0 || current + run > 256 {
            return -1;
        }
        for _ in 0..run {
            let zeros = bits.leading_zeros();
            if zeros > limit {
                return -1;
            }
            bits.skip(zeros + 1);
            let u = (zeros << forced) + bits.read(forced) as i32;
            let length = table_bits::zigzag(u) + ((average + 2) >> 2);
            if !(1..=MAX_LENGTH).contains(&length) {
                return -1;
            }
            average = length + ((3 * average + 2) >> 2);
            symbols[count as usize] = current as u8;
            current += 1;
            lengths[count as usize] = length;
            count += 1;
        }
        if current == 256 {
            break;
        }
        used = false;
    }

    if current == 256 && count >= 2 {
        count
    } else {
        -1
    }
}

fn read_run(bits: &mut MsbBits) -> i32 {
    let zeros = bits.leading_zeros();
    if zeros > 7 {
        return -1;
    }
    bits.skip(zeros);
    bits.read(zeros + 2) as i32 - 1
}

fn read_unary_form(bits: &mut MsbBits, symbols: &mut [u8], lengths: &mut [i32], values: &mut [i32]) -> i32 {
    let forced = bits.read(2) as i32;
    let count = bits.read(8) as i32 + 1;
    let ranges = table_bits::range_value_count(bits, count);
    if !table_bits::read_unary(bits, values, count + ranges) {
        return -1;
    }

    if forced > 0 {
        for i in 0..count as usize {
            values[i] = (values[i] << forced) + bits.read(forced) as i32;
        }
    }
    if bits.overran() {
        return -1;
    }

    if !table_bits::used_symbols(bits, values, count, ranges, count, symbols) {
        return -1;
    }

    let mut sum = 30i32;
    for i in 0..count as usize {
        let delta = table_bits::zigzag(values[i]);
        let length = delta + (sum >> 2) + 1;
        if !(1..=MAX_LENGTH).contains(&length) {
            return -1;
        }
        lengths[i] = length;
        sum += delta;
    }
    count
}
