# Norito

This crate bundles the Norito serialization core along with optional
procedural macros. Norito includes a CRC64-XZ integrity checksum with
portable and hardware-accelerated implementations (x86_64 CLMUL, aarch64 PMULL).

## Derive macros

The companion crate `norito_derive` houses Norito's procedural macros and is
compiled with `proc-macro = true`. Rust requires procedural macro crates to be
pure macro packages; they cannot also expose normal library items. For that
reason `norito_derive` must remain separate and the macros cannot live directly
inside `norito`.

`norito` depends on `norito_derive` optionally and re-exports its macros when
the `derive` feature is enabled. By default, `norito` enables `derive`,
`compression`, `columnar`, `json`, `json-std-io`, and `strict-safe`. If you
disable default features, add back what you need, e.g.:

```toml
[dependencies]
# Enable derive macros only (no compression)
norito = { version = "*", default-features = false, features = ["derive"] }
# Or: enable compression without derives
# norito = { version = "*", default-features = false, features = ["compression"] }
```

### Field attributes

The derive macros understand additional field-level attributes to customise
serialization:

- `#[norito(rename = "other")]` renames the field for the serialized
  representation. While the current binary format does not store field names,
  this allows schemas to remain compatible across languages.
- `#[norito(skip)]` omits the field entirely. During deserialization the field
  is filled with [`Default::default`].
- `#[norito(default)]` behaves similarly to `skip`, using
  [`Default::default`] when reading the value.

These attributes apply to both struct fields and enum variants.

## Benchmarks

The `benches/` directory contains a Criterion-based comparison between
Norito and `parity-scale-codec`. On a sample data structure the
following results were observed:

| Benchmark                | Time      |
|--------------------------|-----------|
| `norito_encode`          | 6.46 µs |
| `norito_encode_compressed` | 31.18 µs |
| `scale_encode`           | 28.66 ns |
| `scale_encode_compressed` | 24.78 µs |

Run `cargo bench -p norito` to reproduce these numbers.

Additional micro-benchmarks:
- `ncb_sink_vs_vec`: compares NCB encoders using a simple `Vec<u8>` growth pattern vs the internal `ByteSink` with aligned growth and coalesced writes.
  Run: `cargo bench -p norito --bench ncb_sink_vs_vec`.

## Optional features

- `packed-seq`: Use packed layouts for variable-sized collections (Vec, maps, sets). Reduces per-element overhead by writing a compact offset table followed by a contiguous data segment.
- `packed-struct`: Hybrid packed-struct layout for derives. With `compact-len`, emits a bitset of fields that require explicit sizes, then only those size varints, then the data block.
- `compact-len`: Use a compact varint encoding for length prefixes (for strings, option/result payloads, and per-element collection layouts). Can be combined with or without `packed-seq`.
- `columnar`: Enables experimental Norito Column Blocks (NCB) for adaptive columnar encodings used in scan-heavy paths.
- `strict-safe`: Catch panics inside fallible decode and return `Error::DecodePanic`; prefer slice-based parsing with bounds checks.
- `simd-accel`: Enable SIMD-accelerated CRC64 selection when the CPU supports it (deterministic; same output across hardware).
- `json`: Enable the native JSON parser/writer (`norito::json::{to_json, to_json_fast, from_json, from_json_fast}`), typed derives, DOM helpers, and streaming IO adapters.
- `json-std-io`: Add std::io streaming helpers (`norito::json::{to_writer, from_reader}`) on top of the native JSON stack.
- `metal-stage1`: Enable experimental Metal Stage‑1 JSON structural indexer (macOS/aarch64 only). Falls back to scalar if unavailable.
- `metal-stage2`: Enable experimental Metal Stage‑2 classifier (macOS/aarch64 only). Annotates the structural tape with cached character kinds via the optional `libjsonstage2_metal.dylib` helper and falls back to scalar metadata when unavailable.
- `cuda-stage1`: Enable experimental CUDA Stage‑1 JSON structural indexer (attempts to load on Windows/macOS/Linux). Falls back to scalar if unavailable.
- `metal-crc64` / `cuda-crc64`: Offload CRC64 to Metal/CUDA when payloads exceed the GPU cutoff (default 192 KiB, override with `NORITO_GPU_CRC64_MIN_BYTES`), falling back to SIMD/CPU deterministically.
- `stage1-validate`: Debug-only validator for Stage‑1 accelerated backends. When enabled in debug builds, compares accelerated tapes against the scalar reference for inputs up to 256 KiB and falls back on mismatch. Adds a small overhead in debug builds only.

