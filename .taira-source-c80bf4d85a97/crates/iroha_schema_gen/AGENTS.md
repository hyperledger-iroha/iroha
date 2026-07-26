# AGENTS Instructions

These guidelines apply to the `crates/iroha_schema_gen` crate (schema generation utilities/tools).

## Overview
- Generates or manipulates schema definitions used across Iroha.
- May include a binary for codegen; check `Cargo.toml` targets.

## Development workflow
- Keep outputs deterministic and stable across runs; avoid nondeterministic ordering.
- When generator CLI flags change, update documentation and examples.
- Add tests for generation helpers and, where feasible, golden-file tests for generated outputs.
- Test: `cargo test -p iroha_schema_gen`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Useful commands
- Run the generator: `cargo run -p iroha_schema_gen -- --help`

## Notes
- Coordinate with `iroha_schema`, `iroha_schema_derive`, and `iroha_data_model` to ensure generated schema matches expectations.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
