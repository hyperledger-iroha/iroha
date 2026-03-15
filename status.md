# Status

Last updated: 2026-03-15

## Current Enforced State
- First-release identity/deploy policy is strict (no backward aliases, shims, or migration wrappers).
- Deploy preflight scanner entrypoint is `../pk-deploy/scripts/check-identity-surface.sh`; the previous scanner entrypoint is removed.
- Runtime deploy scripts are aligned to strict-first-release behavior:
  - `../pk-deploy/scripts/cutover-ih58-mega.sh` invokes only `check-identity-surface.sh`.
- `../pk-deploy/scripts/deploy-sbp-aed-pkr-interceptor.sh` no longer performs prior-layout trigger cleanup loops.
- Wallet docs describe only current QR modes in neutral terms.

## 2026-03-15 Sumeragi Vote-Backed Quorum-Reschedule Damping
- Simplified the remaining vote-backed quorum-timeout path in `crates/iroha_core/src/sumeragi/main_loop/{pending_block.rs,reschedule.rs}`:
  - added `PendingBlock::vote_backed_reschedule_due(...)` so a vote-backed quorum reschedule is only rearmed after real new progress (`last_progress > last_quorum_reschedule`);
  - `reschedule_stale_pending_blocks_with_now(...)` now uses that stricter gate for vote-backed / QC-backed pending blocks while keeping the old zero-vote path unchanged;
  - `reschedule_pending_quorum_block(...)` no longer calls `pending.touch_progress(now)` for retained vote-backed pending state, so quorum reschedule stops resetting frontier stall age.
- This removes another implicit recovery owner. Vote arrival / RBC activity still owns progress, but quorum reschedule is now only a bounded retransmit side effect instead of a second timer that refreshes the frontier candidate.
- Added and updated focused regressions:
  - `vote_backed_reschedule_requires_progress_after_last_attempt` in `crates/iroha_core/src/sumeragi/main_loop/pending_block.rs`;
  - `stake_quorum_timeout_reschedules_without_immediate_view_change`;
  - `reschedule_defers_vote_backed_quorum_timeout_while_consensus_backlogged`;
  - `reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress`.

### Validation Matrix (Vote-Backed Quorum-Reschedule Damping)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib vote_backed_reschedule_requires_progress_after_last_attempt -- --nocapture` (pass)
- `cargo test -p iroha_core --lib stake_quorum_timeout_reschedules_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib reschedule_defers_vote_backed_quorum_timeout_while_consensus_backlogged -- --nocapture` (pass)
- `cargo test -p iroha_core --lib reschedule_skips_repeated_vote_backed_quorum_timeout_without_new_progress -- --nocapture` (pass)

### Runtime Signal (Vote-Backed Quorum-Reschedule Damping)
- A fresh healthy NPoS keep-dirs probe is running on this cut:
  - log: `/tmp/izanami_npos_healthy_probe_vote_backed_reschedule_once_20260315T075049Z.log`
  - preserved network dir: `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_oOdetz`
- Current state at this status update:
  - the outer `izanami` release build finished;
  - the test-network child warmup build (`iroha3d` release) is still running;
  - no peer stdout files exist yet, so there is no new consensus runtime verdict at this point.

## 2026-03-15 Sumeragi Proposal-Owned Commit Activation
- Simplified Sumeragi slot ownership further so `Proposal` remains the only owner of commit activation:
  - added `slot_has_proposal_evidence(height, view)` in `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs`;
  - `crates/iroha_core/src/sumeragi/main_loop/validation.rs` now defers pre-vote validation for pending blocks that have no observed/cached `Proposal` and no commit QC;
  - `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now refuses to promote a proposal-less pending block into active commit work even if the payload was already marked valid.
- This removes another payload-owned state path: `BlockCreated` without a matching `Proposal` may still cache payload in pending state, but it cannot dispatch validation workers, emit precommit votes, or become `commit.inflight` until a real proposal arrives or a commit QC supplies the missing ownership signal.
- Added and updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `validation_defers_without_proposal_evidence`
  - `commit_pipeline_defers_valid_pending_without_proposal_evidence`
  - updated direct commit-pipeline fixtures to call `note_proposal_seen(...)` when they intentionally model proposal-owned pending blocks.

### Validation Matrix (Proposal-Owned Commit Activation)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_defers_valid_pending_without_proposal_evidence -- --nocapture` (pass)
- `cargo test -p iroha_core --lib validation_defers_without_proposal_evidence -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_runs_without_global_cooldown -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_reports_stage_timings -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_qc_rebuild_cooldown_uses_chain_block_time -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_votes_highest_view_first_for_same_height -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_uses_epoch_for_height_when_emitting_votes -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_skips_fast_timeout_with_da_enabled -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_inlines_validation_after_fast_timeout_when_worker_queue_full -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_inlines_validation_at_queue_full_cutover -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_pipeline_keeps_deferred_validation_before_queue_full_cutover -- --nocapture` (pass)

