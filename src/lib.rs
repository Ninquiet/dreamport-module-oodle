//! Somnium's Oodle decoder and Unreal .pak reader as a WebAssembly module (ABI v4).
//! The decoder is a function-by-function translation of `dreamport/contract/Decode/Oodle/*.cs`; the reader, of
//! the client's `Unpackers/Pak/*.cs`. The reader's only import is `dp_host.read`, the container being listed.

mod block_chunks;
mod coded_array;
mod decoder;
mod growing;
mod host;
mod huffman_array;
mod huffman_code;
mod huffman_streams;
mod huffman_table;
mod kraken;
mod leviathan;
mod leviathan_commands;
mod lines;
mod lz_steps;
mod lz_tail;
mod mermaid;
mod mermaid_commands;
mod msb_bits;
mod multi_array;
mod pak;
mod refusal;
mod run_length;
mod table_bits;
mod tans_array;
mod tans_weights;

use std::alloc::{alloc, dealloc, Layout};
use std::cell::UnsafeCell;

const ABI_VERSION: u32 = 4;
const DESCRIPTION: &[u8] = br#"{"readers":["unrealpak"],"decoders":["oodle"]}"#;
const ALIGN: usize = 16;

struct Held {
    chunks: Option<block_chunks::BlockChunks>,
    listing: Vec<u8>,
}

struct State(UnsafeCell<Held>);

// The module is single-threaded: one instance, one call at a time.
unsafe impl Sync for State {}

static STATE: State = State(UnsafeCell::new(Held { chunks: None, listing: Vec::new() }));

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
pub extern "C" fn dp_decode(name_ptr: u32, name_len: u32, in_ptr: u32, in_len: u32, out_ptr: u32, out_len: u32) -> i64 {
    if (name_ptr == 0 && name_len != 0) || (in_ptr == 0 && in_len != 0) || (out_ptr == 0 && out_len != 0) || out_len > i32::MAX as u32 {
        return refusal::BAD_ARGUMENTS as i64;
    }
    let name: &[u8] = if name_len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(name_ptr as usize as *const u8, name_len as usize) } };
    let input: &[u8] = if in_len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(in_ptr as usize as *const u8, in_len as usize) } };
    let output: &mut [u8] = if out_len == 0 { &mut [] } else { unsafe { std::slice::from_raw_parts_mut(out_ptr as usize as *mut u8, out_len as usize) } };
    if name != b"oodle" {
        output.fill(0);
        return refusal::UNSUPPORTED_CODEC as i64;
    }
    let chunks = unsafe { (*STATE.0.get()).chunks.get_or_insert_with(block_chunks::BlockChunks::new) };
    decoder::decompress(chunks, input, output, out_len as i32)
}

#[no_mangle]
pub extern "C" fn dp_list(container_len: i64) -> i64 {
    let held = unsafe { &mut *STATE.0.get() };
    held.listing = Vec::new();
    if container_len < 0 {
        return refusal::BAD_ARGUMENTS as i64;
    }
    match lines::list_pak(&host::Container::new(container_len)) {
        Ok(listing) => {
            held.listing = listing;
            held.listing.len() as i64
        }
        Err(refused) => refused.0,
    }
}

#[no_mangle]
pub extern "C" fn dp_output() -> u32 {
    unsafe { (*STATE.0.get()).listing.as_ptr() as usize as u32 }
}

/// What this module answers, in its own words, at dp_output until the next dp_list or dp_describe.
#[no_mangle]
pub extern "C" fn dp_describe() -> i64 {
    let held = unsafe { &mut *STATE.0.get() };
    held.listing = DESCRIPTION.to_vec();
    held.listing.len() as i64
}
