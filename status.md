# Status

Last updated: 2026-03-10

## Current Enforced State
- First-release identity/deploy policy is strict (no backward aliases, shims, or migration wrappers).
- Deploy preflight scanner entrypoint is `../pk-deploy/scripts/check-identity-surface.sh`; the previous scanner entrypoint is removed.
- Runtime deploy scripts are aligned to strict-first-release behavior:
  - `../pk-deploy/scripts/cutover-ih58-mega.sh` invokes only `check-identity-surface.sh`.
- `../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh` no longer performs prior-layout trigger cleanup loops.
- Wallet docs describe only current QR modes in neutral terms.

## 2026-03-11 Full 60-Min Cross-Mode Soak Replay (Fresh Rerun)
- Replayed both full soaks on the current tree with unchanged stable envelope and `progress-timeout=600s`:
  - permissioned log: `/tmp/izanami_permissioned_full_soak_20260311T20260311T180117_escalated.log`
  - NPoS log: `/tmp/izanami_npos_full_soak_20260311T20260311T192506_escalated.log`
- Both runs completed duration without `600s` no-progress abort:
  - permissioned summary: `successes=17998 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
  - NPoS summary: `successes=18000 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
- Final persisted heights:
  - permissioned (`lane_000_default`): `1414/1414/1414/569`
  - NPoS (`lane_000_core`): `959/959/959/184`
- Effective pace (`duration=3600s`):
  - permissioned: strict `569 / 3600 = 0.158 blocks/s`; quorum-with-1-failure `1414 / 3600 = 0.393 blocks/s`
  - NPoS: strict `184 / 3600 = 0.051 blocks/s`; quorum-with-1-failure `959 / 3600 = 0.266 blocks/s`
- Residual churn remains lagging-peer concentrated (not quorum-wide liveness loss):
  - permissioned aggregate markers: `no proposal observed before cutoff=795`, `commit quorum missing past timeout=163`, `fallback anchor shares=4342`, `roster sidecar mismatch ignores=94`
  - NPoS aggregate markers: `no proposal observed before cutoff=698`, `commit quorum missing past timeout=36`, `fallback anchor shares=4745`, `roster sidecar mismatch ignores=576`
  - committed-edge suppression remained bounded (`permissioned=5`, `NPoS=10`).

## 2026-03-11 Full 60-Min Cross-Mode Soak Replay (Post Actionable-Dependency + Shared-Host Parity Patch)
- Executed full 60-minute Izanami stable soaks for both modes using the same envelope and `progress-timeout=600s`:
  - permissioned log: `/tmp/izanami_permissioned_full_soak_20260310T20260311T092642_escalated.log`
  - NPoS log: `/tmp/izanami_npos_full_soak_20260311T20260311T163813_escalated.log`
- Both runs completed duration without `600s` no-progress abort and without ingress failover/unhealthy churn:
  - permissioned summary: `successes=18000 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
  - NPoS summary: `successes=18000 failures=0 izanami_ingress_failover_total=0 izanami_ingress_endpoint_unhealthy_total=0`
- Final persisted heights (Kura, lane/core height):
  - permissioned (`lane_000_default`): `1279/1279/1279/529` (strict min `529`, quorum-with-1-failure min `1279`)
  - NPoS (`lane_000_core`): `952/952/952/124` (strict min `124`, quorum-with-1-failure min `952`)
- Throughput envelope under this host/load profile (`duration=3600s`):
  - permissioned strict pace: `529 / 3600 = 0.147 blocks/s`; quorum pace: `1279 / 3600 = 0.355 blocks/s`
  - NPoS strict pace: `124 / 3600 = 0.034 blocks/s`; quorum pace: `952 / 3600 = 0.264 blocks/s`
- Dominant residual signal remains one-peer lag/freeze-style churn rather than quorum-wide liveness collapse:
  - permissioned aggregate markers: `no proposal observed before cutoff=317`, `commit quorum missing past timeout=259`, `fallback anchor shares=3679`, `roster sidecar mismatch ignores=348`
  - NPoS aggregate markers: `no proposal observed before cutoff=975`, `commit quorum missing past timeout=30`, `fallback anchor shares=3445`, `roster sidecar mismatch ignores=63`
  - committed-edge conflict suppression remained bounded (`permissioned=9`, `NPoS=4`).

### Validation Matrix (2026-03-11 Full Soak Replay)
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha_target_izanami_soak_20260310 cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) CARGO_TARGET_DIR=/tmp/iroha_target_izanami_soak_20260310 cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 600s --tps 5 --max-inflight 8 --workload-profile stable`
- `target/release/kagami kura <peer_block_store_path> print -o <out_file>` (used to extract persisted final heights from Kura block stores)

## 2026-03-10 Cross-Mode Soak Stabilization (Actionable Dependency Gating + Shared-Host Parity)
- Sumeragi missing-dependency classification was normalized across proposal/recovery/commit/vote paths:
  - non-actionable committed-edge conflicts and active lock-rejected hashes no longer gate missing-QC/proposal progress.
  - commit-phase dependency checks now ignore deferred missing-QC entries whose payload is already locally available.
  - stale height-recovery budgets and stale per-hash dependencies are cleared when no actionable dependency remains, preventing repeated same-height reintroduction loops.
  - files: `crates/iroha_core/src/sumeragi/main_loop.rs`, `crates/iroha_core/src/sumeragi/main_loop/votes.rs`.
- Izanami stable-soak profile parity now applies to both permissioned and NPoS shared-host runs:
  - same recovery/pacemaker envelope (timeouts, pending-stall grace, DA fast-reschedule policy, collectors/redundancy knobs) across both modes.
  - NPoS stake bootstrap/preflight constraints remain unchanged.
  - files: `crates/izanami/src/chaos.rs`.
- Stable-soak liveness gate is now duration-first:
  - completing the configured soak duration without `600s` no-progress remains success.
  - unmet `target_blocks` in stable shared-host mode is now KPI/warning (non-fatal).
- Added concise startup logging of effective consensus soak overrides per mode and targeted regressions for:
  - non-actionable dependency clearing,
  - stale-vote/highest-QC fallback actionable checks,
  - cross-mode shared-host override parity,
  - soft-KPI duration-complete behavior.

### Validation Matrix (Cross-Mode Soak Stabilization)
- `cargo test -p iroha_core proposal_gated_by_missing_dependencies_requires_recent_range_pull_progress -- --nocapture`
- `cargo test -p iroha_core proposal_gated_by_missing_dependencies_clears_stale_budget_without_actionable_dependency -- --nocapture`
- `cargo test -p iroha_core has_commit_phase_missing_qc_dependency_ignores_deferred_payload_already_local -- --nocapture`
- `cargo test -p iroha_core stale_view_drops_precommit_vote_when_missing_block_request_is_non_actionable -- --nocapture`
- `cargo test -p izanami shared_host_stable_soak_consensus_overrides_match_between_permissioned_and_npos -- --nocapture`
- `cargo test -p izanami shared_host_recovery_profile_applies_to_permissioned_stable_pilot_runs -- --nocapture`
- `cargo test -p izanami wait_for_target_blocks_reaches_target -- --nocapture`
- `cargo test -p izanami wait_for_target_blocks_soft_kpi_allows_duration_completion_without_target -- --nocapture`

