---
lang: az
direction: ltr
source: docs/source/rust_1_92_adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa055cf57d1043162c541730762d1a59c50ba6a2ddcfec53df675c04963dc8c3
source_last_modified: "2026-01-30T12:29:10.195666+00:00"
translation_last_reviewed: 2026-02-07
title: Rust 1.92 Rollout
summary: Workspace-wide checklist for adopting the Rust 1.92 release and its APIs, lints, and tooling updates.
---

# Rust 1.92 Rollout

Rust 1.92.0 (2025-12-11) is now the workspace baseline. The highlights below are
pulled from `$(rustc --print sysroot)/share/doc/rust/html/releases.md` and map
every feature class to concrete tasks so contributors can finish the migration
incrementally.

## Completed in this change

- Toolchain pin: `rust-toolchain.toml` now targets `1.92.0` so CI and local
  builds agree on the exact compiler.
- Minimum supported Rust version: `[workspace.package]` declares
  `rust-version = "1.92"`, ensuring every crate (unless it overrides the field)
  inherits the same MSRV.
- Docs: the SoraFS CI cookbook & template snippets document the new MSRV for
  both GitHub Actions and GitLab.
- Lint gating: `ci/check_rust_1_92_lints.sh` adds the new never-type fallback
  and macro-export lint gates for a focused `cargo check` sweep.
- Lint sweep: ran the 1.92 lint gate and removed dead-code/unused-import hits
  in the RBC chunk broadcaster and Norito RPC fixture tests.
- Std features in use:
  - `NonZero::div_ceil` now backs the operator-auth rate conversion so minute
    budgets map to per-second caps without hand-rolled ceil division
    (`crates/iroha_torii/src/operator_auth.rs`).
- Std `RwLock` sweep: no write→read handoffs required a downgrade; no code
  changes needed.
- Const rotation sweep: no const helpers reimplementing slice rotation remain.

## Task matrix

Legend: DONE = complete, WIP = in progress (patches linked below), TODO = needs
an owner. Reference these items when editing `status.md` or grooming roadmap
entries.

| Area | Highlight | Action | Status | Notes |
|------|-----------|--------|--------|-------|
| Language | `never_type_fallback_flowing_into_unsafe`, `dependency_on_unit_never_type_fallback` deny-by-default | Run the 1.92 lint sweep and fix any new findings | DONE | Lint sweep completed; removed unused RBC chunk helpers + redundant base64 import. |
| Libraries | `RwLockWriteGuard::downgrade` | Replace write-then-read lock handoffs where a downgrade suffices | DONE | Audit complete; no std `RwLock` write→read handoffs found. |
| Libraries | `Location::file_as_c_str` | Use when feeding caller file paths into FFI or logging C APIs | DONE | No `Location::caller()` call sites currently feed C-FFI/logging APIs; keep this in mind when new FFI bridges are added. |
| Libraries | `NonZero::div_ceil` | Adopt in rate/limit conversions that currently rely on `get().div_ceil()` | DONE | Operator auth rate limit conversion updated in this change. |
| Const APIs | `slice::rotate_left`/`rotate_right` const | Audit const helpers that roll their own rotation | DONE | No const rotation helpers remain; no changes needed. |

## Next steps

1. **`Location::file_as_c_str` adoption**: review C-FFI call sites that pass
   `Location::caller()` data and use the new API where appropriate.

Link future PRs to this document so we can tick off rows (convert TODO -> WIP ->
DONE) and keep `status.md` honest about the rollout.
