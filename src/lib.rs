//! Somnium's Oodle decoder as a WebAssembly module: bytes in, bytes out, no imports.
//! A function-by-function translation of `dreamport/contract/Decode/Oodle/*.cs`.

mod block_chunks;
mod coded_array;
mod decoder;
mod growing;
mod huffman_array;
mod huffman_code;
mod huffman_streams;
mod huffman_table;
mod kraken;
mod leviathan;
mod leviathan_commands;
mod lz_steps;
mod lz_tail;
mod mermaid;
mod mermaid_commands;
mod msb_bits;
mod multi_array;
mod refusal;
mod run_length;
mod table_bits;
mod tans_array;
mod tans_weights;

use std::alloc::{alloc, dealloc, Layout};
use std::cell::UnsafeCell;

const ABI_VERSION: u32 = 1;
const ALIGN: usize = 16;

struct State(UnsafeCell<Option<block_chunks::BlockChunks>>);

// The module is single-threaded: one instance, one call at a time.
unsafe impl Sync for State {}

static STATE: State = State(UnsafeCell::new(None));

#[no_mangle]
pub extern "C" fn dp_abi() -> u32 {
    ABI_VERSION
}

#[no_mangle]
pub extern "C" fn dp_alloc(len: u32) -> u32 {
    if len == 0 {
        return 0;
    }
    match Layout::from_size_align(len as usize, ALIGN) {
        Ok(layout) => unsafe { alloc(layout) as usize as u32 },
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "C" fn dp_free(ptr: u32, len: u32) {
    if ptr == 0 || len == 0 {
        return;
    }
    if let Ok(layout) = Layout::from_size_align(len as usize, ALIGN) {
        unsafe { dealloc(ptr as usize as *mut u8, layout) }
    }
}

#[no_mangle]
pub extern "C" fn dp_decode(in_ptr: u32, in_len: u32, out_ptr: u32, out_len: u32) -> i64 {
    if (in_ptr == 0 && in_len != 0) || (out_ptr == 0 && out_len != 0) || out_len > i32::MAX as u32 {
        return refusal::BAD_ARGUMENTS as i64;
    }
    let input: &[u8] = if in_len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(in_ptr as usize as *const u8, in_len as usize) } };
    let output: &mut [u8] = if out_len == 0 { &mut [] } else { unsafe { std::slice::from_raw_parts_mut(out_ptr as usize as *mut u8, out_len as usize) } };
    let chunks = unsafe { (*STATE.0.get()).get_or_insert_with(block_chunks::BlockChunks::new) };
    decoder::decompress(chunks, input, output, out_len as i32)
}
