# AGENTS Instructions

These guidelines apply to the `docs/` directory.

## Documentation structure
- `README.md` provides entry points to external documentation.
- Markdown sources for the documentation site reside in `source/`.

## Development workflow
- Write in Markdown and keep links relative when possible.
- When code examples are updated, run `cargo test`, fix any build issues, and ensure they compile.
- Prefer runnable examples; keep them in `examples/` within the relevant crate when feasible and reference them here.
- Validate links and anchors when changing paths. Optionally use `lychee` with `lychee.toml` at the repo root for link checking.
- Follow the repository root `AGENTS.md` for formatting and general guidance.

## Status and Roadmap
- For up-to-date project status and planned work, consult `status.md` and `roadmap.md` at the repository root.

## Useful commands
- Build docs for a crate locally: `cargo doc -p <crate> --no-deps --open`

## Serialization
- Do not introduce direct `serde`/`serde_json` usage in this crate. Use Norito wrappers for JSON and binary encoding/decoding.
- JSON: `norito::json::{from_*, to_*, json!, Value}`; Binary: `norito::{Encode, Decode}`.
- If Serde interop is unavoidable for external types, isolate it behind Norito and avoid adding new serde dependencies.
