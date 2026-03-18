# AGENTS Instructions

These guidelines apply to the `crates/ivm` directory and supplement `../AGENTS.md` plus the repository root instructions.

## Overview
- IVM is the Iroha Virtual Machine that executes Kotodama smart-contract bytecode (`.to`). Kotodama sources (`.ko`) compile into that bytecode; they do **not** target generic RISC‑V.
- `docs/architecture_spec.md`, `docs/opcodes.md`, and `docs/gpu_offloading.md` are the canonical references for ISA, syscall surface, and acceleration plans—keep them aligned with code changes.

## Layout hints
- `src/` – core VM (decoder, instruction set, gas, host/syscalls, Kotodama frontend, zk helpers, Metal/CUDA bridges, etc.).
- `tests/` – behavioural, ABI, pointer-type, SIMD, zk, and syscall goldens. Long-running suites are guarded by `#[ignore]` or features.
- `examples/` – runnable demos (`cargo run -p ivm --example <name>`).
- `benches/` – Criterion microbenchmarks (`cargo bench -p ivm -- benches::bench_vm`).
- `spec/` – design notes and opcode tables consumed by docs/tests.
- `fuzz/` – `cargo fuzz` targets (e.g., `cargo fuzz run decode_header`) for instruction decoding, parser, and Norito header coverage.
- `cuda/` – kernels built when the `cuda` feature is enabled. The build script honours `IVM_CUDA_NVCC`, `IVM_CUDA_GENCODE`, and `IVM_CUDA_NVCC_EXTRA`.
- `target/prebuilt/` – Kotodama samples produced for integration tests; keep them deterministic because `integration_tests` copies them into `fixtures/ivm`.

## Development workflow
- Keep changes deterministic across hardware. Every SIMD/Metal/CUDA/HTM path must have a byte-for-byte equivalent scalar fallback.
- Features to know:
  - `metal` enables Apple Metal acceleration for SHA256BLOCK and vector helpers.
  - `cuda` builds kernels from `cuda/`. Requires `nvcc`; failures should fall back gracefully.
  - `htm` toggles hardware transactional memory experiments on x86_64.
  - `ml-dsa`, `ivm_vrf_tests`, `ivm_zk_tests` gate heavy cryptography/VRF/ZK suites.
- Run `cargo test -p ivm` for the default set, plus targeted commands when relevant:
  - `cargo test -p ivm --features ivm_zk_tests`
  - `cargo test -p ivm --features ivm_vrf_tests`
  - `cargo test -p ivm --features "cuda metal"`
  - `cargo bench -p ivm benches::bench_vm`
  - `cargo fuzz run decode_header` (from `crates/ivm/fuzz`)
- When adding opcodes/syscalls/pointer types:
  - Update the opcode tables in `docs/opcodes.md` and the helpers in `src/instruction/wide.rs` / `src/encoding`.
  - Refresh syscall/pointer goldens in `tests/abi_syscall_list_golden.rs`, `tests/abi_hash_versions.rs`, and `tests/pointer_type_ids_golden.rs`.
  - Thread new syscalls through the host trait and ensure unknown numbers still yield `VMError::UnknownSyscall`.
- Kotodama diagnostics use `iroha_i18n`. Add new message IDs in `src/kotodama/i18n/mod.rs` and provide translations under `src/kotodama/i18n/translations/`.
- Any new GPU/SIMD path must document the feature flag, include a deterministic fallback, and mention the change in `docs/gpu_offloading.md`.
- Keep README tables current (status of SIMD/Metal/CUDA, VRF gating, etc.) whenever behaviour or requirements change.

## Testing tips
- Unit-test opcodes, syscalls, decoder helpers, and Kotodama passes individually; include negative cases.
- Add roundtrip tests for Kotodama compiler outputs and Norito-encoded program headers.
- Use `tests/abi_hash_versions.rs` to lock down ABI digests—regenerate with `cargo test -p ivm abi_hash_versions -- --nocapture` when intentionally changing the surface.
- Benchmark hot paths before and after SIMD/Metal/CUDA tweaks to confirm regressions.

## Determinism and cross-hardware parity
- Avoid undefined behaviour, unordered floating-point reductions, or instruction sequences that depend on host endianness.
- Guard optional acceleration behind feature flags and detect support at runtime (`is_x86_feature_detected!`, Metal device availability, CUDA presence).
- Keep Kotodama compiler + VM behaviour in sync: regenerate `target/prebuilt/samples/*.to` whenever compiler semantics change and ensure integration tests pick up the new artefacts.

## Tooling tip
- Python tooling in this crate (e.g., docs sync) expects `python3`; if `python` is missing, use `python3`.

## Serialization
- Avoid direct `serde`/`serde_json` usage. Prefer Norito everywhere for JSON and binary encoding/decoding.
- Use `norito::json` helpers for JSON and `norito::{Encode, Decode}` for binary TLVs.
- If unavoidable, keep Serde interop behind Norito wrappers and do not add new serde dependencies.