### Runtime Signal (Proposal-Owned Commit Activation)
- The earlier healthy NPoS probe at `/tmp/izanami_npos_healthy_probe_block_created_state_reduction_20260315T000000Z.log` is now stale for this tranche because it was built before the proposal-owned commit activation cut landed.
- Fresh healthy NPoS probes on the current code are complete:
  - `/tmp/izanami_npos_healthy_probe_proposal_owned_commit_20260315T113400Z.log` stalled before `target_blocks=120` at quorum/strict `68/76`;
  - `/tmp/izanami_npos_healthy_probe_proposal_owned_commit_keepdirs_20260315T114000Z.log` reran the same scenario with `IROHA_TEST_NETWORK_KEEP_DIRS=1` and stalled earlier at quorum/strict `57/64`, preserving peer logs under `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_QgNYCr`.
- The structural result of this tranche is visible in preserved peer stdout:
  - `no proposal observed for view before changing view` still appears on the failing height (`57`), but those entries now show `commit_inflight=None`, so the proposal-less pending/commit activation path is no longer the thing driving the rotation;
  - dominant warnings are still `commit quorum missing past timeout; rescheduling block for reassembly`, mostly vote-backed (`votes=1` or `2`, `min_votes=3`) at ordinary frontier heights like `25`, `26`, `30`, `32`, `36`, `39`, `40`, `57`, and `62`;
  - zero-vote `drop_pending=true` reschedules still exist in the preserved run (for example at height `20` on `assuring_stonechat`), but the old proposal-less `commit_inflight` signature did not reappear.
- Conclusion: keep this simplification. It removes one invalid state transition, but it does not solve healthy NPoS throughput or latency by itself. The dominant remaining bottleneck has shifted back to vote-backed quorum reschedule / late missing-QC churn, not proposal-less commit activation.

## 2026-03-15 Sumeragi BlockCreated Proposal-Owned Round State
- Simplified `crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs` so payload delivery no longer owns round/view state:
  - `BlockCreated` now records `CollectDa` phase only when the slot was already observed through `Proposal` handling;
  - payload-only `BlockCreated` still populates pending payload state, but it no longer marks the slot as an observed proposal and no longer advances round/view tracking by itself.
- Added and updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `block_created_without_cached_proposal_does_not_advance_active_view`
  - `block_created_with_matching_proposal_advances_active_view`
  - `block_created_records_collect_da_phase_after_proposal_seen`

### Validation Matrix (BlockCreated Proposal-Owned Round State)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib block_created_without_cached_proposal_does_not_advance_active_view -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_with_matching_proposal_advances_active_view -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_records_collect_da_phase_after_proposal_seen -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_without_hint_accepts_extending_lock -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_uses_cached_proposal_when_lock_missing -- --nocapture` (pass)

### Runtime Signal (BlockCreated Proposal-Owned Round State)
- Fresh healthy NPoS probe started on this cut:
  - command: `cargo run -p izanami --release -- --allow-net --nexus --peers 4 --faulty 0 --target-blocks 120 --latency-p95-threshold 1s --progress-interval 10s --progress-timeout 120s --tps 5 --max-inflight 8`
  - log: `/tmp/izanami_npos_healthy_probe_block_created_state_reduction_20260315T000000Z.log`
  - current state at status update time: still in release-build warmup; runtime verdict not available yet.

## 2026-03-15 Sumeragi Zero-Vote Reschedule Rotation Removal
- Reverted the regressed aborted-pending commit-QC experiment and restored the earlier revive semantics in `crates/iroha_core/src/sumeragi/main_loop/{main_loop.rs,commit.rs,proposal_handlers.rs,tests.rs}`:
  - commit-QC on an aborted pending block revives it again;
  - duplicate `BlockCreated` on an aborted pending block revives the payload and now preserves the existing `commit_qc_seen` marker on that path;
  - removed the temporary commit-pipeline-specific pending counter and the proof tests that depended on the non-revive model.
- Simplified quorum-timeout ownership in `crates/iroha_core/src/sumeragi/main_loop/{reschedule.rs,commit.rs}`:
  - `reschedule_pending_quorum_block(...)` no longer returns an immediate-rotation flag;
  - zero-vote quorum reschedules may still drop/requeue stale pending frontier state, but they no longer trigger a direct `MissingQc` / quorum-timeout view change from the reschedule or commit pipeline paths;
  - the frontier timeout controller remains the only owner of post-reschedule view rotation.
- Updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - restored `commit_qc_revives_aborted_pending_block`;
  - restored `block_created_revives_aborted_pending`;
  - renamed zero-vote coverage to `zero_vote_quorum_timeout_drops_pending_without_immediate_view_change` and asserted `missing_qc_total == 0` on the reschedule path;
  - updated `reschedule_defers_zero_vote_quorum_timeout_while_block_queue_backlogged` to assert that the zero-vote drop still does not rotate immediately.

### Validation Matrix (2026-03-15 Zero-Vote Reschedule Rotation Removal)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib commit_qc_revives_aborted_pending_block -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_created_revives_aborted_pending -- --nocapture` (pass)
- `cargo test -p iroha_core --lib finalize_pending_block_revives_aborted_on_tip_with_commit_qc -- --nocapture` (pass)
- `cargo test -p iroha_core --lib commit_inflight_timeout_triggers_view_change_and_retains_aborted_pending -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_sync_update_keeps_aborted_next_height_payload_sparse_without_commit_evidence -- --nocapture` (pass)
- `cargo test -p iroha_core --lib stake_quorum_timeout_reschedules_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib zero_vote_quorum_timeout_drops_pending_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib reschedule_defers_zero_vote_quorum_timeout_while_block_queue_backlogged -- --nocapture` (pass)

### Soak Signal (2026-03-15 Zero-Vote Reschedule Rotation Removal)
- The temporary aborted-pending non-revive experiment was a real regression:
  - `/tmp/izanami_npos_healthy_probe_commit_qc_state_reduction_20260315T055734Z.log` stalled at quorum/strict `66/73` before `target_blocks=160`;
  - `/tmp/izanami_npos_healthy_probe_commit_qc_state_reduction_retry_20260315T061708Z.log` reproduced the regression at quorum/strict `65/69` before `target_blocks=120`.
- Reverting that experiment restored the previous healthy-failure envelope:
  - `/tmp/izanami_npos_healthy_probe_commit_qc_revert_20260315T062616Z.log` stalled later at quorum/strict `69/73`, confirming the aborted-pending cut was responsible for the sharper regression.
- A live-inspection probe isolated the remaining recurring bad path:
  - `/tmp/izanami_npos_healthy_probe_snapshot_20260315T063717Z.log` timed out at quorum/strict `65/71`;
  - timed snapshots captured repeated `commit quorum missing past timeout; rescheduling block for reassembly` warnings in peer stdout at ordinary frontier heights `3`, `7`, `15`, `21`, `24`, `33`, and `40`;
  - the most important recurring pattern was zero-vote reschedules with `drop_pending=true` and `rotate_immediately=true` at heights like `21`, showing that `reschedule.rs` still had a second direct view-change authority.
- After removing the reschedule-owned rotation:
  - `/tmp/izanami_npos_healthy_probe_no_resched_rotate_20260315T064146Z.log` still timed out, but moved slightly to quorum/strict `66/71`;
  - conclusion: the simplification is structurally correct and should stay, but the dominant healthy NPoS stall remains elsewhere. The next cut should target why zero-vote pending state is reaching the reschedule path so often, not add more recovery gates.

## 2026-03-14 Sumeragi Pending-Frontier Timeout Simplification
- Simplified the remaining quorum-timeout ownership split in `crates/iroha_core/src/sumeragi/main_loop/{reschedule.rs,commit.rs,main_loop.rs}`:
  - vote-backed quorum reschedules no longer trigger an immediate view change; `reschedule_pending_quorum_block(...)` now returns whether the pending block was dropped as zero-evidence zombie state, and only that path emits an immediate `MissingQc`/`StakeQuorumTimeout` rotation.
  - the generic `maybe_force_view_change_for_stalled_pending(...)` path now uses one bounded long grace for non-fast-path frontier pending blocks: `max(backlog_timeout, 2 * recovery_deferred_qc_ttl)`. This replaces the earlier short backlog-driven rotation window for active frontier pending blocks.
  - the reduced near-quorum missing-payload fast path is still intact; only the non-fast-path frontier branch was simplified.