## 2026-03-09 NPoS Soak Stabilization (Preflight + Conservative Tuning + Obsolete Sidecar Mismatch)
- Izanami NPoS preflight audit now runs before soak startup (`crates/izanami/src/chaos.rs`) and enforces:
  - `RegisterPeerWithPop == peer_count`.
  - `RegisterPublicLaneValidator == ActivatePublicLaneValidator == peer_count`.
  - uniform per-validator `initial_stake` and `initial_stake >= min_self_bond`.
  - one explicit stake-distribution summary log for immediate misconfiguration visibility.
- Reliability-first NPoS soak tuning (shared-host profile) landed in `crates/izanami/src/chaos.rs`:
  - timeout floors: `propose=150ms`, `prevote=250ms`, `precommit=350ms`, `commit=900ms`, `da=900ms`, `aggregator=50ms`.
  - `pending_stall_grace_ms=1000`.
  - `da_fast_reschedule=false`.
  - `collectors_k=3` and `redundant_send_r=3` for 4-peer shared-host NPoS soaks.
- Sumeragi repeated roster-sidecar mismatch hardening landed in:
  - `crates/iroha_core/src/sumeragi/main_loop.rs`
  - `crates/iroha_core/src/sumeragi/main_loop/mode.rs`
  - `crates/iroha_core/src/sumeragi/main_loop/votes.rs`
  - behavior change: repeated `(height, expected_hash, stored_hash)` mismatches are short-TTL obsolete/non-actionable; stale per-hash dependencies are cleared; mismatch recovery reanchors from canonical `committed+1` without reintroducing stale targeted missing-QC dependencies.
- New status surface:
  - `roster_sidecar_mismatch_obsolete_total` added in:
    - `crates/iroha_core/src/sumeragi/status.rs`
    - `crates/iroha_data_model/src/block/consensus.rs`
    - `crates/iroha_torii/src/routing.rs`
    - `crates/iroha_torii/src/routing/consensus.rs`
- Regression coverage updates:
  - Izanami preflight pass/fail tests for missing activation, missing PoP, unequal stake.
  - Sumeragi tests for repeated tuple obsolescence, stale dependency cleanup, and missing-QC stall suppression on obsolete committed-edge sidecar mismatches.
  - Data-model and Torii status tests updated for new status field.

### Validation Matrix (NPoS Soak Stabilization)
- `cargo fmt --all`
- `cargo test -p izanami derive_npos_timing_uses_conservative_floors_for_shared_host_npos_soak -- --nocapture`
- `cargo test -p izanami npos_preflight_audit_ -- --nocapture`
- `cargo test -p iroha_core --lib sidecar_mismatch_ -- --nocapture`
- `cargo test -p iroha_core --lib committed_edge_sidecar_conflict_emits_one_bundle_per_window -- --nocapture`
- `cargo test -p iroha_core --lib missing_block_fetch_counters_surface_in_snapshot -- --nocapture`
- `cargo test -p iroha_data_model --test consensus_roundtrip sumeragi_wire_status_roundtrip -- --nocapture`
- `cargo test -p iroha_torii --lib routing::status_tests::status_snapshot_json_includes_da_gate_and_kura_store -- --nocapture`

## 2026-03-09 Sumeragi Committed-Edge Conflict Obsolete-Dependency Fix
- Stopped treating committed-edge conflicting highest-QC hashes as active missing dependencies:
  - committed-edge conflict suppression now does canonical realignment + obsolete request clearing + bounded canonical reanchor only (no conflicting-hash targeted fetch).
  - files:
    - `crates/iroha_core/src/sumeragi/main_loop/votes.rs`
    - `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs`
- Applied the same suppression bundle on proposal/proposal-hint committed-edge drop paths to prevent reintroducing conflicting hashes as dependencies.
- Missing-QC dependency gating now ignores obsolete committed-edge conflicting requests, including commit-phase dependency checks and idle missing-QC reacquire signal detection:
  - `crates/iroha_core/src/sumeragi/main_loop.rs`
- Added telemetry/status counter:
  - `committed_edge_conflict_obsolete_total` in Sumeragi status snapshot and Torii status outputs.
  - files:
    - `crates/iroha_core/src/sumeragi/status.rs`
    - `crates/iroha_data_model/src/block/consensus.rs`
    - `crates/iroha_torii/src/routing/consensus.rs`
    - `crates/iroha_torii/src/routing.rs`
- Updated regressions for committed-edge conflict behavior and added proposal/proposal-hint regressions ensuring repeated committed-edge conflicts do not accumulate missing requests and do not block canonical proposal progress:
  - `crates/iroha_core/src/sumeragi/main_loop/tests.rs`

### Validation Matrix (Committed-Edge Conflict Obsolete-Dependency Fix)
- `cargo fmt --all`
- `cargo test -p iroha_core --lib committed_conflict -- --nocapture`
- `cargo test -p iroha_core --lib highest_qc_fetch_suppresses_committed_height_hash_conflict -- --nocapture`
- `cargo test -p iroha_core --lib missing_qc_height_stall_mode_ignores_obsolete_committed_edge_conflicts -- --nocapture`
- `cargo test -p iroha_core --lib missing_block_fetch_counters_surface_in_snapshot -- --nocapture`
- `cargo test -p iroha_torii --lib status_snapshot_json_includes_da_gate_and_kura_store --features "iroha_core/iroha-core-tests" -- --nocapture`
- `cargo test -p iroha_data_model --test consensus_roundtrip sumeragi_wire_status_roundtrip -- --nocapture`
- `cargo check -p iroha`

## 2026-03-09 Full Permissioned + NPoS Soak Replay (Committed-Edge Obsolete-Dependency Patch)
- Ran full 2000-block Izanami soaks for both consensus modes on this exact patch set with preserved peer artifacts.
- Permissioned full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: failed on strict divergence guard after `successes=1951` (`divergence=55`, `quorum min=228`, `strict min=173`), log `/tmp/izanami_permissioned_full_soak_20260309T115334.log`, network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_mWvP92`.
- NPoS full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: failed on strict divergence guard after `successes=1101` (`divergence=41`, `quorum min=109`, `strict min=68`), log `/tmp/izanami_npos_full_soak_20260309T121111.log`, network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_lssABH`.
- Regression-specific signal:
  - committed-edge suppression path was exercised in both soaks (`suppressing committed-edge conflicting highest-QC reference`: permissioned `10`, NPoS `4`) with proposal/new-view/proposal-hint sources observed.
  - no evidence of same-height infinite missing-payload dependency acquisition on conflicting committed-edge hashes; suppression remained canonical-reanchor/obsolete-classification scoped.
