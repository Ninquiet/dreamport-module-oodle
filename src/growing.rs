//! GrowingBuffers.cs: working buffers that only grow, with the same sizes as the C# decoder's.

use crate::coded_array::WORKING_LIMIT;

pub fn bytes(buffer: &mut Vec<u8>, size: i32) {
    if (buffer.len() as i32) < size {
        *buffer = vec![0u8; grown(buffer.len() as i32, size) as usize];
    }
}

pub fn grow(buffer: &mut Vec<u8>, size: i32, kept: i32) {
    if buffer.len() as i32 >= size {
        return;
    }
    let mut next = vec![0u8; grown(buffer.len() as i32, size) as usize];
    if kept > 0 {
        next[..kept as usize].copy_from_slice(&buffer[..kept as usize]);
    }
    *buffer = next;
}

pub fn ints(buffer: &mut Vec<i32>, size: i32) {
    if (buffer.len() as i32) < size {
        *buffer = vec![0i32; grown(buffer.len() as i32, size) as usize];
    }
}

fn grown(current: i32, size: i32) -> i32 {
    size.max(current.wrapping_mul(2).min(WORKING_LIMIT * 2))
}