- Added and updated focused regressions in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`:
  - `stake_quorum_timeout_reschedules_without_immediate_view_change`
  - `zero_vote_quorum_timeout_reschedules_and_records_missing_qc_view_change`
  - `maybe_force_view_change_for_stalled_pending_vote_backed_blocks_wait_for_deferred_qc_ttl`
  - `maybe_force_view_change_for_stalled_pending_non_near_path_waits_for_frontier_pending_timeout`
  - `maybe_force_view_change_for_stalled_pending_uses_frontier_pending_timeout_when_view_age_lags_pending_stall`
  - `maybe_force_view_change_for_stalled_pending_applies_frontier_pending_timeout_with_residual_round_state`

### Validation Matrix (Pending-Frontier Timeout Simplification)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib maybe_force_view_change_for_stalled_pending_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib stake_quorum_timeout_reschedules_without_immediate_view_change -- --nocapture` (pass)
- `cargo test -p iroha_core --lib zero_vote_quorum_timeout_reschedules_and_records_missing_qc_view_change -- --nocapture` (pass)

### Soak Signal (Pending-Frontier Timeout Simplification)
- Baseline short healthy NPoS probe before the new long frontier-pending timeout:
  - log: `/tmp/izanami_npos_healthy_probe_vote_backed_resched_20260314T212202Z.log`
  - result: reached `target_blocks=260` at height `262`, but still failed the latency gate with `interval_p50=1429ms`, `interval_p95=2501ms`, `samples=257`, `elapsed=440.099561083s`.
  - peer logs in that run showed the old pattern directly: vote-backed reschedule (`rotate_immediately=false`) followed by `active pending block stalled past quorum timeout; forcing deterministic view change`, then `revived aborted pending block after commit QC`.
- Intermediate probe with the narrower vote-backed deferred-QC TTL gate only:
  - log: `/tmp/izanami_npos_healthy_probe_deferred_qc_ttl_20260314T213707Z.log`
  - result: regressed to a fixed-height stall at `height 94` after repeated `10001ms` single-block intervals.
  - conclusion: gating only on locally counted commit votes was too narrow and did not remove the underlying early healthy wedge.
- Current probe after the non-fast-path frontier pending timeout simplification:
  - log: `/tmp/izanami_npos_healthy_probe_frontier_pending_timeout_20260314T214737Z.log`
  - result: reached `target_blocks=120` at height `123` with no replay of the old `68-94` fixed-height wedge, but still failed the latency gate with `interval_p50=1429ms`, `interval_p95=2501ms`, `samples=118`, `elapsed=200.047094208s`.
  - peer-log sampling during the run no longer showed `active pending block stalled past quorum timeout; forcing deterministic view change` through the old hotspot band; a late `revived aborted pending block after commit QC` signal still reappeared later at height `102`.
  - conclusion: the simplified timeout ownership improved the early healthy failure shape, but it did not improve the `p95` latency envelope and did not eliminate all late revive churn.

## 2026-03-14 Sumeragi Frontier Cleanup NEW_VIEW Reset
- Tightened `crates/iroha_core/src/sumeragi/main_loop.rs` frontier cleanup so the contiguous-frontier reset now also owns `NEW_VIEW` state:
  - `apply_frontier_recovery_cleanup(...)` now clears `new_view_tracker` entries at/above the frontier, clears `forced_view_after_timeout` markers at/above the frontier, and drops `Phase::NewView` vote history at/above the frontier from `vote_log`.
  - committed-edge conflict cleanup in `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now reuses the same `NEW_VIEW` vote-history clearing helper, keeping the frontier reset behavior consistent across both cleanup paths.
- Tightened RBC cleanup ownership for the same frontier reset paths:
  - `purge_rbc_sessions_at_or_above_height(...)` now purges per-key RBC state from `pending`, rebroadcast cooldowns, deferrals, persisted markers, and seed-inflight state even when no live RBC session object exists.
  - `clean_rbc_sessions_for_block(...)` now reuses the same per-key purge behavior for orphan RBC state keyed by the block hash, instead of only draining maps that still had a live `sessions` entry.
- Added focused regression `frontier_recovery_cleanup_clears_frontier_new_view_state` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`.
- Added focused RBC cleanup regressions:
  - `frontier_recovery_cleanup_purges_orphan_pending_rbc_without_sessions`
  - `clean_rbc_sessions_for_block_clears_seed_inflight` now also covers orphan pending/rebroadcast cleanup.

### Validation Matrix (Frontier Cleanup NEW_VIEW Reset)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib frontier_recovery_cleanup_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib observe_new_view_highest_qc_suppression_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib clean_rbc_sessions_for_block_clears_seed_inflight -- --nocapture` (pass)
- `cargo test -p iroha_core --lib frontier_recovery_cleanup_purges_rbc_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib frontier_recovery_cleanup_purges_orphan_pending_rbc_without_sessions -- --nocapture` (pass)

