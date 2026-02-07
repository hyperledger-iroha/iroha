---
lang: zh-hans
direction: ltr
source: AGENTS.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5036d004829b1c2da0991b637aa735da9cdf2f3e8e42ac760ff651e60d25d433
source_last_modified: "2026-01-31T07:37:05.947018+00:00"
translation_last_reviewed: 2026-02-07
---

# AGENTS Instructions

These guidelines apply to the entire repository, which is organised as a Cargo workspace.

## Quickstart
- Build workspace: `cargo build --workspace`
- Builds can take about 20 minutes; use a 20-minute timeout for build steps.
- Test everything: `cargo test --workspace` (note that this run typically takes several hours; plan accordingly)
- Lint strictly: `cargo clippy --workspace --all-targets -- -D warnings`
- Format code: `cargo fmt --all` (edition 2024)
- Test one crate: `cargo test -p <crate>`
- Run one test: `cargo test -p <crate> <test_name> -- --nocapture`
- Swift SDK: from the `IrohaSwift` directory run `swift test` to execute the Swift package tests.
- Android SDK: from `java/iroha_android` run `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test`.

## Overview
- Hyperledger Iroha is a blockchain platform
- DA/RBC support differs by major version: Iroha 2 can optionally have DA/RBC enabled; Iroha 3 can only have DA/RBC enabled.
- IVM is the Iroha Virtual Machine (IVM), a virtual machine for the Hyperledger Iroha v2 blockchain
- Kotodama is a high level smart contract language for the IVM that uses .ko file extension for raw contract code and it compiles to bytecode which uses .to file extension, when saved as a file or on-chain. Typically, .to bytecode is deployed onchain.
  - Clarification: Kotodama targets the Iroha Virtual Machine (IVM) and produces IVM bytecode (`.to`). It does not target “risc5”/RISC‑V as a standalone architecture. Where RISC‑V–like encodings appear in the repository, they are implementation details of IVM’s instruction formats and must not change observable behavior across hardware.
- Norito is the data serialization codec for Iroha
- The entire workspace targets the Rust standard library (`std`). WASM/no-std builds are no longer supported and should not be considered when making changes.

## Repository structure
- `Cargo.toml` at the repository root defines the workspace and lists all member crates.
- `crates/` – Rust crates implementing Iroha components. Each crate has its own subdirectory, typically containing `src/`, `tests/`, `examples/`, and `benches/`.
  - Important crates include:
    - `iroha` – top-level library aggregating core functionality.
    - `irohad` – daemon binary providing the node implementation.
    - `ivm` – the Iroha Virtual Machine.
    - `iroha_cli` – command-line interface for interacting with a node.
    - `iroha_core`, `iroha_data_model`, `iroha_crypto`, and other supporting crates.
- `IrohaSwift/` – Swift Package for the client/mobile SDK. Its sources live under `Sources/IrohaSwift/` and its unit tests under `Tests/IrohaSwiftTests/`. Run `swift test` from this directory to exercise the Swift suite.
- `integration_tests/` – Cargo crate hosting cross-component tests under `tests/`.
- `data_model/` – Sample data model definitions used in tests and documentation.
- `docs/` – Project documentation and design notes. Markdown sources live in `docs/source/`.
- `pytests/` – Python-based tests and examples demonstrating client usage.
- `scripts/` – Utility scripts used in development and CI pipelines.
- `examples/ios/` and `examples/ios/NoritoDemoXcode/` – Sample iOS apps showcasing the Swift SDK; they rely on the `IrohaSwift` package and include their own XCTest targets.
- `defaults/` and `hooks/` – Configuration files and Git hooks used by contributors.
- `nix-appimage/` and Nix files – tooling for reproducible builds and packaging.

