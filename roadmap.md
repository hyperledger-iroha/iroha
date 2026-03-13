# Roadmap (Open Work Only)

Last updated: 2026-03-13

## Multilane Genesis Pre-exec Follow-up
1. Investigate and fix `RegisterPublicLaneValidator` pre-exec failures that reference synthetic `aid:*` asset-definition IDs (currently pre-exec falls back to synthetic success in cross-dataspace localnet startup).
2. Ensure staking asset-definition lookup stays deterministic across pre-exec and runtime paths without relying on fallback behavior, and add a regression integration test for the corrected path.

## Multisig Admission Follow-up
1. Add an end-to-end integration test that proves an unregistered signatory can successfully complete `MultisigPropose`/`MultisigApprove` against a live multi-peer network (not just unit-level admission/execution slices).

## Query Performance Follow-up
1. Re-run `snapshot_(stored|ephemeral)_sorted_asset_defs_first_batch` on an isolated host/profile and lock acceptance on stable repeated samples (current host shows large Criterion noise spikes).
2. If noisy outliers remain after isolation, tune stored fast-start prefix thresholds to preserve batch-one gains while keeping continuation cost bounded.
3. Restore a green baseline for `cargo test -p iroha_core` in the current branch (77 failing tests currently block meaningful `cargo test --workspace` pass/fail signal for this perf pass).

## API Versioning Follow-up
1. Audit remaining `v1/` Torii URL call-sites in `crates/iroha/src/client.rs` and integration tests, and migrate those backed only by `/v2` routes to avoid runtime 404 polling loops.