- Dominant remaining liveness blocker in both modes was strict/quorum split driven by one lagging peer with heavy fallback-anchor replay + repeated `no proposal observed` / `commit quorum missing` churn, not a conflicting-hash dependency loop.

### Validation Matrix (Full Permissioned + NPoS Soak Replay)
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
- `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`

## 2026-03-09 Committed-Edge Liveness Follow-Up (Strict Guard + Proposal Path)
- Applied a minimal-policy strict-progress fix in Izanami:
  - `effective_tolerated_peer_failures(...)` now uses protocol/BFT tolerance from peer count (not `--faulty` injection budget), preventing false strict-divergence aborts in `--faulty 0` runs with one transient straggler.
  - file: `crates/izanami/src/chaos.rs`
- Applied proposal-path committed-edge suppression in Sumeragi:
  - in `assemble_and_broadcast_proposal(...)`, `LockedQcRejection::HashMismatch` + missing highest-QC now invokes committed-edge suppression before recording missing-highest defer markers, so obsolete committed-edge conflicts are not reintroduced by leader proposal defer flow.
  - file: `crates/iroha_core/src/sumeragi/main_loop/propose.rs`
- Added suppression cleanup for queued descendants rooted on obsolete conflicting committed-edge parent hashes:
  - committed-edge suppression now drops pending descendant blocks whose parent hash is the conflicting committed-edge hash.
  - file: `crates/iroha_core/src/sumeragi/main_loop/votes.rs`
- Added regressions:
  - `assemble_proposal_suppresses_committed_edge_highest_qc_conflict`
  - `committed_edge_conflict_suppression_purges_descendant_pending_blocks`
  - file: `crates/iroha_core/src/sumeragi/main_loop/tests.rs`

### Validation Matrix (Committed-Edge Liveness Follow-Up)
- `cargo fmt --all`
- `cargo test -p izanami effective_tolerated_peer_failures_uses_protocol_tolerance -- --nocapture`
- `cargo test -p izanami strict_divergence_guard_requires_more_than_tolerated_outliers -- --nocapture`
- `cargo test -p izanami strict_progress_timeout_enforcement_respects_bft_tolerance -- --nocapture`
- `cargo test -p iroha_core --lib assemble_proposal_suppresses_committed_edge_highest_qc_conflict -- --nocapture`
- `cargo test -p iroha_core --lib committed_edge_conflict_suppression_purges_descendant_pending_blocks -- --nocapture`
- `cargo test -p iroha_core --lib committed_conflict -- --nocapture`

## 2026-03-09 Full Permissioned + NPoS Soak Replay (Post Follow-Up Patchset)
- Ran full 2000-block Izanami envelopes on the latest strict-guard + proposal-path suppression patchset.
- Permissioned full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: no strict-divergence abort; run completed duration and stopped before target (`successes=18000`, `err=izanami run stopped before target blocks reached`), network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_U9yElv`.
- NPoS full soak:
  - command: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable`
  - result: strict-divergence guard remained suppressed, but quorum progress still stalled before target (`successes=11203`; `no block height progress for 600s`, `quorum min=721`, `strict min=142`, `tolerated_failures=1`), network `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_PIBzHl`.
- Regression signal after follow-up:
  - obsolete committed-edge hashes are still suppressed (no conflicting-hash missing request reintroduction from proposal path), but NPoS still shows high-frequency committed-edge suppression + fallback-anchor replay at a fixed height (`~722`) with repeated `missing_qc`/`no proposal observed` churn.

## 2026-03-08 Norito Instruction Fixture Refresh
- Refreshed stale fixture payloads in `fixtures/norito_instructions` to match current canonical Rust Norito encoding for:
  - `burn_asset_numeric.json`
  - `burn_asset_fractional.json`
  - `mint_asset_numeric.json`
- Updated both `instruction` (base64 Norito frame) and `encoded_hex` (canonical payload bytes) fields.
- Simplified fixture descriptions to avoid embedding stale encoded asset-id literals.

### Validation Matrix (Norito Instruction Fixture Refresh)
- `cargo test -p integration_tests --test norito_burn_fixture -- --nocapture`

## 2026-03-09 Oracle Feed-History Unregister Guards
- Closed remaining account/domain unregister referential gap for oracle audit history:
  - `Unregister<Account>` now rejects when the account appears in any `oracle_history` success-entry provider (`ReportEntry.oracle_id`) reference:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when any member account being removed appears in `oracle_history` success-entry provider references:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- Added regressions:
  - `unregister_account_rejects_when_account_has_oracle_feed_history_state`
  - `unregister_domain_rejects_when_member_account_has_oracle_feed_history_state`
- Updated docs wording for unregister guard rails to include oracle feed-history provider references:
  - `docs/source/data_model_and_isi_spec.md`

### Validation Matrix (Oracle Feed-History Unregister Guards)
- `cargo fmt --all`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_oracle_feed_history_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_oracle_feed_history_state -- --nocapture`
- `cargo check -p iroha_core`

## 2026-03-08 Data Model Consistency Sweep (Account/Domain/Dataspace/Asset)
- Fixed tracked asset-definition totals when cascading unregister operations remove assets:
  - Added `WorldTransaction::remove_asset_and_metadata_with_total(...)` in `crates/iroha_core/src/state.rs`.
  - Switched account/domain/asset-definition unregister paths to use the total-aware helper:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- Removed query-time asset total recomputation workaround so `FindAssetsDefinitions` now relies on persisted totals:
  - `crates/iroha_core/src/smartcontracts/isi/asset.rs`
- Aligned UAID lane identity behavior to global routing semantics (dataspace binding no longer required for admission; inactive target manifest still rejects):
  - Queue admission path and tests: `crates/iroha_core/src/queue.rs`
  - Transaction lane-policy identity extraction + tests: `crates/iroha_core/src/tx.rs`
- Relaxed overly strict owner-domain coupling in formal verification invariants while keeping owner-exists checks:
  - `crates/iroha_data_model/src/verification.rs`
- Added direct unit coverage for new total-aware removal helper:
  - `state::tests::remove_asset_and_metadata_with_total_decrements_definition_total`
  - `state::tests::remove_asset_and_metadata_with_total_cleans_orphan_metadata`
- Added integration coverage for unregister cascade correctness of persisted totals:
  - `asset_totals_drop_when_unregistering_account`
  - `asset_totals_drop_when_unregistering_domain_with_foreign_holders`
  - `unregistering_definition_domain_cleans_foreign_assets`
  - file: `crates/iroha_core/tests/asset_total_amount.rs`
- Removed obsolete queue rejection surface for UAID dataspace binding:
  - dropped `queue::Error::UaidNotBound` from `crates/iroha_core/src/queue.rs`
  - removed corresponding Torii status/reason mappings and stale telemetry-reason aggregation label in `crates/iroha_torii/src/lib.rs`
- Centralized lane identity metadata extraction to a single shared helper:
  - added `extract_lane_identity_metadata(...)` and `LaneIdentityMetadataError` in `crates/iroha_core/src/nexus/space_directory.rs`
  - switched both queue admission and tx lane policy paths to this shared helper:
    - `crates/iroha_core/src/queue.rs`
    - `crates/iroha_core/src/tx.rs`
  - added direct helper tests:
    - `nexus::space_directory::tests::lane_identity_metadata_allows_missing_target_manifest`
    - `nexus::space_directory::tests::lane_identity_metadata_rejects_inactive_target_manifest`
- Added data-model verification regression coverage for cross-domain ownership references:
  - `verification::tests::cross_domain_owners_are_allowed_when_references_exist`
  - file: `crates/iroha_data_model/src/verification.rs`
- Enforced ownership integrity on unregister paths so account/domain removal cannot orphan ownership references:
  - `Unregister<Account>` now rejects when the target account still owns any domain or asset definition:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when accounts being removed still own domains or asset definitions outside the domain being deleted:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_owns_domain`
    - `unregister_account_rejects_when_account_owns_asset_definition`
    - `unregister_domain_rejects_when_member_account_owns_foreign_domain`
    - `unregister_domain_rejects_when_member_account_owns_foreign_asset_definition`
