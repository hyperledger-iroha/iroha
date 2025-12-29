# AGENTS Instructions

These guidelines apply to the `crates/norito_derive` crate (procedural macros for Norito).

## Overview
- Provides derive/proc-macro helpers for the Norito serialization codec.
- Errors must be clear and point to the right spans.

## Development workflow
- Avoid panics; return `syn::Error`/`proc_macro_error` diagnostics with helpful messages.
- Keep macro-generated code deterministic and stable across Rust versions where possible.
- Add tests:
  - Unit tests for helpers and parsing.
  - UI/trybuild tests for compile-time success and failure cases.
- Test: `cargo test -p norito_derive`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- Coordinate changes with `norito` and `iroha_data_model` to keep derives aligned with encoding rules.

## UI tests layout
- Prefer `trybuild` for compile-time behavior checks of the proc-macros.
- Structure:
  - `tests/ui/pass/*.rs` — cases that must compile.
  - `tests/ui/fail/*.rs` — cases that must fail with expected diagnostics (`.stderr`).
- Harness: `tests/ui.rs` that invokes trybuild across both sets.
- Filter a single case: `cargo test -p norito_derive ui -- --nocapture --ignored <name>` or use trybuild filters.

See `TEST_TEMPLATES.md` for a minimal harness and samples.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
