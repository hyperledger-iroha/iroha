# AGENTS Instructions

These guidelines apply to the `crates/iroha_schema_derive` crate (procedural macros for schema generation).

## Overview
- Provides derive/proc-macro helpers powering `iroha_schema` and related crates.
- Diagnostics should be precise and user-friendly.

## Development workflow
- Avoid panics in macros; prefer rich diagnostics via `syn::Error` or `proc_macro_error`.
- Maintain deterministic codegen; avoid relying on HashMap iteration order without seeding.
- Add tests:
  - Unit tests for parsing and codegen helpers.
  - UI/trybuild tests covering success and expected failures.
- Test: `cargo test -p iroha_schema_derive`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- Coordinate updates with `iroha_schema` and `iroha_schema_gen` to keep generated output consistent.

## UI tests layout
- Preferred approach: use `trybuild` for compile-fail/compile-pass tests.
- Directory structure (example):
  - `tests/ui/pass/*.rs` — should compile successfully.
  - `tests/ui/fail/*.rs` — expected to fail; pair with `.stderr` files.
- Test harness: a single `tests/ui.rs` that runs trybuild on both pass and fail sets.
- Run subset: `cargo test -p iroha_schema_derive ui -- --nocapture`.

See `TEST_TEMPLATES.md` for a minimal harness and sample case layout.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