## Compatibility

Norito uses explicit header flags for layout selection. Header-framed helpers
validate the header (major/minor) and apply the header flag byte as the
authoritative layout selection; unknown bits are rejected. Bare decoders
(`codec::Decode`) are internal-only for hashing/bench scenarios and always use
the fixed v1 default flags (`0x00`) with no heuristics.

Decoding details:
- `from_compressed_bytes` returns an owning `ArchivedBox<T>` so archived payload
  bytes remain valid for the lifetime of the wrapper. It enforces the header
  length during decompression and rejects overlong outputs.
- `ArchiveView::decode` enforces the header schema hash; use
  `ArchiveView::decode_unchecked` only for raw inspection tools.

## Adaptive Acceleration (CPU/GPU) and Layout

Norito auto-detects available hardware and applies heuristics to pick the best
path while keeping outputs deterministic:

- `to_bytes_auto(&T) -> Vec<u8>`: adaptively selects compression (None/CPU zstd/GPU zstd)
  based on payload size and hardware availability. The on-wire format remains the
  same; only the `compression` header byte differs. Layout flags remain the v1
  defaults unless explicitly set and framed by the caller.
- Headerless (bare) codec: `norito::codec::encode_adaptive(&T) -> Vec<u8>` and
  `norito::codec::decode_adaptive::<T>(&[u8])` use the fixed v1 layout
  (`flags = 0x00`). Use `encode_with_header_flags` and
  `norito::core::frame_bare_with_header_flags` when you must preserve explicit layout
  flags alongside bare payloads.
- CPU SIMD: CRC64 uses CLMUL (x86_64) or PMULL (aarch64) when available; JSON string escaping uses AVX2/NEON fast paths when supported, with safe scalar fallbacks.
- GPU CRC64: with `metal-crc64`/`cuda-crc64`, `hardware_crc64` attempts the GPU helper for payloads above the configured cutoff (default 192 KiB via `NORITO_GPU_CRC64_MIN_BYTES`, helper path override via `NORITO_CRC64_GPU_LIB`), then falls back to SIMD/CPU.
- GPU compression: with the `gpu-compression` feature, zstd is offloaded to Metal (macOS/aarch64) or CUDA (other platforms) when a backend is present; falls back to CPU otherwise. Norito looks for `libgpuzstd_metal.dylib` on macOS/aarch64, `libgpuzstd_cuda.so` on Unix platforms, and `gpuzstd_cuda.dll` on Windows when the CUDA path is enabled. On Windows the loader locks DLL resolution down to `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)` and then calls `LoadLibraryExW`. Place a trusted `gpuzstd_cuda.dll` alongside the Norito binaries or in `%SystemRoot%\System32` (or add an explicit directory via `AddDllDirectory`) so it is picked up by the restricted search path. Failures to load a GPU helper emit a warning on stderr and automatically fall back to the CPU backend.
- Stage‑2 metadata: enabling `metal-stage2` attempts to load `libjsonstage2_metal.dylib` (falling back to scalar metadata if missing) to cache structural character kinds alongside the Stage‑1 tape.
- Stage‑1 GPU threshold: Stage‑1 builders (Metal/CUDA) are gated by a 192 KiB default cutoff to amortize launch overhead; override with `NORITO_STAGE1_GPU_MIN_BYTES`. Scalar/CPU SIMD paths remain the fallback and produce identical tapes.
- Heuristics (defaults): skip compression for payloads < 2 KiB; prefer GPU for ≥ 1 MiB; use zstd level 1 for 2–128 KiB and level 3 for larger CPU payloads. These are conservative defaults and can be overridden programmatically for benchmarking.

