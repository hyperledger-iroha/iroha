# AGENTS Instructions

These guidelines apply to the `crates/iroha_core` crate.

## Overview
- Core node logic and services; stability and determinism are critical.

## Development workflow
- Avoid panics in library code; return precise errors.
- Keep consensus- and validation-related logic deterministic and well-tested.
- Add unit tests for each new or modified function and black-box tests in `tests/` for workflows.
- Test: `cargo test -p iroha_core` and run subsets as needed.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- If public types change, coordinate updates with `iroha_data_model` and serialization layout stability.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not add direct `serde`/`serde_json` usage in this crate. Use Norito everywhere for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, wrap it behind Norito and avoid introducing new serde dependencies.
