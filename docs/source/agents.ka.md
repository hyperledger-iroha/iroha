---
lang: ka
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2025-12-29T18:16:35.916241+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Automation Agent Execution Guide

This page summarizes the operational guardrails for any automation agent
working inside the Hyperledger Iroha workspace. It mirrors the canonical
`AGENTS.md` guidance and the roadmap references so build, documentation, and
telemetry changes all look the same whether they were produced by a human or
an automated contributor.

Each task is expected to land deterministic code plus matching docs, tests,
and operational evidence. Treat the sections below as a ready reference before
touching `roadmap.md` items or replying to behaviour questions.

## Quickstart Commands

| Action | Command |
|--------|---------|
| Build the workspace | `cargo build --workspace` |
| Run the full test suite | `cargo test --workspace` *(typically takes several hours)* |
| Run clippy with deny-by-default warnings | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format Rust code | `cargo fmt --all` *(edition 2024)* |
| Test a single crate | `cargo test -p <crate>` |
| Run one test | `cargo test -p <crate> <test_name> -- --nocapture` |
| Swift SDK tests | From `IrohaSwift/`, run `swift test` |

## Workflow Fundamentals

- Read the relevant code paths before answering questions or changing logic.
- Break large roadmap items into tractable commits; never reject work outright.
- Stay inside the existing workspace membership, reuse internal crates, and do
  **not** alter `Cargo.lock` unless explicitly instructed.
- Use feature flags and capability toggles only where mandated by hardware
  accelerators; keep deterministic fallbacks available on every platform.
- Update documentation and Markdown references alongside any functional change
  so docs always describe current behaviour.
- Add at least one unit test for every new or modified function. Prefer inline
  `#[cfg(test)]` modules or the crate’s `tests/` folder depending on scope.
- After finishing work, update `status.md` with a short summary and reference
  relevant files; keep `roadmap.md` focused on items that still need work.

## Implementation Guardrails

### Serialization & Data Models
- Use the Norito codec everywhere (binary via `norito::{Encode, Decode}`,
  JSON via `norito::json::*`). Do not add direct serde/`serde_json` usage.
- Norito payloads must advertise their layout (version byte or header flags),
  and new formats require corresponding documentation updates (e.g.,
  `norito.md`, `docs/source/da/*.md`).
- Genesis data, manifests, and networking payloads should remain deterministic
  so two peers with the same inputs produce identical hashes.

### Configuration & Runtime Behaviour
- Prefer knobs living in `crates/iroha_config` over new environment variables.
  Thread values explicitly through constructors or dependency injection.
- Never gate IVM syscalls or opcode behaviour—ABI v1 ships everywhere.
- When new config options are added, update defaults, docs, and any related
  templates (`peer.template.toml`, `docs/source/configuration*.md`, etc.).

### ABI, Syscalls, and Pointer Types
- Treat ABI policy as unconditional. Adding/removing syscalls or pointer types
  requires updating:
  - `ivm::syscalls::abi_syscall_list` and `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` plus the golden tests
  - `crates/ivm/tests/abi_hash_versions.rs` whenever the ABI hash changes
- Unknown syscalls must map to `VMError::UnknownSyscall`, and manifests must
  retain signed `abi_hash` equality checks in admission tests.

### Hardware Acceleration & Determinism
- New cryptographic primitives or heavy math must ship hardware-accelerated
  paths (METAL/NEON/SIMD/CUDA) while maintaining deterministic fallbacks.
- Avoid non-deterministic parallel reductions; priority is identical outputs on
  every peer even when hardware differs.
- Keep the Norito and FASTPQ fixtures reproducible so SRE can audit fleet-wide
  telemetry.

### Documentation & Evidence
- Mirror any public-facing doc change in the portal (`docs/portal/...`) when
  applicable so the docs site stays current with the Markdown sources.
- When new workflows are introduced, add runbooks, governance notes, or
  checklists explaining how to rehearse, rollback, and capture evidence.
- When translating content into Akkadian, provide semantic renderings written
  in cuneiform rather than phonetic transliterations.

### Testing & Tooling Expectations
- Run the relevant test suites locally (`cargo test`, `swift test`,
  integration harnesses) and document the commands in the PR testing section.
- Keep CI guard scripts (`ci/*.sh`) and dashboards in sync with new telemetry.
- For proc-macros, pair unit tests with `trybuild` UI tests to lock diagnostics.

## Ready-to-Ship Checklist

1. Code compiles and `cargo fmt` produced no diffs.
2. Updated docs (workspace Markdown plus portal mirrors) describe the new
   behaviour, new CLI flags, or config knobs.
3. Tests cover every new code path and fail deterministically when regressions
   appear.
4. Telemetry, dashboards, and alert definitions reference any new metrics or
   error codes.
5. `status.md` includes a short summary referencing the relevant files and
   roadmap section.

Following this checklist keeps roadmap execution auditable and ensures every
agent contributes evidence that other teams can trust.