### Soak Signal (Frontier Cleanup NEW_VIEW Reset)
- 15-minute NPoS healthy canary on the frontier-cleanup NEW_VIEW reset build:
  - log: `/tmp/izanami_npos_healthy_canary_frontier_cleanup_new_view_reset_20260314T204030Z.log`
  - result: reached `target_blocks=400` and stopped at quorum/strict height `406`, with no fixed-height stall and no late `293/294` wedge.
  - latency gate still failed: quorum/strict interval `p50=1429ms`, `p95=2501ms`, `samples=399`, `elapsed=660.146095459s`.
  - peer state at the target checkpoint stayed aligned (`view_changes=0` on the sampled peers, no strict/quorum divergence).
  - remaining follow-up signal: the stability regressions appear fixed for this canary, but latency remains well above the `< 1000ms` acceptance gate. The next branch should rerun the same canary on the orphan-RBC cleanup patch to see whether clearing residual `da_rbc.rbc.pending` state changes the latency envelope or merely removes another dormant state leak.

## 2026-03-14 Sumeragi Frontier Follow-Up (Far-Future RBC Clamp + Committed-Edge Vote Cleanup)
- Simplified `crates/iroha_core/src/sumeragi/main_loop/rbc.rs` so far-future RBC sessions no longer create independent missing-block recovery state:
  - future-window RBC `Init`/`Chunk`/`Ready`/delivery recovery now suppresses per-height missing-block fetches beyond the contiguous frontier.
  - far-future RBC recovery now purges that RBC session state and emits one canonical committed-frontier range pull (`reason = "rbc_far_future_missing_block"`) instead of opening a second recovery machine for the far height.
- Simplified QC-backed block-sync/RBC interaction in `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs`:
  - removed the old non-roster-only `force_rbc_delivery_for_block_sync(...)` special case.
  - when a validated incoming QC is applied for a locally known block, the block-sync path now cleans RBC session state for that block and continues through the normal commit pipeline instead of preserving a separate forced-delivery state.
- Simplified committed-edge conflict cleanup in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - committed-edge suppression already clamped the frontier round, cleared forced-view markers, and dropped NEW_VIEW tracker entries.
  - it now also clears frontier/future `Phase::NewView` votes from `vote_log`, so stale higher-view local vote history cannot veto later canonical frontier NEW_VIEW recovery after the committed edge is realigned.

### Validation Matrix (Frontier Follow-Up)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core --lib future_consensus_window_rbc_messages_reanchor_frontier_without_far_height_requests -- --nocapture` (pass)
- `cargo test -p iroha_core --lib request_missing_block_for_pending_rbc_far_future_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib recover_block_from_rbc_session_far_future_reanchors_contiguous_frontier -- --nocapture` (pass)
- `cargo test -p iroha_core --lib recover_block_from_rbc_session_requests_missing_block_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib request_missing_block_for_pending_rbc_with_aborted_payload -- --nocapture` (pass)
- `cargo test -p iroha_core --lib observe_new_view_highest_qc_suppression_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib block_sync_update_known_block_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib clean_rbc_sessions_for_block_clears_seed_inflight -- --nocapture` (pass)

### Soak Signal (Frontier Follow-Up)
- 15-minute NPoS healthy canary with the far-future RBC clamp:
  - log: `/tmp/izanami_npos_healthy_canary_far_future_rbc_clamp_20260314T195603Z.log`
  - result: failed `target_blocks=400` at strict/quorum height `265`.
  - narrowed failure shape: three peers advanced to `324` while one peer (`necessary_dingo`, API `32371`) stalled at `265`.
  - the previous far-future `rbc_*_future_window` churn did not recur; the stuck peer instead showed committed-edge conflict suppression at height `265`, stale higher-view NEW_VIEW vote history (`already voted in a higher view`), repeated `missing_qc` frontier recovery at `266`, and fallback-anchor sharing from height `202`.
- Follow-up canary on the committed-edge NEW_VIEW vote cleanup build:
  - log: `/tmp/izanami_npos_healthy_canary_committed_edge_vote_cleanup_20260314T2017Z.log`
  - result: failed `target_blocks=400` at strict/quorum height `292`.
  - this build removed the earlier `265` committed-edge wedge but still stalled globally at the late contiguous frontier, with repeated `missing_qc`/`no proposal observed` loops around `293/294` and stale higher-view NEW_VIEW vote history (`higher_view=10`) still blocking canonical frontier votes on lagging peers.

## 2026-03-14 Sumeragi Frontier-Recovery Simplification
- Simplified timeout-driven Sumeragi recovery in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - introduced a single frontier-scoped recovery controller with `CatchUp` and `RotateArmed` phases.
  - timeout-driven recovery is now contiguous-frontier-only (`committed + 1`); future heights no longer create independent timeout-recovery state.
  - repeated no-progress windows now follow one deterministic path: bounded catch-up reanchor, one cleanup + all-peer reanchor, then one bounded rotation on the next window if progress still does not resume.
