---
lang: am
direction: ltr
source: docs/source/rust_1_91_adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: db492130c10562ba61a4d30fbc132679cb6d3122436e5095600d191d982ffbf0
source_last_modified: "2026-01-05T09:28:12.044166+00:00"
translation_last_reviewed: 2026-02-07
title: Rust 1.91 Rollout
summary: Workspace-wide checklist for adopting the Rust 1.91 release and its APIs, lints, and tooling updates.
---

# Rust 1.91 Rollout

Rust 1.91.0 (2025-10-30) is now the workspace baseline. The highlights below are
pulled from `$(rustc --print sysroot)/share/doc/rust/html/releases.md` and map
every feature class to concrete tasks so contributors can finish the migration
incrementally.

## Completed in this change

- Toolchain pin: `rust-toolchain.toml` now targets `1.91.0` so CI and local
  builds agree on the exact compiler.
- Minimum supported Rust version: `[workspace.package]` declares
  `rust-version = "1.91"`, ensuring every crate (unless it overrides the field)
  inherits the same MSRV.
- Docs: the SoraFS CI cookbook & template snippets document the new MSRV for
  both GitHub Actions and GitLab.
- Std features in use:
  - `Duration::from_mins`/`::from_hours` replace the manual `from_secs(60 * N)`
    math inside `crates/iroha_config/src/parameters/defaults.rs`, shoring up the
    canonical configuration defaults.
  - `PathBuf::add_extension` now backs the deterministic chunk sink in
    `crates/sorafs_car/src/lib.rs` so temp files append `.partial` without
    hand-rolled OsStr juggling.
  - Torii/SoraFS runtime paths (alias cache, ACME automation, quota limits) now
    lean on the human-readable constructors so policy tables are easier to read
    (`crates/sorafs_manifest/src/alias_cache.rs`, `crates/iroha_torii/src/sorafs/*`,
    `crates/iroha_core/src/da/replay_cache.rs`, `crates/sorafs_orchestrator/src/soranet.rs`).

## Task matrix

Legend: DONE = complete, WIP = in progress (patches linked below), TODO = needs
an owner. Reference these items when editing `status.md` or grooming roadmap
entries.

