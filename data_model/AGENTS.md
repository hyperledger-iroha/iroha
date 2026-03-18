# AGENTS Instructions

These guidelines apply to the `data_model/` directory.

## Overview
- Contains sample data model definitions used in tests and documentation.

## Development workflow
- Keep examples synchronized with `iroha_data_model` and update tests accordingly.
- Validate serialization with roundtrip tests where feasible.
- Follow the repository root `AGENTS.md` for testing and formatting.


## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