Determinism: All accelerated paths produce bit-for-bit identical bytes to their portable counterparts. Hardware differences only affect performance, never encoded content.

## Header Format

Every Norito payload begins with a compact header followed by the archived payload bytes. The header layout is:

- magic: 4 bytes, ASCII `NRT0`
- version: 1 byte major, 1 byte minor (current: 0.0; v1 minor is fixed to 0x00)
- schema_hash: 16 bytes (type-name based by default; structural with `schema-structural`)
- compression: 1 byte (`0 = None`, `1 = Zstd`)
- length: 8 bytes little-endian (uncompressed payload length in bytes)
- checksum: 8 bytes little-endian (CRC64-XZ of the uncompressed payload)
- flags: 1 byte (feature/layout flags)

Header size is 40 bytes. After header validation, decoders verify the checksum and reconstruct the value.

## Decode Flags and Feature Bits

The trailing header byte encodes feature/layout flags for the payload and is applied to the decoder context for the duration of decoding.

- `PACKED_SEQ (0x01)`: Variable-length collections (e.g., `Vec<T>`) use a packed layout:
  - `[len:u64][(len+1)*u64 offsets][data...]`
  - Offsets start at 0, are monotonic, and the last offset equals the size of the contiguous data segment.

- `COMPACT_LEN (0x02)`: Per-value length prefixes (string-like values, element payloads, blobs) are varint-encoded instead of fixed `u64` prefixes.

- `PACKED_STRUCT (0x04)`: Derive-generated structs use packed layout. With `COMPACT_LEN`, derives emit a compact bitset of which fields carry explicit sizes, then only those sizes and the data block (no redundant per-field headers).

- `VARINT_OFFSETS (0x08)`: Packed sequences (and packed-struct) encode element/field sizes as varint-coded sizes rather than `(len+1)` 64-bit offsets.

- `COMPACT_SEQ_LEN (0x10)`: Sequence length headers are varint instead of fixed `u64`.

- `FIELD_BITSET (0x20)`: Hybrid packed-struct encodes a bitset selecting fields that carry explicit sizes.

Flags are set explicitly by the encoder and recorded in the header. The
default v1 helpers (`to_bytes`, `to_compressed_bytes`, `to_bytes_auto`) emit
`flags = 0x00` unless you opt into a layout and frame those bytes with the
corresponding header flags.

Flag scoping:
- `COMPACT_LEN` affects per-value length prefixes only.
- `COMPACT_SEQ_LEN` affects only the outer sequence length header.
- `VARINT_OFFSETS` affects only packed-sequence offsets.

## Error Mapping

Decode helpers return structured errors instead of panicking in common malformed cases:

- `InvalidMagic`, `UnsupportedVersion`, `UnsupportedCompression` — header validation
- `LengthMismatch` — premature EOF or inconsistent length accounting
- `ChecksumMismatch` — CRC64 does not match payload
- `SchemaMismatch` — schema hash differs from the expected type
- `InvalidUtf8`, `InvalidTag`, `InvalidNonZero` — malformed value encodings
- `DecodePanic` — when `strict-safe` is enabled, panics inside `try_deserialize` are caught and mapped to this error

Top-level helpers (`decode_from_bytes`, `decode_from_reader`) validate the header and checksum and scope decode flags before reconstructing the value.

## CRC64-XZ (ECMA-182)