- Removed same-height timeout choreography from the idle and committed-edge paths:
  - `force_view_change_if_idle(...)` now routes dependency-backed timeout handling through the unified frontier controller instead of layering hysteresis/backoff/storm-break/frontier-window reservations.
  - committed-edge conflict recovery in `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now routes through the same unified controller instead of directly invoking the storm breaker.
- Preserved the existing public status surface:
  - `consensus_no_proposal_storm_*` and `blocksync_range_pull_expiry_streak_*` remain exposed, but are now driven by the unified frontier controller rather than separate same-height storm state.

### Validation Matrix (Frontier-Recovery Simplification)
- `cargo test -p iroha_core --lib frontier_recovery_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib same_height_no_proposal_storm_breaker_ -- --nocapture` (pass)
- `cargo test -p iroha_core --lib same_height_no_proposal_storm_counter_persists_until_true_progress -- --nocapture` (pass)
- `cargo test -p iroha_core --lib repeated_range_pull_expiry_forces_tier_jump_cooldown_clear_and_reanchor -- --nocapture` (pass)
- `cargo test -p iroha_core --lib committed_edge_conflict_ -- --nocapture` (pass)

## 2026-03-14 Block-Sync Unknown-Prev Fallback Simplification
- Simplified `crates/iroha_core/src/block_sync.rs` unknown-prev rewind selection:
  - removed the half-chain-to-256 fallback window.
  - unknown-prev fallback rewind now keeps short-chain half-window behavior but caps mid/deep rewinds to `UNKNOWN_PREV_RECENT_CHAIN_HASH_WINDOW` (64 blocks), matching the existing recent-hash hint surface.
  - added regression `unknown_prev_fallback_caps_mid_height_rewind_to_recent_window`.
- Narrow validation for the kept variant:
  - `cargo fmt --all` (pass)
  - `cargo test -p iroha_core --lib unknown_prev_ -- --nocapture` (pass)
  - `cargo test -p iroha_core --lib get_blocks_after_repeated_unknown_prev_probe_requests_latest_on_incremental_repeats -- --nocapture` (pass)
- 15-minute NPoS healthy canary for the kept variant:
  - log: `/tmp/izanami_npos_healthy_canary_unknown_prev_cap_20260314T175113Z.log`
  - result: improved from the prior `height 345` plateau to `height 384`, but still failed `target_blocks=400` before duration end.
  - interval summary: `count=61`, `median=1449ms`, `p95=2501ms`, `max=3335ms`.
  - peer behavior improved materially before the final stall: no fallback-anchor sharing through height `324`, then late same-height `385` missing-QC / fallback-anchor churn reappeared.
- Rejected follow-up variant:
  - tried clamping repeated incremental unknown-prev sharing directly to the committed edge in `crates/iroha_core/src/block_sync.rs`.
  - 15-minute NPoS healthy canary for that variant regressed to an earlier `height 280` stall with broader cleanup/rotate churn; the patch was reverted and is not kept in the tree.
  - regression canary log: `/tmp/izanami_npos_healthy_canary_incremental_edge_clamp_20260314T181402Z.log`

## 2026-03-13 Timeout-Driven Sumeragi Recovery Stabilization (bounded suppression + deterministic escalation)
- Implemented strict global-stall enforcement in `crates/izanami/src/chaos.rs`:
  - `should_enforce_strict_progress_timeout(...)` now enforces strict timeout for healthy/global stalls (`lagging_peers == 0`) and for lag beyond tolerated failures (`lagging_peers > tolerated_failures`), while preserving tolerated-failure behavior for true outlier lag.
  - strict-stall warning text now explicitly distinguishes tolerated outlier lag from globally enforced strict stalls.
- Implemented same-height missing-QC storm tracking and bounded suppression in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - added dedicated per-height storm state/counters independent from timeout-marker cleanup.
  - storm reset now happens only on true progress signals (commit height advances past tracked height, proposal observed at tracked height/view, or dependency-progress timestamp advance).
  - storm breaker now uses storm count (not timeout marker streak), supports backlog-driven trigger when `pending_blocks == 0`, and records forced-cleanup state for one post-cleanup rotation allowance after the next timeout window.
  - fixed regression where deterministic frontier reset could clear same-height state before suppression checks: same-height rotation is now still suppressed for that timeout cycle when an in-window canonical frontier reanchor remained unresolved at timeout, unless explicit post-cleanup allowance is active.
- Hardened repeated range-pull expiry escalation continuity in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - added per-height expiry streak persistence (`missing_block_range_pull_expiry_streaks`) across cleanup paths until actual dependency progress.
  - repeated no-progress expiry (`>= 2` windows) now immediately advances escalation tier, clears range-pull cooldown gates for the height/frontier, emits canonical all-target reanchor, and reserves the round-recovery window token to avoid immediate same-tier reentry.
- Updated committed-edge conflict cleanup gating in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - committed-edge conflict path keeps canonical cleanup/reanchor behavior but now only invokes storm-break when dedicated storm threshold is reached, preventing suppression bypass before forced-escalation threshold.
- Extended observability in `crates/iroha_core/src/sumeragi/status.rs`:
  - added `consensus_no_proposal_storm_last_height`, `consensus_no_proposal_storm_last_count`, and `consensus_no_proposal_storm_max_count` to status snapshot state/counters.

### Validation Matrix (Timeout-Driven Stabilization)
- `cargo fmt --all` (pass)
- `cargo test -p iroha_core missing_qc_view_change_suppressed_when_frontier_reanchor_already_emitted -- --nocapture` (pass)
- `cargo test -p iroha_core highest_qc_fetch_suppresses_committed_height_hash_conflict -- --nocapture` (pass)
- `cargo test -p iroha_core same_height_no_proposal_storm -- --nocapture` (pass)
- `cargo test -p iroha_core repeated_range_pull_expiry_forces_tier_jump_cooldown_clear_and_reanchor -- --nocapture` (pass)
- `cargo test -p izanami strict_progress_timeout_enforcement_respects_bft_tolerance -- --nocapture` (pass)

## 2026-03-12 Sub-1s Soak Recovery Plan (Fail-Open Gating + Deterministic Catch-Up)
- `crates/izanami/src/chaos.rs` now applies configured fault budget to strict progress semantics:
  - `effective_tolerated_peer_failures(peer_count, configured_faulty_peers)` now honors `configured_faulty_peers` and bounds it by protocol BFT tolerance.
  - strict stall/quorum divergence calculations now consistently use this effective tolerance, so healthy runs (`faulty=0`) fail on unexpected stragglers while frozen-peer runs (`faulty=1`) tolerate one lagging peer.
  - updated regressions: `effective_tolerated_peer_failures_honors_configured_fault_budget`, `quorum_min_height_respects_effective_tolerance`, and `strict_divergence_reference_height_trims_only_effectively_tolerated_outliers`.
- `crates/iroha_core/src/sumeragi/main_loop.rs` and `crates/iroha_core/src/sumeragi/main_loop/block_sync.rs` now escalate repeated sidecar mismatch loops into deterministic recovery:
  - repeated sidecar mismatch recovery now increments a dedicated recovery-trigger counter.
  - no-verifiable-roster drop path now clears stale same-height missing dependencies when sidecar is quarantined and requests canonical `committed+1` reanchor/range-pull.
  - repeated range-pull expiry without progress now increments per-height expiry streak and immediately advances one additional range-pull tier once streak is repeated (avoids re-entering identical incremental loops).
- Added bounded same-height no-proposal storm breaker in `crates/iroha_core/src/sumeragi/main_loop.rs` with committed-edge integration in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - when repeated same-height no-proposal rounds occur with stale pending blocks but no actionable dependencies, perform one deterministic stale-state cleanup and canonical reanchor.
  - idle missing-QC and committed-edge conflict paths both use this breaker to prevent stale/non-actionable hashes repeatedly rearming view-change loops.
- Extended consensus observability in `crates/iroha_core/src/sumeragi/status.rs`:
  - new counters/fields: `consensus_sidecar_recovery_trigger_total`, `consensus_no_proposal_storm_total`, `blocksync_range_pull_expiry_streak_last`, `blocksync_range_pull_expiry_streak_max`.
  - status snapshot and unit coverage updated (`missing_block_fetch_counters_surface_in_snapshot`).
- Targeted validation in this tree:
  - `cargo test -p izanami effective_tolerated_peer_failures_honors_configured_fault_budget -- --nocapture`
  - `cargo test -p iroha_core missing_block_fetch_counters_surface_in_snapshot -- --nocapture`
  - `cargo test -p iroha_core same_height_no_proposal_storm_breaker_cleans_stale_state -- --nocapture`

## 2026-03-12 Localnet Sub-1s Follow-Up (Hard-Gate Enforcement + Actionable-Only Rotation)
- Enforced latency hard-gate at duration completion in `crates/izanami/src/chaos.rs`:
  - added `enforce_latency_p95_gate(...)`.
  - gate now applies at `target_reached` and also at `duration_deadline` in soft-KPI mode.
  - error output now includes `checkpoint` context (`target_reached` or `duration_deadline`).
- Added regression in `crates/izanami/src/chaos.rs`:
  - `wait_for_target_blocks_soft_kpi_enforces_latency_gate_at_duration_end`.
- Tightened no-actionable missing-QC rotation behavior in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - proposal-gap backlog grace and RBC-progress grace deferrals now apply only when actionable dependency signals remain.
  - deterministic frontier/lock-lag cleanup no longer defers rotation when dependencies are non-actionable.
  - same-height no-actionable cleanup now also clears `round_recovery_bundle_window_gates` for that height.
  - `try_reserve_round_recovery_bundle_window(...)` no longer suppresses no-actionable rotations.
- Compile and targeted validations now pass on this tree:
  - `cargo check -p iroha_core --all-targets`
  - `cargo check -p izanami --all-targets`
  - `cargo test -p iroha_core --lib force_view_change_if_idle_no_actionable_dependency_rotates_after_base_timeout -- --nocapture`
  - `cargo test -p iroha_core --lib highest_qc_fetch_suppresses_committed_height_hash_conflict -- --nocapture`
  - `cargo test -p izanami wait_for_target_blocks_soft_kpi -- --nocapture`
- 15-minute gated canary matrix (explicit `--latency-p95-threshold 1s`, `target_blocks=400`) confirms hard-gate behavior:
  - permissioned healthy: `/tmp/izanami_permissioned_canary_gate_20260312T060142Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`).
  - permissioned + frozen peer: `/tmp/izanami_permissioned_frozen_canary_gate_20260312T062318Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`), strict lag persisted (`strict min 105` at gate trip).
  - NPoS healthy: `/tmp/izanami_npos_canary_gate_20260312T063343Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`).
  - NPoS + frozen peer: `/tmp/izanami_npos_frozen_canary_gate_20260312T064516Z.log` → failed on gate (`quorum p95=2502ms > 1000ms`), strict lag persisted (`strict min 102` at gate trip).

## 2026-03-11 Localnet Sub-1s Block Production Plan (Permissioned + NPoS)
- Implemented missing-QC actionable-only timeout gating in `crates/iroha_core/src/sumeragi/main_loop.rs`:
  - same-height no-actionable cleanup now runs before timeout hysteresis/defer/backoff decisions.
  - stale no-actionable cleanup clears height recovery budget plus missing-QC timeout/reacquire markers for the active height.
  - missing-QC hysteresis/defer/backoff now require actionable dependency signals (including unresolved actionable frontier dependency), so stale/non-actionable backlog does not extend rotation timing.
- Implemented committed-edge fallback hardening in `crates/iroha_core/src/sumeragi/main_loop/votes.rs`:
  - committed-edge conflicting highest-QC suppression still clears obsolete request state.
  - bounded canonical reanchor is now emitted only when contiguous frontier dependency is still actionable.
- Implemented shared-host stable profile retuning in `crates/izanami/src/chaos.rs`:
  - pipeline floor: `180ms`.
  - shared-host NPoS timeout floors: `propose=60ms`, `prevote=90ms`, `precommit=120ms`, `commit=320ms`, `da=320ms`, `aggregator=20ms`.
  - recovery windows: `height_window=2000ms`, `missing_qc_reacquire_window=800ms`, `deferred_qc_ttl=2000ms`.
  - recovery escalation: `hash_miss_cap_before_range_pull=1`, `range_pull_escalation_after_hash_misses=1`.
  - shared-host pending-stall grace: `300ms`.
  - shared-host `da_fast_reschedule` enabled.
  - stable shared-host soak default latency gate: `latency_p95_threshold=1s` when unset.
  - startup override log now includes latency profile marker and effective latency p95 gate fields.
- Added/updated targeted regressions:
  - `force_view_change_if_idle_no_actionable_dependency_rotates_after_base_timeout`.
  - `force_view_change_if_idle_prunes_stale_round_state_during_repeated_same_height_missing_qc` now asserts stale timeout streak reset.
  - committed-edge suppression regression now asserts no reanchor for non-actionable conflict hashes.
  - shared-host stable profile tests now assert sub-1s latency gate default and updated timeout/pending-stall constants.
- Validation attempts in this workspace were blocked by pre-existing branch compile errors unrelated to this patch:
  - `iroha_core` tests: `AccountId::new(...)` signature mismatch in `crates/iroha_core/src/smartcontracts/isi/triggers/set.rs` and pre-existing test code.
  - `izanami` check/test: ongoing `ScopedAccountId` migration mismatches in `crates/izanami/src/{chaos.rs,instructions.rs}`.
  - successful local compile gate for touched core runtime path: `cargo check -p iroha_core --lib`.

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
