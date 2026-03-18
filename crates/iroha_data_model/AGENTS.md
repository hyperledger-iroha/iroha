# AGENTS Instructions

These guidelines apply to the `crates/iroha_data_model` crate.

## Overview
- Defines core data types and schemas used across the system.
- Norito serialization layout stability must be preserved.

## Development workflow
- Any change to public types requires serialization roundtrip tests and, where relevant, versioning updates.
- Add unit tests for new or modified types and conversions.
- Test: `cargo test -p iroha_data_model`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- Keep schema-generation and derive crates (`*_derive`) in sync.
- Update docs and examples when introducing new types or fields.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
