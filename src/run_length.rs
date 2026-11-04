//! RunLengthArray.cs: literal bytes taken from the front of a command buffer, commands from its back.

use crate::coded_array::{CodedArray, WORKING_LIMIT};
use crate::growing;
use crate::refusal;

#[allow(clippy::too_many_arguments)]
pub fn decode(arrays: &mut CodedArray, src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32, depth: i32) -> i32 {
    let size = end - start;
    if size == 0 {
        return refusal::CORRUPT;
    }
    if size == 1 {
        dst[dst_at as usize..(dst_at + length) as usize].fill(src[start as usize]);
        return 0;
    }

    if src[start as usize] == 0 {
        return if run(src, start + 1, end, dst, dst_at, length) { 0 } else { refusal::CORRUPT };
    }

    let mut buffer = std::mem::take(&mut arrays.commands_by_depth[depth as usize]);
    let result = decode_nested(arrays, &mut buffer, src, start, end, dst, dst_at, length, depth);
    arrays.commands_by_depth[depth as usize] = buffer;
    result
}

#[allow(clippy::too_many_arguments)]
fn decode_nested(arrays: &mut CodedArray, buffer: &mut Vec<u8>, src: &[u8], start: i32, end: i32, dst: &mut [u8], dst_at: i32, length: i32, depth: i32) -> i32 {
    let room = CodedArray::room_for(src, start, end, WORKING_LIMIT);
    growing::bytes(buffer, room + (end - start));
    let mut decoded = 0;
    let used = arrays.decode(src, start, end, buffer, 0, WORKING_LIMIT, depth + 1, &mut decoded);
    if used < 0 {
        return used;
    }
    let rest = end - (start + used);
    crate::decoder::copy(src, start + used, buffer, decoded, rest);
    if run(buffer, 0, decoded + rest, dst, dst_at, length) {
        0
    } else {
        refusal::CORRUPT
    }
}

fn run(buffer: &[u8], mut front: i32, mut back: i32, dst: &mut [u8], dst_at: i32, length: i32) -> bool {
    let mut o = dst_at;
    let stop = dst_at + length;
    let mut repeated = 0u8;

    while front < back {
        let c = buffer[(back - 1) as usize] as i32;
        let mut literals = 0i32;
        let mut run = 0i32;

        if c == 0x01 {
            back -= 1;
            if front >= back {
                return false;
            }
            repeated = buffer[front as usize];
            front += 1;
            continue;
        }

        if c == 0 || c >= 0x30 {
            back -= 1;
            literals = 15 - (c & 15);
            run = c >> 4;
        } else {
            if back - front < 2 {
                return false;
            }
            let mut w = buffer[(back - 2) as usize] as i32 + 256 * c;
            back -= 2;
            if c >= 0x10 {
                w -= 0x1000;
                literals = w & 63;
                run = w >> 6;
            } else if c >= 0x09 {
                run = (w - 0x8FF) * 128;
            } else {
                literals = (w - 0x1FF) * 64;
            }
        }

        if literals > back - front || literals > stop - o {
            return false;
        }
        crate::decoder::copy(buffer, front, dst, o, literals);
        front += literals;
        o += literals;

        if run > stop - o {
            return false;
        }
        dst[o as usize..(o + run) as usize].fill(repeated);
        o += run;
    }

    front == back && o == stop
}