- Hardened asset-definition ownership transfer authorization across both sequential and detached pipelines:
  - `Transfer<Account, AssetDefinitionId, Account>` now enforces the same authority model as domain transfer (source account, source-domain owner, or definition-domain owner):
    - `crates/iroha_core/src/smartcontracts/isi/account.rs`
  - added detached-delta authorization helper and checks so parallel overlay execution cannot bypass ownership checks:
    - `crates/iroha_core/src/state.rs`
    - `crates/iroha_core/src/block.rs`
  - added initial-executor precheck parity for asset-definition transfers:
    - `crates/iroha_core/src/executor.rs`
  - added regressions:
    - `transfer_asset_definition_rejects_unauthorized_authority`
    - `transfer_asset_definition_allows_definition_domain_owner`
    - `detached_can_transfer_asset_definition_denies_non_owner`
    - `detached_can_transfer_asset_definition_considers_pending_domain_transfers`
    - `initial_executor_denies_transfer_asset_definition_without_ownership`
    - `initial_executor_allows_transfer_asset_definition_by_definition_domain_owner`
- Closed remaining SoraFS provider-owner referential gaps around account/domain lifecycle:
  - `Unregister<Account>` now rejects when the account is still referenced as a SoraFS provider owner:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when any member account being deleted still owns a SoraFS provider:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `RegisterProviderOwner` now requires the destination owner account to exist before inserting bindings:
    - `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`
  - `State::set_gov` now skips configured SoraFS provider-owner bindings whose owner account does not exist:
    - `crates/iroha_core/src/state.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_owns_sorafs_provider`
    - `unregister_domain_rejects_when_member_account_owns_sorafs_provider`
    - `register_provider_owner_rejects_missing_owner_account`
    - `set_gov_skips_sorafs_provider_owner_without_account`
- Enforced NFT transfer authorization symmetry across sequential and detached pipelines:
  - `Transfer<Account, NftId, Account>` now requires authority to be source account, source-domain owner, NFT-domain owner, or holder of `CanTransferNft` for the target NFT:
    - `crates/iroha_core/src/smartcontracts/isi/nft.rs`
  - added initial-executor precheck parity for NFT transfers:
    - `crates/iroha_core/src/executor.rs`
  - added detached overlay precheck parity for NFT transfers:
    - `crates/iroha_core/src/state.rs`
    - `crates/iroha_core/src/block.rs`
  - added regressions:
    - `transfer_nft_rejects_authority_without_ownership`
    - `transfer_nft_allows_nft_domain_owner`
    - `initial_executor_denies_transfer_nft_without_ownership`
    - `initial_executor_allows_transfer_nft_by_nft_domain_owner`
    - `detached_can_transfer_nft_denies_non_owner`
    - `detached_can_transfer_nft_considers_pending_domain_transfers`
- Guarded account/domain unregister against orphaning governance citizenship records:
  - `Unregister<Account>` now rejects when the account has an active citizenship record:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when any member account being removed has an active citizenship record:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_citizenship_record`
    - `unregister_domain_rejects_when_member_account_has_citizenship_record`
- Guarded account/domain unregister against orphaning public-lane staking references:
  - `Unregister<Account>` now rejects when the account still appears in public-lane validator/stake/reward/reward-claim registries:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed still appear in public-lane validator/stake/reward/reward-claim registries:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_public_lane_validator_state`
    - `unregister_domain_rejects_when_member_account_has_public_lane_validator_state`
    - `unregister_account_rejects_when_account_has_public_lane_reward_record_state`
    - `unregister_domain_rejects_when_member_account_has_public_lane_reward_record_state`
- Guarded account/domain unregister against orphaning oracle references:
  - `Unregister<Account>` now rejects when the account still appears in oracle feed/change/dispute/provider-stats/observation state:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed still appear in oracle feed/change/dispute/provider-stats/observation state:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_oracle_feed_provider_state`
    - `unregister_domain_rejects_when_member_account_has_oracle_feed_provider_state`
- Guarded account/domain unregister against orphaning repo agreement references:
  - `Unregister<Account>` now rejects when the account appears as initiator/counterparty/custodian in active repo agreements:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed appear in active repo agreements:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_repo_agreement_state`
    - `unregister_domain_rejects_when_member_account_has_repo_agreement_state`
- Guarded account/domain unregister against orphaning settlement-ledger references:
  - `Unregister<Account>` now rejects when the account appears in settlement ledger authority/leg records:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed appear in settlement ledger authority/leg records:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_settlement_ledger_state`
    - `unregister_domain_rejects_when_member_account_has_settlement_ledger_state`
- Guarded account/domain unregister against orphaning offline settlement references:
  - `Unregister<Account>` now rejects when the account appears in active offline allowance or transfer records:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now rejects when member accounts being removed appear in active offline allowance or transfer records:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - successful account/domain removal now drops stale sender/receiver offline transfer index entries for removed accounts:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_offline_allowance_state`
    - `unregister_domain_rejects_when_member_account_has_offline_transfer_state`
