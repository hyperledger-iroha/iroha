# AGENTS Instructions

These guidelines apply to the `crates/` directory and supplement the repository root instructions.

## Crates layout
- Each subdirectory is a Rust crate that participates in the Cargo workspace.
- Common folders inside each crate include `src/` for implementation code, `tests/` for unit tests, and optional `examples/`, `benches/`, or `docs/`.
- Some crates provide procedural macros or derive helpers while others implement runtime components.

## Development workflow
- Keep crate-specific README files and documentation up to date.
- Avoid modifying `Cargo.lock`.
- Prefer internal modules over adding new dependencies. If a dependency is essential, justify it in the PR description.
- Add tests for each new or modified function. Co-locate with code via `#[cfg(test)]` or use `tests/` for black-box tests.
- Use feature flags for optional behavior (e.g., `std`, `simd`, `cuda`) and keep deterministic fallbacks.
- Run `cargo fmt --all` (edition 2024) and `cargo test --workspace` from the repository root after changes; fix any build issues.
- Optionally run `cargo clippy --workspace --all-targets -- -D warnings` for additional lint checks.

## Documentation
- Missing documentation is denied across the workspace. Do not use `#![allow(missing_docs)]` (or per-item `#[allow(missing_docs)]`) to silence lints. Always add appropriate crate/module/item documentation instead.

## Status and Roadmap
- Before larger changes, review the repository-wide `status.md` and `roadmap.md` at the root to understand current state and priorities.

## Useful commands
- Test one crate: `cargo test -p <crate>`
- Run a binary crate: `cargo run -p <bin_crate> -- <args>`
- Benchmarks: `cargo bench -p <crate>`
- Docs for a crate: `cargo doc -p <crate> --no-deps --open`

## Tooling tip
- Python: if `python` is unavailable, use `python3` to run scripts.

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in crates. Prefer Norito wrappers everywhere for JSON and binary encoding/decoding.
- For JSON: use `norito::json` helpers/macros (`from_*`, `to_*`, `json!`, `Value`).
- For binary codecs: use `norito::{Encode, Decode}`. If interoperability with Serde is unavoidable for a third‑party type, isolate it behind Norito wrappers and do not add new direct serde dependencies.
