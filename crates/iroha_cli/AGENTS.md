# AGENTS Instructions

These guidelines apply to the `crates/iroha_cli` command-line interface.

## Overview
- CLI for interacting with a node.
- Run with `cargo run -p iroha_cli -- --help`.

## Development workflow
- Keep commands idempotent and safe by default; confirm destructive actions explicitly.
- Add tests for argument parsing and output formatting where feasible.
- Test: `cargo test -p iroha_cli` and run subsets as needed.
- Follow root/crates `AGENTS.md` for formatting and linting.

## Notes
- Update examples and usage docs when adding or changing flags or subcommands.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in CLI paths. Prefer Norito wrappers everywhere for JSON and binary encoding/decoding.
- For JSON: use `norito::json` helpers/macros (`from_*`, `to_*`, `json!`, `Value`).
- For binary codecs: use `norito::{Encode, Decode}`. If interoperability with Serde is unavoidable for a third‑party type, isolate it behind Norito wrappers and do not add new direct serde dependencies.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.