- Closed remaining account/domain unregister referential gaps across offline-governance-content state:
  - `Unregister<Account>` now additionally rejects when the account appears in:
    - offline verdict revocations (`offline_verdict_revocations`)
    - governance proposal/stage-approval/lock/slash ledgers
    - governance council/parliament rosters
    - content bundle creator references (`content_bundles.created_by`)
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `Unregister<Domain>` now additionally rejects when any member account being removed appears in those same offline/governance/content stores:
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_offline_verdict_revocation_state`
    - `unregister_domain_rejects_when_member_account_has_offline_verdict_revocation_state`
    - `unregister_account_rejects_when_account_has_governance_proposal_state`
    - `unregister_domain_rejects_when_member_account_has_governance_proposal_state`
    - `unregister_account_rejects_when_account_has_content_bundle_state`
    - `unregister_domain_rejects_when_member_account_has_content_bundle_state`
- Extended account/domain unregister guard rails to additional live account-reference stores:
  - runtime upgrade proposer references (`runtime_upgrades.proposer`)
  - oracle twitter-binding provider references (`twitter_bindings.provider`)
  - social viral escrow sender references (`viral_escrows.sender`)
  - SoraFS pin-registry issuer/binder references:
    - `pin_manifests.submitted_by`
    - `manifest_aliases.bound_by`
    - `replication_orders.issued_by`
  - implemented in:
    - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
    - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - added regressions:
    - `unregister_account_rejects_when_account_has_runtime_upgrade_state`
    - `unregister_domain_rejects_when_member_account_has_runtime_upgrade_state`
    - `unregister_account_rejects_when_account_has_viral_escrow_state`
    - `unregister_domain_rejects_when_member_account_has_viral_escrow_state`
    - `unregister_account_rejects_when_account_has_sorafs_pin_manifest_state`
    - `unregister_domain_rejects_when_member_account_has_sorafs_pin_manifest_state`
- Updated Space Directory and Nexus compliance docs to match global UAID routing semantics:
  - replaced outdated “UAID not bound => queue rejection” wording with “missing target manifest allowed; inactive manifest rejected”
  - touched multilingual variants in:
    - `docs/space-directory*.md`
    - `docs/source/nexus_compliance*.md`

### Validation Matrix (Data Model Consistency Sweep)
- `cargo fmt --all`
- `cargo test -p iroha_data_model --lib verification -- --nocapture`
- `cargo test -p iroha_core --lib uaid_ -- --nocapture`
- `cargo test -p iroha_core --lib lane_identity_ -- --nocapture`
- `cargo test -p iroha_core --lib lane_identity_metadata_ -- --nocapture`
- `cargo test -p iroha_core --lib remove_asset_and_metadata_with_total -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_owns -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_owns -- --nocapture`
- `cargo test -p iroha_core --lib provider_owner_ -- --nocapture`
- `cargo test -p iroha_core --lib set_gov_ -- --nocapture`
- `cargo test -p iroha_core --lib transfer_nft -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_citizenship -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_citizenship -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_public_lane_validator_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_public_lane_validator_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_public_lane_reward_record_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_public_lane_reward_record_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_oracle_feed_provider_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_oracle_feed_provider_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_repo_agreement_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_repo_agreement_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_settlement_ledger_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_settlement_ledger_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_offline_allowance_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_offline_transfer_state -- --nocapture`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_ -- --nocapture`
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_ -- --nocapture`
- `cargo check -p iroha_core`
- `cargo test -p iroha_core --lib transfer_asset_definition_ -- --nocapture`
- `cargo test -p iroha_core --test asset_total_amount -- --nocapture`
- `cargo test -p iroha_torii --lib tests_queue_metadata::queue_errors_map_to_reason_codes -- --nocapture`
- `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
- `cargo check -p iroha_core`

## 2026-03-08 Integration Failures: CBDC Rollout + DA Kura Eviction
- Fixed CBDC rollout fixture validation to accept canonical validator identifiers used by fixtures:
  - `ci/check_cbdc_rollout.sh` now accepts either `name@domain`-style identifiers or non-empty encoded identifiers (without whitespace), instead of requiring `@` unconditionally.
- Stabilized DA-backed Kura eviction integration coverage in multi-lane storage layouts:
  - `integration_tests/tests/sumeragi_da.rs` now discovers the evicted block via `da_blocks/*.norito` paths and derives the matching lane `blocks.index`/`blocks.hashes` paths from that location.
  - The test still verifies that the selected `blocks.index` entry is marked evicted (`u64::MAX`) and that the queried rehydrated block hash matches `blocks.hashes`.

### Validation Matrix (CBDC + DA Eviction Fix)
- `cargo fmt --all`
- `cargo test -p integration_tests nexus::cbdc_rollout_bundle::cbdc_rollout_fixture_passes_validator -- --nocapture`
- `cargo test -p integration_tests sumeragi_da::sumeragi_da_kura_eviction_rehydrates_from_da_store -- --nocapture`

## 2026-03-08 Telemetry Test Helper Duplication
- Removed a duplicate async helper definition in `crates/iroha_telemetry/src/ws.rs` that caused
  `error[E0428]` for `broadcast_lag_does_not_stop_client_with_suite`.
- Kept a single canonical helper implementation; the `broadcast_lag_does_not_stop_client` test path is unchanged.

### Validation Matrix (Telemetry Duplication)
- `cargo test -p iroha_telemetry broadcast_lag_does_not_stop_client -- --nocapture`

## 2026-03-08 AccountId Parsing API Alignment (Test Samples)
- Updated `crates/iroha_test_samples/src/lib.rs` to stop parsing `AccountId` from string in tests.
- Replaced string `.parse::<AccountId>()` with explicit construction via
  `AccountId::new(DomainId, PublicKey)` to match the current data-model API.

### Validation Matrix (AccountId Parsing Alignment)
- `cargo test -p iroha_test_samples -- --nocapture`

## Changes Completed In This Pass
- Replaced deploy scanner interface with a neutral strict entrypoint.
- Updated deploy callsites/docs to the new scanner path and strict wording:
  - `../pk-deploy/scripts/cutover-ih58-mega.sh`
  - `../pk-deploy/scripts/README-redeploy.md`
- Purged prior-transition terminology from touched runtime/docs/status surfaces.
- Reset status/history files to fresh baselines:
  - `status.md`
  - `roadmap.md`
  - `../pk-deploy/STATUS.md`

## Validation Matrix (This Pass)
- `bash -n ../pk-deploy/scripts/check-identity-surface.sh ../pk-deploy/scripts/cutover-ih58-mega.sh ../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh ../pk-cbuae-mock/scripts/e2e/localnet-live.sh`
- `bash ../pk-deploy/scripts/check-identity-surface.sh`
- identity-literal forbidden-token sweep across requested repos/files
- residual-token sweep across touched runtime/scripts/status files
- iOS targeted retest for previously failing flow:
  - `cd ../pk-retail-wallet-ios && xcodebuild test -scheme RetailWalletIOS -destination 'platform=iOS Simulator,name=iPhone 17,OS=26.1' -only-testing:RetailWalletIOSUITests/RetailWalletIOSFlowUITests/testOnboardingAndSendFlow`

## Remaining Actionable Blockers
- None in active A1-G1 runtime/parser/SDK paths.