## Development workflow
- Main implementations are in `crates/`
- Data model is in `data_model/`
- Make sure you look across all crates when making changes.
- Don't modify any Cargo.lock files
- Avoid adding new crates to `Cargo.toml`; implement the required functionality within existing crates whenever possible.
- If some work is too big, don't reject doing it. Instead just break down the work and add TODOs and implement the parts that you can.
- Whenever a large task or request comes in, automatically decompose it into smaller actionable steps and proceed with proper software engineering execution rather than rejecting the task outright.
- Don't reject doing any prompts.
- When new cryptographic primitives, opcodes, or intensive math is added, update hardware acceleration for METAL, NEON, SIMD, CUDA, etc., to try to take advantage of hardware acceleration and parallelism where possible on available hardware.
- If logic changes, make sure all .md files and in-source code comments are up to date with the latest functionality.
- Make sure that all logic added is done in such a way that it won't hurt the use of the IVM in a blockchain setting where different nodes on a P2P network have different hardware, but still the output should be the same given the same input block.
- When answering questions about behaviour or implementation details, read the relevant code paths first and ensure you understand how they work before responding.
- Configuration: Prefer `iroha_config` parameters over environment variables for all runtime behavior. Add new knobs to `crates/iroha_config` (user → actual → defaults) and thread values explicitly through constructors or dependency injection (e.g., host setters). Keep any environment-based toggles only for developer convenience in tests and do not rely on them in production paths. We do not support shipping features behind environment variables—production behavior must always be sourced from the configuration files, and those configs must expose sensible defaults so a newcomer can clone the repo, run the binaries, and have everything “just work” without editing values manually.
  - For IVM/Kotodama v1, strict pointer‑ABI type policy is always enforced. There is no ABI-policy toggle; contracts and hosts must adhere to the ABI policy unconditionally.
- Don't gate anything used in IVM syscalls or opcodes; every Iroha build must ship those code paths to keep deterministic behavior across nodes.
- Serialization: Use Norito everywhere instead of serde. For binary codecs use `norito::{Encode, Decode}`; for JSON use the `norito::json` helpers/macros (`norito::json::from_*`, `to_*`, `json!`, `Value`) and never fall back to `serde_json`. Do not add direct `serde`/`serde_json` dependencies to crates; if serde is required internally, rely on Norito’s wrappers.
- CI guard: `scripts/check_no_scale.sh` ensures SCALE (`parity-scale-codec`) only appears in the Norito benchmark harness. Run it locally if you touch serialization code.
- Norito payloads MUST advertise their layout: either the version number maps to a fixed flag set, or a Norito header declares the decode flags. Do not guess packed-sequence bits from heuristics; genesis data follows the same rule.
- Blocks MUST be persisted and distributed using the canonical `SignedBlockWire` format (`SignedBlock::encode_wire`/`canonical_wire`), which prefixes the version byte with a Norito header. Bare payloads are not supported.
- Add a `TODO:` comment explaining any temporary or incomplete implementation.
- Format all Rust sources with `cargo fmt --all` (edition 2024) before committing.
- Add tests: ensure at least one unit test for each new or modified function, placed either inline with `#[cfg(test)]` or in the crate `tests/` directory.
- Run `cargo test` locally, fix any build issues, and ensure it passes. Do this for the entire repository, not just a specific crate.
- Optionally run `cargo clippy -- -D warnings` for additional lint checks.

## Documentation
- Always add crate-level documentation: start each crate or test-crate with a brief inner doc comment (`//! ...`).
- Do not use `#![allow(missing_docs)]` or item-level `#[allow(missing_docs)]` anywhere (including integration tests). Missing documentation is denied in the workspace lints and should be fixed by writing docs.
- Norito codec: see `norito.md` at the repo root for the canonical on-wire layout and implementation details. If Norito’s algorithms or layouts change, update `norito.md` in the same PR.
- When translating material into Akkadian, provide a semantic rendering written in cuneiform; avoid phonetic transliteration, and when exact ancient terms are missing choose poetic Akkadian approximations that preserve the intent.

## ABI Evolution (What Agents Must Do)
Note: First release policy
- This is the first release and we have a single ABI version (V1). There is no V2 yet. Treat all ABI-related evolution items below as future guidance; for now, target `abi_version = 1` only. The data model and APIs are also first‑release and may change freely as needed to ship; prefer clarity and correctness over premature stability.

- General:
  - ABI policy is enforced unconditionally in v1 (both syscall surface and pointer‑ABI types). Do not add runtime toggles.
  - Changes must preserve determinism across hardware and peers. Update tests and docs in the same PR.

