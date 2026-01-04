# AGENTS Instructions

These guidelines apply to the `crates/norito` crate (Iroha serialization codec).

## Overview
- Norito defines canonical serialization for Iroha types.
- Determinism and stable wire layout are critical.

## Development workflow
- Maintain canonical, deterministic encoding; avoid platform-dependent behavior (endianness, locale, float NaNs ordering, etc.).
- Any change to public encoding/decoding requires:
  - Roundtrip tests (encode→decode→equal) for affected types.
  - Golden vector tests: verify bytes against saved fixtures for multiple versions if applicable.
  - Versioning notes and migration guidance when wire format changes.
- Add unit tests for new or modified functions and error paths; prefer property tests where feasible.
- Test: `cargo test -p norito`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- Coordinate changes with `iroha_data_model` and downstream consumers before altering encodings.
- Gate optional features behind flags and provide portable fallbacks.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## CRC64
- Algorithm: CRC64-ECMA (`0x42F0E1EBA9EA3693`) via the `crc64fast` crate.
- Runtime selection: `hardware_crc64` is an alias of `crc64_fallback`; both use the same implementation.
- Determinism: Parity tests ensure consistent outputs across platforms.

### Parity tests
- Files: `crates/norito/tests/crc64.rs`, `crates/norito/tests/crc64_prop.rs`.
- Run: `cargo test -p norito`

### Benchmarks
- File: `crates/norito/benches/crc64.rs`
- Run: `cargo bench -p norito -- benches::bench_crc64`
- Run (portable): `cargo test -p norito`
- x86_64 CLMUL: `RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito`
- aarch64 PMULL: `RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito`

### Benchmarks
- File: `crates/norito/benches/crc64.rs`
  - x86_64: `RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo bench -p norito -- benches::bench_crc64`
  - aarch64: `RUSTFLAGS='-C target-feature=+neon,+aes' cargo bench -p norito -- benches::bench_crc64`

## Performance Notes

- The checksum uses the optimized `crc64fast` implementation.
- Buffer reservations and derive-provided length hints help minimize reallocations during serialization.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