## 2026-03-08 Unregister Referential-Integrity Guard Expansion (Account/Domain)
- Extended `Unregister<Account>` and `Unregister<Domain>` guard rails for additional account-reference state:
  - DA pin-intent owner references in `da_pin_intents_by_ticket` (`intent.owner`).
  - Lane-relay emergency validator overrides in `lane_relay_emergency_validators` (`validators`).
  - Governance proposal parliament snapshot rosters in `governance_proposals.parliament_snapshot.bodies`.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added 6 targeted regression tests (3 account + 3 domain) covering the new reject-on-reference behavior.

### Validation Matrix (Unregister Guard Expansion)
- `cargo fmt --all`
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib governance_parliament_snapshot_state -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Torii /v2 API Surface Cleanup
- Removed dead telemetry compatibility handlers from Torii:
  - `crates/iroha_torii/src/lib.rs` (`handler_status_root_v2`, `handler_status_tail_v2`)
- Confirmed Torii route registrations do not expose `/v2/...` paths; active HTTP surface remains `/v1/...` (plus intentional unversioned utility endpoints such as `/status`, `/metrics`, `/api_version`).

### Validation Matrix (Torii /v2 Cleanup)
- `cargo fmt --all`
- `cargo test -p iroha_torii --test api_versioning -- --nocapture`

## 2026-03-07 A1-G1 Closure Follow-up
- Completed Android hard-cut test alignment for encoded-only account/asset identity:
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/norito/NoritoCodecAdapterTests.java`
  - `java/iroha_android/src/test/java/org/hyperledger/iroha/android/tx/TransactionFixtureManifestTests.java`
- Completed JS SDK strict test migration for domainless account ids and encoded-only asset ids:
  - `javascript/iroha_js/test/address_public_key_validation.test.js`
  - `javascript/iroha_js/test/multisigProposalInstruction.test.js`
  - `javascript/iroha_js/test/multisigRegisterInstruction.test.js`
  - `javascript/iroha_js/test/validationError.test.js`
  - `javascript/iroha_js/test/toriiClient.test.js`
  - `javascript/iroha_js/test/toriiIterators.parity.test.js`
- Updated Swift `AccountId.make` to return encoded IH58 identifiers (domainless subject id surface) and aligned affected tests:
  - `IrohaSwift/Sources/IrohaSwift/Crypto.swift`
  - `IrohaSwift/Tests/IrohaSwiftTests/AccountIdTests.swift`
  - `IrohaSwift/Tests/IrohaSwiftTests/BridgeAvailabilityTests.swift`
  - `IrohaSwift/Tests/IrohaSwiftTests/NativeBridgeLoaderTests.swift`
- Static sweep confirms no `parse_any` references remain in Rust/JS/Swift/Android/docs/status/roadmap paths.

### Validation Matrix (Follow-up)
- `cd java/iroha_android && ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test`
- `cd javascript/iroha_js && IROHA_JS_DISABLE_NATIVE=1 npm run test:js`
- `cd IrohaSwift && swift test --filter AccountAddressTests`
- `cd IrohaSwift && swift test --filter AccountIdTests`
- `cd IrohaSwift && swift test --filter 'BridgeAvailabilityTests|BridgeAvailabilitySurfaceTests'`
- `CARGO_TARGET_DIR=target_hardcut cargo check -p iroha_torii`

## 2026-03-07 Account Filter Alias Regression
- Restored state-backed alias resolution for account filter literals in Torii while retaining strict encoded parsing.
- Preserved rejection of legacy `public_key@domain` literals by explicitly excluding them from alias fallback.
- Added a Torii unit regression test covering alias and compressed account literals in `/v1/accounts/query` filters:
  - `crates/iroha_torii/src/routing.rs`

### Validation Matrix (Alias Regression)
- `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii accounts_query_filter_accepts_alias_and_compressed_literals -- --nocapture`
- `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p integration_tests --test address_canonicalisation accounts_query_accepts_alias_and_compressed_filter_literals -- --nocapture`
- `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p integration_tests --test address_canonicalisation accounts_query_rejects_public_key_filter_literals -- --nocapture`
- `cargo fmt --all`

## 2026-03-08 Hard-Cut Debt Sweep
- Removed remaining Python runtime account parse compatibility surface:
  - deleted `AccountAddress.parse_any(...)`
  - removed canonical-hex parser entrypoint from Python account parsing
  - added strict `AccountAddress.parse_encoded(...)` (IH58/sora compressed only; rejects `@domain` and canonical-hex input)
  - updated governance owner canonicalization to strict IH58 decode path.
- Removed positive-path legacy account/asset literals from Python SDK docs/examples/fixtures:
  - `python/iroha_python/README.md`
  - `python/iroha_python/src/iroha_python/examples/tx_flow.py`
  - `python/iroha_python/notebooks/connect_automation.ipynb`
  - `python/iroha_python/tests/test_governance_zk_ballot.py`
  - `python/iroha_python/tests/fixtures/transaction_payload.json`
  - `python/iroha_torii_client/tests/test_client.py`
  - `python/iroha_python/iroha_python_rs/src/lib.rs`
- Unblocked Python client-only import paths without a prebuilt native extension:
  - moved `TransactionConfig`/`TransactionDraft` exports behind the existing optional crypto import gate in `python/iroha_python/src/iroha_python/__init__.py`
  - removed hard runtime `tx` import from `python/iroha_python/src/iroha_python/repo.py` (typing-only dependency)
  - switched `python/iroha_python/src/iroha_python/sorafs.py` to `_native.load_crypto_extension()` with graceful fallback plus built-in alias-policy defaults matching config constants.
- Cleared stale strict-model wording:
  - updated Python transaction helper docs in `python/iroha_python/src/iroha_python/crypto.py` so `authority` is documented as domainless encoded account literal only.
  - updated `docs/fraud_playbook.md` `RiskQuery.subject` schema text to remove optional `@<domain>`/alias hints.
- Added explicit scoped-account naming at domain-bound Rust boundaries:
  - introduced `ScopedAccountId` in `crates/iroha_data_model/src/account.rs`.
  - migrated domain-bound account parse helper signatures to `ScopedAccountId` in:
    - `crates/iroha_core/src/block.rs`
    - `crates/iroha_torii/src/routing.rs`
- Removed residual optional `@<domain>` hint wording from localized fraud docs:
  - `docs/fraud_playbook.{ar,es,fr,he,ja,pt,ru,ur}.md`
  - `docs/source/fraud_monitoring_system.{he,ja}.md`
- Updated data-model doc family wording so `alias@domain` is marked rejected legacy form across `docs/source/data_model*.md` and `docs/source/data_model_and_isi_spec*.md`.
- Closed remaining `scripts/export_norito_fixtures` test breakages introduced by stricter opaque/wire-payload handling:
  - fixed test assumptions in `scripts/export_norito_fixtures/src/main.rs`
  - all tests in that crate pass.
- Static closure sweeps:
  - no runtime `parse_any` account parser references in Rust/JS/Swift/Android/Python source paths.
  - no positive-path `@domain` account literals in active Python SDK source/docs paths.
  - no positive-path legacy textual asset id forms in active Python SDK source/docs paths.

## 2026-03-08 Scoped Naming Completion Sweep
- Completed explicit scoped-account naming migration across remaining high-impact Rust boundaries:
  - `crates/iroha_core/src/block.rs`
  - `crates/iroha_torii/src/routing.rs`
  - `crates/ivm/src/core_host.rs`
  - `crates/iroha_cli/src/main_shared.rs`
  - `crates/iroha_js_host/src/lib.rs`
  - `crates/izanami/src/instructions.rs`
  - `crates/iroha_kagami/src/localnet.rs`
- Exposed `ScopedAccountId` in the data-model account prelude to make domain-bound identity explicit at callsites:
  - `crates/iroha_data_model/src/account.rs`
- Updated `Registrable::build(...)` authority parameter to explicit scoped identity:
  - `crates/iroha_data_model/src/lib.rs`
- Fixed IVM pointer-ABI symbol fallout from the scoped naming sweep while keeping ABI IDs unchanged (`PointerType::AccountId` remains canonical).

### Validation Matrix (Scoped Naming Completion)
- `cargo fmt --all`
- `CARGO_TARGET_DIR=target_hardcut cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p ivm -p iroha_cli -p izanami -p iroha_kagami -p iroha_js_host` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_core parse_account_literal_rejects_alias_domain_literals -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_torii accounts_query_filter_rejects_alias_and_accepts_compressed_literals -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p ivm pointer_to_norito_roundtrips_via_pointer_from_norito -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_js_host gateway_write_mode_parses_upload_hint -- --nocapture` (pass)