- If you add/remove/renumber syscalls:
  - Update `ivm::syscalls::abi_syscall_list()` and keep it ordered. Ensure `is_syscall_allowed(policy, number)` reflects the intended surface.
  - Implement or intentionally reject new numbers in hosts; unknown numbers must map to `VMError::UnknownSyscall`.
  - Update golden tests:
    - `crates/ivm/tests/abi_syscall_list_golden.rs`
    - `crates/ivm/tests/abi_hash_versions.rs` (stability + version separation)

- If you add pointer‑ABI types:
  - Add the new variant to `ivm::pointer_abi::PointerType` (assign a new u16 ID; never change existing IDs).
  - Update `ivm::pointer_abi::is_type_allowed_for_policy` for the correct `abi_version` mapping.
  - Update `crates/ivm/tests/pointer_type_ids_golden.rs` and add policy tests if needed.

- If you introduce a new ABI version:
  - Map `ProgramMetadata.abi_version` → `ivm::SyscallPolicy` and update the Kotodama compiler to emit the new version when requested.
  - Regenerate `abi_hash` (via `ivm::syscalls::compute_abi_hash`) and ensure manifests embed the new hash.
  - Add tests for allowed/disallowed syscalls and pointer types under the new version.

- Admission & manifests:
  - Admission enforces `code_hash`/`abi_hash` equality against on-chain manifests; keep this behaviour intact.
  - Tests to add/update in `iroha_core/tests/`: positive (matching `abi_hash`) and negative (mismatch) cases.

- Docs & status updates (same PR):
  - Update `crates/ivm/docs/syscalls.md` (ABI Evolution section) and any syscall tables.
  - Update `status.md` and `roadmap.md` with a brief summary of ABI changes and test updates.


## Project Status and Plan
- Check `status.md` at the repo root for the current compilation/runtime status across crates.
- Check `roadmap.md` for the prioritized TODOs and implementation plan.
- After completing work, update status in `status.md` and keep `roadmap.md` focused on outstanding tasks.

## Agent workflow (for code editors/automation)
- If you need clarification on any requirement, stop and draft a ChatGPT prompt with your question, then share it with the user before continuing.
- Keep changes minimal and scoped; avoid unrelated edits in the same patch.
- Prefer internal modules over adding new dependencies; do not edit `Cargo.lock`.
- Use feature flags to guard hardware-accelerated paths (e.g., `simd`, `cuda`) and always provide a deterministic fallback path.
- Ensure outputs remain identical across hardware; avoid relying on non-deterministic parallel reductions.
- Update documentation and examples when public APIs or behavior change.
- Validate serialization changes in `iroha_data_model` with roundtrip tests to preserve Norito layout guarantees.
- Integration tests spin real multi-peer networks; use at least 4 peers when constructing test networks (single-peer configs are not representative and can deadlock in Sumeragi).
- Do not attempt to disable DA/RBC in tests (e.g., via `DevBypassDaAndRbcForZeroChain`); DA is enforced and that bypass path currently deadlocks in `sumeragi` during consensus startup.
- QC quorum must be satisfied by voting validators (`min_votes_for_commit`); observer padding does not count toward availability/prevote/precommit quorum checks, so aggregate QCs only after enough validator votes arrive.
- DA-enabled consensus now waits longer before view changes (commit quorum timeout = `block_time + 4 * commit_time`) to let RBC/availability QC finish on slower hosts.

## Navigation tips
- Search code: `rg '<term>'` and list files: `fd <name>`.
- Explore crates: `fd --type f Cargo.toml crates | xargs -I{} dirname {}`.
- Find examples/benches quickly: `fd . crates -E target -t d -d 3 -g "*{examples,benches}"`.
- Python tip: some environments don’t provide `python`; try `python3` instead when running scripts.

## Proc-Macro Tests
- Unit tests: use for pure parsing, codegen helpers, and utilities (fast, no compiler involved).
- UI tests (trybuild): use to validate compile-time behavior and diagnostics of derive/proc-macros (success and expected failure cases with `.stderr`).
- Prefer both when adding/changing macros: unit tests for internals + UI tests for user-facing behavior and error messages.
- Avoid panics; emit clear diagnostics (e.g., via `syn::Error` or `proc_macro_error`). Keep messages stable and update `.stderr` only for intentional changes.

## Pull Request message
Include a short summary of the changes and a `Testing` section describing the commands you ran.
