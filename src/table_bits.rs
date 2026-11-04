//! TableBits.cs: zigzag, truncated binary, unary values and the used-symbol ranges.

use crate::msb_bits::MsbBits;

pub const MAX_UNARY: i32 = 255;
pub const MAX_VALUES: usize = 512;

#[inline(always)]
pub fn zigzag(value: i32) -> i32 {
    if (value & 1) == 0 {
        value >> 1
    } else {
        -((value + 1) >> 1)
    }
}

pub fn bit_length(mut value: i32) -> i32 {
    let mut bits = 0;
    while value > 0 {
        bits += 1;
        value >>= 1;
    }
    bits
}

pub fn truncated_binary(bits: &mut MsbBits, below: i32) -> i32 {
    let width = bit_length(below - 1);
    let spare = (1 << width) - below;
    let look = bits.peek(width) as i32;
    if (look >> 1) < spare {
        bits.skip(width - 1);
        return look >> 1;
    }
    bits.skip(width);
    look - spare
}

/// Range-value count for n used symbols: none when all 256 are used.
pub fn range_value_count(bits: &mut MsbBits, used: i32) -> i32 {
    if used == 256 {
        return 0;
    }
    let other = 257 - used;
    truncated_binary(bits, 2 * if other < used { other } else { used })
}

/// Reads count unary values; false when one is too long.
pub fn read_unary(bits: &mut MsbBits, values: &mut [i32], count: i32) -> bool {
    for i in 0..count as usize {
        let mut total = 0i32;
        loop {
            let zeros = bits.leading_zeros();
            if zeros < 32 {
                bits.skip(zeros + 1);
                total += zeros;
                break;
            }
            bits.skip(32);
            total += 32;
            if total > MAX_UNARY {
                return false;
            }
        }
        if total > MAX_UNARY {
            return false;
        }
        values[i] = total;
    }
    !bits.overran()
}

/// Fills symbols with the used symbols in ascending order, from the range values found after the first used values.
pub fn used_symbols(bits: &mut MsbBits, values: &[i32], first: i32, range_count: i32, used: i32, symbols: &mut [u8]) -> bool {
    let (mut current, mut taken, mut r) = (0i32, 0i32, first as usize);
    if (range_count & 1) != 0 {
        let width = values[r];
        r += 1;
        if width >= 8 {
            return false;
        }
        current = bits.read(width + 1) as i32 + (1 << (width + 1)) - 1;
    }

    for _ in 0..range_count / 2 {
        let a = values[r];
        let b = values[r + 1];
        r += 2;
        if a >= 9 || b >= 8 {
            return false;
        }
        let count = bits.read(a) as i32 + (1 << a);
        let gap = bits.read(b + 1) as i32 + (1 << (b + 1)) - 1;
        if current + count + gap >= 256 || taken + count >= used {
            return false;
        }
        for k in 0..count {
            symbols[taken as usize] = (current + k) as u8;
            taken += 1;
        }
        current += count + gap;
    }

    if current >= 256 || taken >= used || current + (used - taken) > 256 {
        return false;
    }
    while taken < used {
        symbols[taken as usize] = current as u8;
        taken += 1;
        current += 1;
    }
    true
}
