# Dreamport module: Oodle

A fork of [Dreamport-Test/dreamport-module](https://github.com/Dreamport-Test/dreamport-module) that
offers the decoder `oodle` (Kraken, Mermaid, Selkie and Leviathan blocks). It is a Rust translation of
Somnium's own clean-room Oodle decoder (C#), written from that decoder only and checked byte for byte
against the official library's output. It is not RAD's or Epic's code.

What follows is the base's description.

---

A **Dreamport module** is a decoder that a Dreamport world can ask Somnium to use when it reads a game's
files: bytes go in, bytes come out. It is a WebAssembly module, and the client runs it **isolated**: it
imports nothing, so it cannot open files, use the network or see anything of the player's machine.

This repository is the base. **To make a module, fork it**, put your decoder in `src/`, list the decoder
names it offers in `module.json`, and push a tag (`v1`, `v2`, ...). The release workflow builds
`module.wasm` and publishes it with its SHA-256.

A world's pattern names a module by **its fork, its release tag and the SHA-256 of `module.wasm`**. The
client asks the player before downloading it, keeps one copy per SHA-256, and refuses a file whose hash
differs. A module you change gets a new tag and a new hash; patterns that name the old one keep working.

> Test repository (2026-11): a prototype of the design, not a Somnium service.

## The ABI (version 1)

The module exports `memory` and four functions, and **imports nothing** (the client refuses a module
with any import):

| Export | Signature | Meaning |
|---|---|---|
| `dp_abi` | `() -> u32` | 1 |
| `dp_alloc` | `(len: u32) -> u32` | a block of `len` bytes in the module's memory, 16-byte aligned; 0 when it cannot |
| `dp_free` | `(ptr: u32, len: u32)` | gives back a block from `dp_alloc`, with the same `len` |
| `dp_decode` | `(in_ptr: u32, in_len: u32, out_ptr: u32, out_len: u32) -> i64` | decodes `in_len` bytes into exactly `out_len` bytes; returns `out_len`, or a negative refusal |

The input and output blocks must not overlap. A refusal is negative: -1 corrupt, -2 not supported,
-5 bad arguments. A refused call must leave zeros in the output. The client gives each call a time
limit and caps the module's memory; a module that traps or runs out of time is treated as a refusal.

`module.json`:

```json
{ "abi": 1, "decoders": ["example"] }
```

`decoders` are the names a pattern asks for (for example `oodle`).

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

The result is `target/wasm32-unknown-unknown/release/dreamport_module.wasm`. Use no crate that needs
an import (no `wasm-bindgen`, no WASI).

## This fork's decoder

`src/lib.rs` offers `oodle`; one Rust file per file of the C# decoder. Measured 2026-11-04 in Wasmtime
48 from .NET 10: byte-identical to the official library on 8,405 valid streams, refuses the same
corrupt streams as the C# decoder, about 560 MiB/s with the client's memory and time limits (0.77x the
C# decoder).
