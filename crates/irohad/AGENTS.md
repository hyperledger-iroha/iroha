# AGENTS Instructions

These guidelines apply to the `crates/irohad` daemon (node binary).

## Overview
- `irohad` is the node process that runs consensus, networking, and execution.
- Binary target; use `cargo run -p irohad -- --help` to see CLI options.

## Development workflow
- Keep changes deterministic and side-effect free under test; avoid time- and network-dependent behavior in unit tests.
- Test: `cargo test -p irohad` and optionally run a subset: `cargo test -p irohad <name> -- --nocapture`.
- Follow root/crates `AGENTS.md` for formatting, linting, and dependency policy.

## Notes
- Public configuration changes should be documented and examples updated under `docs/` or crate README.
- When touching networking or P2P components, coordinate changes with `iroha_p2p` and `iroha_core` crates.

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
