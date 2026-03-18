# AGENTS Instructions

These guidelines apply to the `crates/iroha_crypto` crate.

## Overview
- Cryptographic primitives and utilities; correctness and side-channel resistance are critical.

## Development workflow
- Prefer well-reviewed implementations; avoid writing constant-time logic by hand unless necessary.
- Gate optional hardware acceleration (e.g., SIMD) behind features and keep deterministic, portable fallbacks.
- Add unit tests for new or modified functions; include vector tests when available.
- Test: `cargo test -p iroha_crypto`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- Changes may require updates in dependent crates (e.g., `iroha_core`, `ivm`). Coordinate accordingly.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