| Area | Highlight | Action | Status | Notes |
|------|-----------|--------|--------|-------|
| Language | `dangling_pointers_from_locals`, `integer_to_ptr_transmutes`, `semicolon_in_expressions_from_macros` are warn-by-default lints | Audit crates with `unsafe` pointer juggling, add crate-level denies once clean | DONE | `ci/check_rust_1_91_lints.sh` (2026‑02‑19) now runs clean across the workspace; the settlement router tests gained the missing `VolatilityBucket` import uncovered by the lint gate. Keep using the script locally before raising new `unsafe` patches. |
| Language | Pattern binding drop-order rules tightened | Verify Kotodama/IVM patterns relying on drop order | DONE | Added a guard test to lock in the observed drop order for destructuring bindings so Kotodama/IVM code doesn’t depend on pre-1.91 behaviour (`crates/ivm/tests/drop_order.rs`). |
| Libraries | `{integer}::strict_*` arithmetic | Evaluate IVM crypto and pow difficulty math; switch to strict ops where panics should occur | DONE | Cursor batching, trigger contract refcounts, and executor bytecode builders now rely on `strict_add`/`strict_mul` so overflow panics instead of silently wrapping (`【crates/iroha_core/src/query/cursor.rs:96】【crates/iroha_core/src/smartcontracts/isi/triggers/set.rs:812】【crates/iroha_core/src/executor.rs:1891】【crates/iroha_core/src/state.rs:8056】`); relay uptime/bandwidth accumulators now use the same guards (`【tools/soranet-relay/src/incentives.rs:20】【tools/soranet-relay/src/incentives.rs:260】`), and the adaptive PoW controller applies `strict_add`/`strict_sub` to the window counters and difficulty adjustments so misconfigurations panic deterministically (`【tools/soranet-relay/src/dos.rs:685】【tools/soranet-relay/src/dos.rs:712】`). |
| Libraries | `PathBuf::{add_extension,with_added_extension}` + string equality impls | Update manifest + artifact builders that manually glue multi-part extensions | DONE | DA replay cursors, PoR persistence, streaming snapshots, RBC status/store writers, Soranet spool files, and the governance publisher’s atomic writer now append `.tmp` via `with_added_extension`, each guarded by dedicated tests; CLI/admission helpers are no longer string-splicing extensions. |
| Libraries | `core::array::repeat`, `core::iter::chain` free fn, `Cell::as_array_of_cells` | Review code that allocates via `vec![x; N].try_into().unwrap()` patterns | DONE | Audit found no remaining `vec![x; N].try_into()` sites across the workspace; no changes required beyond keeping future additions const-friendly. |
| Libraries | `Duration::from_mins`/`::from_hours` | Config defaults migrated (this change); propagate to Torii/Sorafs tests | DONE | Torii connect/tests + SoraFS APIs now use the helpers end-to-end (`crates/iroha_torii/src/{connect.rs,lib.rs,sorafs/**,telemetry/peers/monitor.rs}`, `crates/iroha_torii/tests/zk_vk_post_integration.rs`), and the SoraFS orchestrator cache/parity suites (`crates/sorafs_orchestrator/{src/lib.rs,src/taikai_cache.rs,tests/*}`) mirror the same pattern so minute/hour constants no longer spell out `from_secs(60 * N)`. |
| Libraries | `Ipv4Addr::from_octets`/`Ipv6Addr::from_segments` | Replace manual constructors in networking crates | DONE | `crates/iroha_primitives/src/addr.rs` now converts via the new const-friendly helpers so P2P/telemetry modules inherit the const-safe std bridge. |
| APIs-in-const | Arrays/OsString/PathBuf constructors const-stable | Revisit `const fn` helpers that currently allocate at runtime | DONE | Handshake PoW defaults now expose const constructors so `SoranetPow`/`SoranetPuzzle` can be instantiated in static contexts (`crates/iroha_config/src/parameters/actual.rs`), unblocking const-initialized config snapshots without runtime allocs. |
| Cargo | `build.build-dir` stabilized | Decide whether CI wants deterministic `target` relocation; document in `ci/README.md` | DONE | Default `target/` path retained; rationale + cache references captured in `ci/README.md`. |
| Rustdoc | Raw-pointer search parity | Regenerate published docs once code changes land | DONE | Rustdoc output was rebuilt with 1.91 so the search index now includes raw-pointer items; the docs sync pipeline picked up the refreshed HTML bundle for the next publish cut. |

## Next steps

1. **Lint sweep** (`TODO: lint sweep`): ✅ Completed — `ci/check_rust_1_91_lints.sh` now runs clean under the stricter lints, and the settlement router tests import `VolatilityBucket` so the sweep covers every crate.
2. **Strict arithmetic rollout** (`TODO: crypto strict math`): ✅ Completed —
   executor/trigger/cursor paths were already using `strict_*`, and the relay’s
   adaptive PoW controller now swaps the remaining saturating counters for
   `strict_add`/`strict_sub` so invalid windows or difficulty steps panic
   instead of silently clamping (`tools/soranet-relay/src/dos.rs`).
3. **Path helpers sweep** (`TODO: path extension helper sweep`): ✅ Completed —
   DA replay cursors, PoR persistence, streaming snapshots, RBC stores, Soranet
   spool files, and the governance publisher atomics now rely on
   `PathBuf::with_added_extension` with regression tests; keep this pattern in
   mind when adding new CLI/admission artefacts.
4. **Const-friendly initializers**: ✅ Completed — handshake PoW defaults now
   ship const constructors so `SoranetPow`/`SoranetPuzzle` can be embedded in
   static config snapshots without runtime allocs; prefer these helpers when
   wiring new config defaults.

Link future PRs to this document so we can tick off rows (convert TODO -> WIP ->
DONE) and keep `status.md` honest about the rollout.
