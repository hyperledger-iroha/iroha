# AGENTS Instructions

These guidelines apply to the `crates/iroha_schema` crate.

## Overview
- Provides schema definitions/utilities used for describing and validating Iroha data structures.
- Impacts derive macros and code generation in related crates.

## Development workflow
- Update generated schema and document breaking changes.
- Add unit tests for new or modified schema types and conversions.
- When schema output changes, update any fixtures and regenerate examples used in docs if applicable.
- Test: `cargo test -p iroha_schema`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- Coordinate updates with `iroha_schema_derive`, `iroha_schema_gen`, and `iroha_data_model` to keep derivations and outputs consistent.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