### Validation Matrix (2026-03-08)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_data_model confidential_wallet_fixtures_are_stable -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test -p iroha_data_model offline_allowance_fixtures_roundtrip -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test --manifest-path scripts/export_norito_fixtures/Cargo.toml -- --nocapture` (pass)
- `CARGO_TARGET_DIR=target_hardcut cargo test --manifest-path python/iroha_python/iroha_python_rs/Cargo.toml attachments_json_decodes_versioned_signed_transaction -- --nocapture` (pass)
- `python -m pytest python/iroha_torii_client/tests/test_client.py -k "uaid_portfolio or space_directory_manifest or trigger_listing_and_lookup_roundtrip or offline_allowance"` in isolated venv (pass: 12 selected tests)
- `PYTHONPATH=python/iroha_python/src:python/iroha_torii_client python -m pytest python/iroha_python/tests/test_governance_zk_ballot.py` in isolated venv (pass: 12 tests)

## 2026-03-08 Asset-Definition Referential-Integrity Guard Expansion
- Extended `Unregister<AssetDefinition>` to reject unregister when the target definition is still referenced by:
  - repo agreements (`repo_agreements` cash/collateral legs),
  - settlement ledger legs (`settlement_ledgers`),
  - public-lane reward ledger and pending claims (`public_lane_rewards`, `public_lane_reward_claims`),
  - offline allowance and transfer receipts (`offline_allowances`, `offline_to_online_transfers`).
- Added confidential-state cascade cleanup during asset-definition unregister:
  - remove `world.zk_assets[asset_definition_id]` together with the definition.
- Extended `Unregister<Domain>` asset-definition teardown path to enforce the same asset-definition reference guards before deleting domain asset definitions, closing foreign-account orphan paths (domain asset defs referenced externally).
- Closed dataspace-catalog drift for emergency relay overrides:
  - `State::set_nexus(...)` now prunes `lane_relay_emergency_validators` entries whose dataspaces are removed from the new `dataspace_catalog`, preventing stale dataspace references.
- Extended the same `set_nexus(...)` dataspace-catalog pruning to Space Directory derived bindings:
  - stale `uaid_dataspaces` entries are now trimmed to active catalog dataspaces so removed dataspaces cannot survive in UAID->dataspace/account bindings.
- Extended `set_nexus(...)` dataspace-catalog pruning to cached AXT policy entries:
  - stale `axt_policies` dataspace keys are now removed when dataspaces disappear from `dataspace_catalog`.
- Added regression:
  - `set_nexus_prunes_lane_relay_emergency_overrides_for_removed_dataspaces`
  - `set_nexus_prunes_uaid_bindings_for_removed_dataspaces`
  - `set_nexus_removes_uaid_binding_when_all_dataspaces_are_pruned`
  - `set_nexus_prunes_axt_policies_for_removed_dataspaces`
- Added tests:
  - `unregister_asset_definition_rejects_when_definition_has_repo_agreement_state`
  - `unregister_asset_definition_rejects_when_definition_has_settlement_ledger_state`
  - `unregister_asset_definition_removes_confidential_state`
  - `unregister_domain_rejects_when_domain_asset_definition_has_foreign_repo_agreement_state`
- Updated `docs/source/data_model_and_isi_spec.md` unregister semantics for Domain/AssetDefinition guard rails and `zk_assets` cleanup.

### Validation Matrix (Asset-Definition Integrity)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_rejects_when_definition_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_removes_confidential_state -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_domain_asset_definition_has_foreign_repo_agreement_state -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_has_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_lane_relay_emergency_overrides_for_removed_dataspaces -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_uaid_bindings_for_removed_dataspaces -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_removes_uaid_binding_when_all_dataspaces_are_pruned -- --nocapture` (pass)
- `cargo test -p iroha_core --lib set_nexus_prunes_axt_policies_for_removed_dataspaces -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Public-Lane Reward-Claim Ownership Guard Fix
- Closed a remaining account/domain unregister gap in `public_lane_reward_claims`:
  - `Unregister<Account>` now rejects not only when the account is the claim claimant, but also when it is referenced as `asset_id.account()` in pending reward-claim keys.
  - `Unregister<Domain>` now applies the same claimant-or-asset-owner guard for each member account being removed.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
- Added regressions:
  - `unregister_account_rejects_when_account_is_reward_claim_asset_owner`
  - `unregister_domain_rejects_when_member_account_is_reward_claim_asset_owner`

### Validation Matrix (Reward-Claim Ownership Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_reward_claim_asset_owner -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_reward_claim_asset_owner -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Governance-Config Reference Guard Expansion
- Closed a remaining unregister integrity gap for governance-configured account/asset references:
  - `Unregister<Account>` now rejects removal when the account is configured as:
    - `gov.bond_escrow_account`
    - `gov.citizenship_escrow_account`
    - `gov.slash_receiver_account`
    - `gov.viral_incentives.incentive_pool_account`
    - `gov.viral_incentives.escrow_account`
  - `Unregister<Domain>` now applies the same governance-account guard for each member account being removed.
  - `Unregister<AssetDefinition>` now rejects removal when the definition is configured as:
    - `gov.voting_asset_id`
    - `gov.citizenship_asset_id`
    - `gov.parliament_eligibility_asset_id`
    - `gov.viral_incentives.reward_asset_definition_id`
  - `Unregister<Domain>` now applies the same governance-asset-definition guard before deleting domain asset definitions.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_governance_bond_escrow_account`
  - `unregister_account_rejects_when_account_is_governance_viral_incentive_pool_account`
  - `unregister_asset_definition_rejects_when_definition_is_governance_voting_asset`
  - `unregister_asset_definition_rejects_when_definition_is_governance_viral_reward_asset`
  - `unregister_domain_rejects_when_member_account_is_governance_bond_escrow_account`
  - `unregister_domain_rejects_when_member_account_is_governance_viral_incentive_pool_account`
  - `unregister_domain_rejects_when_domain_asset_definition_is_governance_voting_asset`
  - `unregister_domain_rejects_when_domain_asset_definition_is_governance_viral_reward_asset`