Norito computes a CRC64-XZ checksum of each payload using the ECMA-182
polynomial `0x42F0_E1EB_A9EA_3693` in reflected form with init/xor all ones via
the [`crc64fast`](https://crates.io/crates/crc64fast) crate. The
`norito::hardware_crc64` and `norito::crc64_fallback` APIs are aliases that
provide the same optimized implementation on all platforms.

Parity tests in `crates/norito/tests` ensure the checksum is stable across
platforms.
`0x42F0_E1EB_A9EA_3693`.

- Portable paths: bitwise fallback and slicing-by-8 table updates (reflected).
- x86_64: uses PCLMULQDQ (carry‑less multiply) with Barrett reduction when
  available, processing 8 bytes per step with a 128→64 reduction. Selected at
  runtime via `std::arch::is_x86_feature_detected!("pclmulqdq")` and compiled
  under `target-feature=+pclmulqdq`.
- aarch64: uses PMULL similarly when available, detected via
  `std::arch::is_aarch64_feature_detected!("pmull")` and compiled under
  `target-feature=+aes`.
- API: `norito::hardware_crc64` chooses the best implementation at runtime;
  `norito::crc64_fallback` provides the portable reference. For explicit
  variants, `norito::crc64_sse42` and `norito::crc64_neon` are exported on
  compatible targets.

Parity tests in `crates/norito/tests` ensure the accelerated paths exactly
match the fallback across random windows and known vectors.

Tip: to force-enable the intrinsics locally when benchmarking, pass
`RUSTFLAGS`, for example:

```bash
# x86_64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo bench -p norito -- benches::bench_crc64
# aarch64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo bench -p norito -- benches::bench_crc64
```


## Parity Tests and Benches

- Parity tests: see `crates/norito/tests/crc64.rs` and `crates/norito/tests/crc64_prop.rs`.
  Run with `cargo test -p norito`.

- Benchmarks: `crates/norito/benches/crc64.rs` via `cargo bench -p norito -- benches::bench_crc64`.

  - Run portable: `cargo test -p norito`
  - x86_64 CLMUL: `RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito`
  - aarch64 PMULL: `RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito`

- Benchmarks: `crates/norito/benches/crc64.rs`
  - x86_64: `RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo bench -p norito -- benches::bench_crc64`
  - aarch64: `RUSTFLAGS='-C target-feature=+neon,+aes' cargo bench -p norito -- benches::bench_crc64`

Benches produce an optional size/throughput summary when `NORITO_BENCH_SUMMARY=1` is set. For example:

```
NORITO_BENCH_SUMMARY=1 cargo bench -p norito --bench parity_compare
```

This writes a machine-readable summary to `crates/norito/benches/artifacts/size_summary.json` comparing SCALE vs Norito (bare/headered/zstd).

## JSON Helpers and Benchmarks

With the `json` feature enabled, Norito exposes native typed helpers:

```rust
use norito::json::{self, json};

#[derive(
    Debug,
    PartialEq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::derive::FastJson,
    norito::derive::FastJsonWrite,
)]
struct S { a: u32, b: String }

let value = S { a: 1, b: "x".into() };
let compact = json::to_json(&value)?;
let fast = json::to_json_fast(&value)?;
assert_eq!(compact, fast);

let parsed: S = json::from_json(&compact)?;
assert_eq!(value, parsed);

let dom = json::to_value(&value)?;
assert_eq!(dom["a"].as_u64(), Some(1));

// RawValue usage (zero-copy view as JSON text)
use norito::json::value::{to_raw_value, RawValue};
let val = json!({"k":[1,2,3], "s":"t"});
let raw: Box<RawValue> = to_raw_value(&val)?;
let val_back = json::parse_value(raw.get())?;
assert_eq!(val, val_back);

// std::io helpers
use std::io::Cursor;
let mut buf = Vec::new();
json::to_writer_pretty(&mut buf, &value)?;
let parsed_pretty: S = json::from_reader(Cursor::new(&buf))?;
assert_eq!(value, parsed_pretty);

## Telemetry (adaptive encoders and compression)

Norito exposes lightweight counters for adaptive two-pass decisions and compression choices.

- Columnar adaptive (small-N two-pass AoS vs NCB):
  - Snapshot: `norito::columnar::adaptive_metrics_snapshot()`
  - JSON: `norito::columnar::adaptive_metrics_json_string()` (feature `json`)
  - Optional per-pass times: enable `adaptive-telemetry` feature
  - Optional decision logs: enable `adaptive-telemetry-log` (and `adaptive-telemetry` for times)

- Bare codec adaptive (`codec::encode_adaptive` two-pass):
  - Snapshot/JSON: `norito::codec::{adaptive_metrics_snapshot, adaptive_metrics_json_string}`
  - Logs with `adaptive-telemetry-log`

- Headered compression (`core::to_bytes_auto` / `to_compressed_bytes`):
  - Snapshot/JSON: `norito::core::{compression_metrics_snapshot, compression_metrics_json_string}`

Example to print snapshots:

```
cargo run -p norito --example telemetry_dump
```

With per-pass timings in snapshots and decision logs:

```
cargo run -p norito --example telemetry_dump --features adaptive-telemetry,adaptive-telemetry-log
```
```

Benchmarks comparing Norito JSON encode/decode strategies live in
`benches/json.rs`. Run:

```
cargo bench -p norito --features json --bench json
```


### Typed JSON Fast Path

For hot typed decoding paths, Norito provides a structural‑tape powered fast path that avoids building a DOM and minimizes string work.

- Derive `FastJson` and `FastJsonWrite` on your types.
- Decode with `norito::json::from_json_fast::<T>(input)` and encode with `T::write_json(&mut String)` or the `JsonSerialize` bridge.
- Field dispatch uses a precomputed 64‑bit key hash and `TapeWalker::read_key_hash()`; on collision, `last_key()` guards with a constant‑time string check for correctness.

Example:

```rust
use norito::json::{from_json_fast, JsonSerialize};

#[derive(norito::derive::FastJson, norito::derive::FastJsonWrite, PartialEq, Debug)]
struct Item { id: u64, name: String, tags: Vec<String> }

let input = r#"{"id":7,"name":"alice","tags":["a","b"]}"#;
let v: Item = from_json_fast(input)?; // fast typed decode

let mut out = String::new();
JsonSerialize::json_serialize(&v, &mut out); // fast typed encode
```

Enable feature `crc-key-hash` to use CRC32C for key hashing on supported CPUs; a portable software fallback ensures determinism across hardware.


### Reader & Tokens (zero-copy)

Norito includes a lightweight structural tape and a token `Reader` for fast, zero‑copy JSON scanning. It yields tokens that borrow from the input slice. Strings are not unescaped automatically; use the helper below if you need an owned, unescaped string.

```rust
use norito::json::{Reader, Token, unescape_json_string};

let input = r#"{"name":"al\u0069ce","tags":["a","b"]}"#;
let mut rdr = Reader::new(input);
while let Some(tok) = rdr.next().unwrap() {
    match tok {
        Token::KeyBorrowed(k) => {
            let key = unescape_json_string(k).unwrap();
            // use key
        }
        Token::StringBorrowed(s) => {
            let val = unescape_json_string(s).unwrap();
            // use val
        }
        _ => {}
    }
}
```

Key hashing utilities (`read_key_hash`) operate on the logical characters of keys (including `\uXXXX` sequences and surrogate pairs), hashing the resulting UTF‑8 bytes with FNV‑1a for stable map key matching.

#### Supported Escapes

Norito’s JSON string handling supports the standard escape sequences and validates them when parsing/unescaping:

- `\"` — double quote
- `\\` — backslash
- `\/` — forward slash
- `\b` — backspace (U+0008)
- `\f` — form feed (U+000C)
- `\n` — line feed (U+000A)
- `\r` — carriage return (U+000D)
- `\t` — tab (U+0009)
- `\uXXXX` — 4‑hex‑digit Unicode code unit
  - Surrogate pairs are supported: a high surrogate `\uD800..\uDBFF` must be immediately followed by a low surrogate `\uDC00..\uDFFF`, and are combined to a single Unicode scalar.

Error cases returned by the helpers include invalid hex digits, unexpected low surrogates, missing low surrogates after a high surrogate, and control characters (`< 0x20`) appearing unescaped.

### Stage‑1 Backend Selection (JSON)

When building a structural index for JSON (`norito::json::build_struct_index`), Norito opportunistically uses the fastest available backend and falls back to a portable scalar implementation:

- `metal-stage1` (macOS/aarch64): dynamic Metal helper if present.
- `cuda-stage1` (Windows/macOS/Linux): dynamic CUDA helper if present.
- `simd-accel` + `aarch64`: NEON backend (with runtime detection); scalar fallback.
- `x86_64`: AVX2 backend with runtime detection; scalar fallback when AVX2 is not available.
- Scalar reference (always available and used for correctness validation in debug builds when `stage1-validate` is enabled).

### Typed Auto Fast‑Path

Use `from_json_auto::<T>(input)` to automatically choose between the generic typed parser and the structural‑tape fast path:

```rust
use norito::json::from_json_auto;

#[derive(norito::derive::FastJson, norito::derive::FastJsonWrite, PartialEq, Debug)]
struct Item { id: u64, name: String, flag: bool }

// Provide the generic typed parser for fallback on small inputs
impl norito::json::JsonDeserialize for Item {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::Error> {
        // ... parse fields using the small typed Parser ...
        # let _ = p; unimplemented!()
    }
}

let s = r#"{"id":7,"name":"alice","flag":true}"#;
let item: Item = from_json_auto(s)?;
```

Current behavior:
- For small inputs (<= 256 bytes): prefers the generic typed parser to avoid tape overhead.
- For larger inputs: uses the `FastFromJson` structural‑tape path.

Note: This initial API requires `T` to implement both `FastFromJson` and `JsonDeserialize` to keep the selection stable on stable Rust without specialization.

Debug builds print the selected backend to stderr (e.g., `norito/json: stage1 backend = cuda`). Enable `stage1-validate` to verify accelerated results against scalar for inputs up to 256 KiB in debug builds.

When `stage1-validate` is active in debug builds, Norito emits a one-time banner to stderr indicating that validation is enabled to reduce log noise.

#### Accelerator Troubleshooting

- Make sure the helper library is discoverable by the dynamic loader:
  - The CUDA and Metal stage-1 helpers are built from the sources in
    `crates/norito/accelerators/jsonstage1_cuda` and
    `crates/norito/accelerators/jsonstage1_metal`.
  - Linux: add the folder to `LD_LIBRARY_PATH` (e.g., `export LD_LIBRARY_PATH=$PWD:$LD_LIBRARY_PATH`) and use `libjsonstage1_cuda.so`.
  - macOS: add the folder to `DYLD_LIBRARY_PATH` (e.g., `export DYLD_LIBRARY_PATH=$PWD:$DYLD_LIBRARY_PATH`). CUDA tries `.dylib` first, then `.so`. Metal looks for `libjsonstage1_metal.dylib` and, when `metal-stage2` is enabled, `libjsonstage2_metal.dylib`.
  - Windows: ensure `jsonstage1_cuda.dll` is on `PATH` or alongside the binary.
- In debug builds, Norito prints which backend was selected (e.g., `norito/json: stage1 backend = cuda`). If it always says `scalar`, the helper library likely wasn’t found or declined the input.
- To cross-check accelerator correctness during development, enable validation: `--features stage1-validate` and build with debug assertions (`RUSTFLAGS='-C debug-assertions=yes'`).

## Encoded Length Hints

Norito derives now implement an optional `encoded_len_hint(&self) -> Option<usize>` method:

- Purpose: allow the encoder to pre‑reserve buffer capacity, reducing reallocations and copies during serialization.
- Behavior: hints are exact or tight upper bounds when cheap to compute; returning `None` lets the encoder fall back to a conservative default.
- Coverage: built‑in primitives and common containers return accurate hints; derive code sums field hints (plus per-field length prefixes in per-element layouts). Packed sequence layouts compute `len + offsets + data`.
- Usage: automatic — `to_bytes()` calls `encoded_len_hint()` internally; no user action required.

## Exact Encoded Length

For faster serialization without reallocations, Norito adds `encoded_len_exact(&self) -> Option<usize>` on `NoritoSerialize`:

- Returns the precise number of bytes that `serialize()` will write for the value (payload only).
- Implemented for primitives, strings/`&str`/`Box<str>`, `Option<T>`, `Result<T,E>`, arrays `[T; N]`, and `Vec<T>` (packed‑seq), and is derived for structs/enums by summing field exact sizes plus their per‑field length prefixes.
- `to_bytes()` and `to_compressed_bytes()` now prefer `encoded_len_exact()` and fall back to `encoded_len_hint()` when unavailable, improving buffer preallocation and reducing copies.
