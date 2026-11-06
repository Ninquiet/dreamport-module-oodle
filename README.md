# Dreamport module: Oodle and Unreal .pak

A fork of [Dreamport-Test/dreamport-module](https://github.com/Dreamport-Test/dreamport-module) that
offers the decoder `oodle` (Kraken, Mermaid, Selkie and Leviathan blocks) and the reader `unrealpak` (every
entry of an Unreal 4 `.pak`, version 10 and later, and where its bytes lie). Both are Rust translations of
Somnium's own C#: the decoder of its clean-room Oodle decoder, checked byte for byte against the official
library's output; the reader of the client's pak reader (`Unpackers/Pak`), checked entry by entry against it.
It is not RAD's or Epic's code.

What follows is the base's description.

---

A **Dreamport module** is code a Dreamport world can ask Somnium to use when it reads a game's files. It is
a WebAssembly module that offers **decoders** (bytes in, bytes out), **readers** (it lists the entries of
one container, such as an Unreal `.pak`), or both, in one `.wasm` with one SHA-256 and one consent. The
client runs it **isolated**: it cannot open files, use the network or see anything of the player's machine.
A reader can read the one container it is listing, through the host, and nothing else.

This repository is the base. **To make a module, fork it**, put your code in `src/`, list the names it
offers in `module.json`, and push a tag of your own name (`v1-lz4`, ...). The release workflow builds `module.wasm` and
publishes it with its SHA-256.

A world's pattern names a module by **its fork, its release tag and the SHA-256 of `module.wasm`**, once per
source, with every name it uses joined by `+`:

```
module=unrealpak+oodle:Owner/repo@tag#sha256
```

The client asks the player before downloading it, keeps one copy per SHA-256, and refuses a file whose hash
differs. A module you change gets a new tag and a new hash; patterns that name the old one keep working.

> Test repository (2026-11): a prototype of the design, not a Somnium service.

## The ABI (version 3)

`module.json`:

```json
{ "abi": 3, "decoders": ["oodle"], "readers": ["unrealpak"] }
```

`decoders` and `readers` are the names a pattern asks for; each must be one Somnium knows. The client checks
that every listed name has its export: `dp_decode` for decoders, `dp_list` for readers. The host runs only
version 3: `dp_abi` must answer 3.

### Exports

| Export | Signature | Meaning |
|---|---|---|
| `memory` | memory | the module's memory |
| `dp_abi` | `() -> i32` | 3 |
| `dp_alloc` | `(len: u32) -> u32` | a block of `len` bytes in the module's memory, 16-byte aligned; 0 when it cannot |
| `dp_free` | `(ptr: u32, len: u32)` | gives back a block from `dp_alloc`, with the same `len` |
| `dp_decode` | `(name_ptr: u32, name_len: u32, in_ptr: u32, in_len: u32, out_ptr: u32, out_len: u32) -> i64` | decodes `in_len` bytes into exactly `out_len` bytes with the decoder named (UTF-8, one of `decoders`); returns `out_len`, or a negative refusal |
| `dp_list` | `(container_len: i64) -> i64` | lists the one container the host has open; returns the listing's length in bytes, or a negative refusal |
| `dp_output` | `() -> i32` | where the listing of the last `dp_list` lies; valid until the next `dp_list` |

`dp_decode`'s input and output blocks must not overlap, and a refused call must leave zeros in the output.
Refusals are negative: -1 corrupt, -2 not supported (a decoder name it does not offer, encrypted, unknown
version, unknown codec), -3 too large, -5 bad arguments. The client gives each call a time limit and caps the module's memory; a module that
traps or runs out of time is treated as a refusal.

### The one import, for readers only

```
(import "dp_host" "read" (func (param i64 i32 i32) (result i32)))   ;; read(offset, dst_ptr, len) -> bytes read or negative
```

- It reads from **the container being listed, and nothing else**: there is no handle, no path and no file
  name, so a module cannot even name another file.
- The host refuses (-5) any read outside `[0, container_len)`, any `len` over 16 MiB, and more than 1 GiB in
  total per `dp_list`. Outside a `dp_list` call it always returns -5.
- A module importing anything else, or importing `dp_host.read` without declaring a reader, is refused before
  it runs. A module used only as a decoder gets an import that always returns -5.

`src/host.rs` wraps it safely: `Container::read` fills a buffer in reads of at most 16 MiB, and
`Container::at` returns `None` for bytes outside the container without asking the host. A module that never
calls it imports nothing.

### The listing (UTF-8 JSON Lines)

The first line describes the container; `indexAt` and `indexLength` are the region the host hashes to tell
one game build from another:

```json
{"archive":{"indexAt":123,"indexLength":456,"mount":"../../../"}}
```

Then one line per entry, **every entry, in the container's own order** (selection stays in the host). The
path is the container's own, before any mount strip; `decoder` is `stored`, `zlib`, `oodle`, ...:

```json
{"path":"Engine/Content/x.uasset","offset":1,"length":2,"method":1,"size":3,"decoder":"oodle","blockSize":65536,"blocks":[100,200]}
```

An entry the reader can name but not locate is `{"path":"...","problem":"..."}`; the host refuses the
container only if the pattern selects that entry.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

The result is `target/wasm32-unknown-unknown/release/dreamport_module.wasm`. Use no crate that needs an
import (no `wasm-bindgen`, no WASI).

## This fork's decoder and reader

`src/lib.rs` offers `oodle`, one Rust file per file of the C# decoder, and `unrealpak` under `src/pak/`, one
Rust file per file of the C# reader it translates (the footer, the primary index, the directory index, the
encoded entries, each entry's record, and locating an entry's bytes). The reader decodes nothing and never
decrypts: an encrypted pak is refused (-2), an encrypted entry is a problem line. It lists every entry; which
ones a world takes, the mount strip and the limits stay in the host.

Measured 2026-11-04 and 2026-11-05 in Wasmtime 48 from .NET 10: the decoder is byte-identical to the
official library on 8,405 valid streams and refuses the same corrupt streams as the C# decoder, at about
560 MiB/s with the client's memory and time limits (0.77x the C# decoder). The reader lists both paks of a
shipped Unreal 4 game (108,755 entries) exactly as the C# reader locates them, in about the same time.
