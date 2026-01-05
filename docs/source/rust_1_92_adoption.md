---
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
- Std features in use:
  - `NonZero::div_ceil` now backs the operator-auth rate conversion so minute
    budgets map to per-second caps without hand-rolled ceil division
    (`crates/iroha_torii/src/operator_auth.rs`).

## Task matrix

Legend: DONE = complete, WIP = in progress (patches linked below), TODO = needs
an owner. Reference these items when editing `status.md` or grooming roadmap
entries.

| Area | Highlight | Action | Status | Notes |
|------|-----------|--------|--------|-------|
| Language | `never_type_fallback_flowing_into_unsafe`, `dependency_on_unit_never_type_fallback` deny-by-default | Run the 1.92 lint sweep and fix any new findings | TODO | `ci/check_rust_1_92_lints.sh` is ready; run it before merging new unsafe work. |
| Libraries | `RwLockWriteGuard::downgrade` | Replace write-then-read lock handoffs where a downgrade suffices | TODO | Focus on std `RwLock` sites that drop a write guard just to re-acquire read. |
| Libraries | `Location::file_as_c_str` | Use when feeding caller file paths into FFI or logging C APIs | TODO | Consider for any `Location::caller()` -> C string conversions. |
| Libraries | `NonZero::div_ceil` | Adopt in rate/limit conversions that currently rely on `get().div_ceil()` | DONE | Operator auth rate limit conversion updated in this change. |
| Const APIs | `slice::rotate_left`/`rotate_right` const | Audit const helpers that roll their own rotation | TODO | Apply where const eval currently blocks `rotate_*` usage. |

## Next steps

1. **Lint sweep** (`TODO: lint sweep`): run `ci/check_rust_1_92_lints.sh` and
   clear any new deny-by-default findings (never-type fallback, macro export).
2. **Lock downgrade sweep** (`TODO: rwlock downgrade`): scan std `RwLock`
   write-then-read patterns and replace with `RwLockWriteGuard::downgrade` where
   it preserves semantics.
3. **Const rotation sweep** (`TODO: const rotate`): re-check const helpers that
   reimplement rotate logic and migrate to the standard API now that it is
   const-stable.

Link future PRs to this document so we can tick off rows (convert TODO -> WIP ->
DONE) and keep `status.md` honest about the rollout.