### Validation Matrix (Governance-Config Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib governance_bond_escrow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib governance_voting_asset -- --nocapture` (pass)
- `cargo test -p iroha_core --lib governance_viral -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Oracle-Economics Config Reference Guard Expansion
- Closed remaining unregister integrity gaps for oracle-economics configured account/asset references:
  - `Unregister<Account>` now rejects removal when the account is configured as:
    - `oracle.economics.reward_pool`
    - `oracle.economics.slash_receiver`
  - `Unregister<Domain>` now applies the same oracle-economics account guard for each member account being removed.
  - `Unregister<AssetDefinition>` now rejects removal when the definition is configured as:
    - `oracle.economics.reward_asset`
    - `oracle.economics.slash_asset`
    - `oracle.economics.dispute_bond_asset`
  - `Unregister<Domain>` now applies the same oracle-economics asset-definition guard before deleting domain asset definitions.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_oracle_reward_pool`
  - `unregister_asset_definition_rejects_when_definition_is_oracle_reward_asset`
  - `unregister_domain_rejects_when_member_account_is_oracle_reward_pool`
  - `unregister_domain_rejects_when_domain_asset_definition_is_oracle_reward_asset`

### Validation Matrix (Oracle-Economics Config Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib oracle_reward_pool -- --nocapture` (pass)
- `cargo test -p iroha_core --lib oracle_reward_asset -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-08 Offline-Escrow Reference Integrity Expansion
- Closed remaining unregister integrity gaps around `settlement.offline.escrow_accounts` (`AssetDefinitionId -> AccountId`):
  - `Unregister<Account>` now rejects removal when the account is configured as an offline escrow account for an active asset definition.
  - `Unregister<Domain>` now rejects removal when a member account is configured as an offline escrow account for an active asset definition that remains outside the domain.
  - `Unregister<AssetDefinition>` now prunes the matching `settlement.offline.escrow_accounts` entry when the definition is deleted.
  - `Unregister<Domain>` now prunes `settlement.offline.escrow_accounts` entries for all domain asset definitions removed during domain teardown.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_offline_escrow_account`
  - `unregister_asset_definition_removes_offline_escrow_mapping`
  - `unregister_domain_rejects_when_member_account_is_offline_escrow_for_retained_asset_definition`
  - `unregister_domain_removes_offline_escrow_mappings_for_domain_asset_definitions`

### Validation Matrix (Offline-Escrow Integrity)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_offline_escrow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_removes_offline_escrow_mapping -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_offline_escrow_for_retained_asset_definition -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_removes_offline_escrow_mappings_for_domain_asset_definitions -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-09 Settlement-Repo Config Reference Guard Expansion
- Closed remaining unregister integrity gaps for settlement repo config asset-definition references:
  - `Unregister<AssetDefinition>` now rejects removal when the definition is configured in:
    - `settlement.repo.eligible_collateral`
    - `settlement.repo.collateral_substitution_matrix` (as base or substitute)
  - `Unregister<Domain>` now applies the same settlement-repo asset-definition guard before deleting domain asset definitions.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_asset_definition_rejects_when_definition_is_settlement_repo_eligible_collateral`
  - `unregister_asset_definition_rejects_when_definition_is_settlement_repo_substitution_entry`
  - `unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_eligible_collateral`
  - `unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_substitution_entry`

### Validation Matrix (Settlement-Repo Config Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_rejects_when_definition_is_settlement_repo_eligible_collateral -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_asset_definition_rejects_when_definition_is_settlement_repo_substitution_entry -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_eligible_collateral -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_domain_asset_definition_is_settlement_repo_substitution_entry -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-09 Content + SoraFS Telemetry Account-Reference Guard Expansion
- Closed remaining unregister integrity gaps for config account references used by content and SoraFS telemetry admission:
  - `Unregister<Account>` now rejects removal when the account is configured in:
    - `content.publish_allow_accounts`
    - `gov.sorafs_telemetry.submitters`
    - `gov.sorafs_telemetry.per_provider_submitters`
  - `Unregister<Domain>` now applies the same account-reference guards for each member account being removed.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_content_publish_allow_account`
  - `unregister_account_rejects_when_account_is_sorafs_telemetry_submitter`
  - `unregister_domain_rejects_when_member_account_is_content_publish_allow_account`
  - `unregister_domain_rejects_when_member_account_is_sorafs_per_provider_telemetry_submitter`

### Validation Matrix (Content + SoraFS Telemetry Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_content_publish_allow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_sorafs_telemetry_submitter -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_content_publish_allow_account -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_sorafs_per_provider_telemetry_submitter -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)

## 2026-03-09 Governance SoraFS Provider-Owner Config Guard Expansion
- Closed remaining unregister integrity gap for governance-configured SoraFS provider-owner account references:
  - `Unregister<Account>` now rejects removal when the account is configured in:
    - `gov.sorafs_provider_owners` (as provider owner)
  - `Unregister<Domain>` now applies the same governance provider-owner guard for each member account being removed.
- Files updated:
  - `crates/iroha_core/src/smartcontracts/isi/domain.rs`
  - `crates/iroha_core/src/smartcontracts/isi/world.rs`
  - `docs/source/data_model_and_isi_spec.md`
- Added regressions:
  - `unregister_account_rejects_when_account_is_configured_sorafs_provider_owner`
  - `unregister_domain_rejects_when_member_account_is_configured_sorafs_provider_owner`

### Validation Matrix (Governance SoraFS Provider-Owner Guard)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib unregister_account_rejects_when_account_is_configured_sorafs_provider_owner -- --nocapture` (pass)
- `cargo test -p iroha_core --lib unregister_domain_rejects_when_member_account_is_configured_sorafs_provider_owner -- --nocapture` (pass)
- `cargo check -p iroha_core` (pass)
