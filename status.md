# Status

Last updated: 2026-04-23

## 2026-04-23 Alias auto-renew regression fixes

- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now unregisters the currently executing alias auto-renew billing trigger before queueing the replacement trigger, so successful and retryable billing runs can reschedule themselves without duplicate-trigger rejection.
- `crates/iroha_config/src/parameters/user.rs` now defaults `torii.onboarding.alias_auto_renew_enabled` to `false` until `alias_auto_renew_subscription_domain` is configured, and the optional onboarding subtree now carries explicit Norito defaults so upgraded configs keep their documented defaults when the new field is omitted.
- `crates/iroha_torii/src/routing.rs` now treats account-alias auto-renew NFTs as a special resume path: `/v1/subscriptions/{id}/resume` no longer requires `subscription_plan` metadata for these NFTs, resets failure state, preserves the current billing window, and rebuilds the billing trigger with the resumed charge time.
- `crates/iroha_torii/tests/accounts_onboard.rs`, `crates/iroha_torii/src/routing.rs`, `crates/iroha_config/src/parameters/user.rs`, and `crates/iroha_core/src/smartcontracts/ivm/host.rs` now cover the reschedule fix, the safe onboarding default, the no-domain onboarding path, and alias auto-renew resume compatibility.
- Focused validation for this fix set:
  - `cargo test -p iroha_core subscription_bill_account_alias_auto_renew_queues_renewal_and_reschedules -- --nocapture`
  - `cargo test -p iroha_config onboarding_alias_auto_renew_defaults_disabled_without_subscription_domain -- --nocapture`
  - `cargo test -p iroha_torii handle_post_v1_subscription_resume_supports_alias_auto_renew_nfts -- --nocapture`
  - `cargo test -p iroha_torii --test accounts_onboard without_auto_renew_subscription_domain_when_disabled -- --nocapture`

## 2026-04-23 Account-alias lease coverage expansion

- `crates/iroha_core/src/sns.rs` now covers two more SNS quote guards for the paid alias lease path: registration rejects an already-registered selector, and renewal rejects a tombstoned alias.
- `crates/iroha_core/src/smartcontracts/isi/sns.rs` now covers the executor guard branches that reject a mismatched payer on `AcquireAccountAliasLease` and reject `RenewAccountAliasLease` when the caller is neither the lease owner nor a `CanManageAccountAlias` holder.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now covers the SNS-specific subscription billing branch directly, including the paid auto-renew success path that queues a canonical `RenewAccountAliasLease` and the missing-alias failure path that suspends the subscription and records a failed invoice.
- `crates/iroha_torii/tests/accounts_onboard.rs` now also verifies the onboarding response lease block plus `GET /v1/accounts/{account_id}/aliases` after onboarding, so the new app-facing lease DTOs and alias-list route are exercised at integration level.
- Focused validation for this coverage follow-up:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_torii --test accounts_onboard -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib quote_account_alias_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib acquire_account_alias_lease_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib renew_account_alias_lease_rejects_non_owner_without_permission -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib subscription_bill_account_alias_auto_renew_ -- --nocapture`

## 2026-04-23 Canonical paid account-alias leases and auto-renew

- `crates/iroha_data_model` now exposes canonical `AcquireAccountAliasLease` / `RenewAccountAliasLease` ISIs plus SNS account-alias auto-renew metadata on subscriptions.
- `crates/iroha_core/src/sns.rs` now quotes paid account-alias registration/renewal lifecycles, and `crates/iroha_core/src/smartcontracts/isi/sns.rs` executes the canonical paid lease acquire/renew path instead of relying on implicit runtime lease seeding.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now recognizes subscription NFTs tagged with account-alias auto-renew metadata and bills them through canonical alias renewal instead of the generic fixed/usage transfer path.
- `crates/iroha_torii/src/routing.rs` now makes `/v1/accounts/onboard` and `/v1/accounts/onboard/multisig` acquire a real finite alias lease in the queued transaction, exposes alias lease status / renew / auto-renew endpoints, and returns lease state in onboarding responses.
- `crates/iroha_torii/tests/accounts_onboard.rs`, `crates/iroha_core/src/sns.rs`, and `crates/iroha_core/src/smartcontracts/isi/sns.rs` now cover the paid onboarding flow, SNS quote helpers, and executor-level acquire/renew round trip.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_torii --test accounts_onboard -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib quote_account_alias_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib acquire_and_renew_account_alias_lease_round_trip -- --nocapture`
  - `git diff --check`

## 2026-04-23 Sumeragi helper guard coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds three more direct guard/fallback tests in the same quorum-target / frontier-wire slice so the remaining conservative-degrade branches are covered without relying on larger end-to-end fixtures.
- Added `quorum_retransmit_targets_fall_back_to_full_fanout_when_signer_mapping_fails`, which proves invalid signer-to-peer mapping degrades to full retransmit fanout instead of dropping recovery traffic.
- Added `frontier_block_created_for_local_proposal_wire_falls_back_to_first_block_signature`, which proves the local proposal wire helper still emits enriched frontier metadata when the proposal’s proposer index does not match any signature on the block.
- Added `frontier_block_created_for_wire_returns_plain_block_without_frontier_metadata`, which proves the generic wire helper degrades to a plain `BlockCreated` when neither proposal cache nor authoritative frontier metadata is available.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib quorum_retransmit_targets_fall_back_to_full_fanout_when_signer_mapping_fails -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_first_block_signature -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_wire_returns_plain_block_without_frontier_metadata -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_uses_live_roster_when_derived_roster_unavailable -- --nocapture`

## 2026-04-23 Sumeragi helper coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds three more direct helper tests in the quorum-target / frontier-proposal slice so the remaining positive and negative fallback branches are exercised without relying on larger pacemaker or reschedule fixtures.
- Added `quorum_retransmit_targets_expand_to_full_fanout_near_commit_quorum`, which proves the helper widens back to every remote peer once the round is one vote short of commit quorum and only a single canonical target appears to be missing.
- Added `frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache`, which proves the generic proposal-wire helper can rebuild enriched `BlockCreated` payloads from authoritative frontier metadata after a prior local `BlockCreated`.
- Added `frontier_block_created_for_proposal_wire_rejects_authoritative_frontier_cache_when_metadata_mismatches`, which proves the generic proposal-wire helper will not reuse cached authoritative metadata once the proposal header no longer matches it.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib quorum_retransmit_targets_expand_to_full_fanout_near_commit_quorum -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_rejects_authoritative_frontier_cache_when_metadata_mismatches -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_rebuilds_authoritative_frontier_metadata -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Sumeragi fallback coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds two more direct branch tests around the quorum-target / cached-frontier fixes instead of relying only on the higher-level regression fixtures.
- Added `precommit_vote_falls_back_to_seeded_collectors_when_quorum_targets_are_satisfied`, which proves `emit_precommit_vote(...)` keeps the seeded collector subset as the fallback target once cached remote commit votes already satisfy quorum-target retransmit selection.
- Added `frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache`, which proves local proposal wire rebuilds can recover from authoritative frontier metadata after a prior enriched `BlockCreated`, even when the roster hint is unavailable.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib precommit_vote_falls_back_to_seeded_collectors_when_quorum_targets_are_satisfied -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib emit_precommit_vote_targets_quorum_retransmit_peers -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_uses_live_roster_when_derived_roster_unavailable -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Sumeragi quorum-target / frontier rebroadcast follow-up

- `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now sends the first local precommit to `quorum_retransmit_targets_for_missing_votes(...)` instead of the seeded collector subset, while still preserving the collector seed state for later retry widening.
- `crates/iroha_core/src/sumeragi/main_loop/propose.rs` now rebuilds cached frontier `BlockCreated` rebroadcasts with `frontier_block_created_for_local_proposal_wire(...)` so a cached local-leader proposal can still emit an enriched payload rebroadcast without a preseeded RBC session.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now aligns the READY-quorum and cached-frontier pacemaker fixtures with PRF/view-aligned sender and leader selection, and the older initial-vote assertions now expect quorum retransmit peers instead of the seeded collector subset.
- Focused validation for this follow-up:
  - `cargo test -p iroha_core --lib commit_vote_targets_collectors_or_topology -- --nocapture`
  - `cargo test -p iroha_core --lib precommit_vote_targets_collectors_without_broadcast -- --nocapture`
  - `cargo test -p iroha_core --lib emit_precommit_vote_targets_quorum_retransmit_peers -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_after_ready_quorum_without_all_chunks -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Sumeragi main_loop coverage edge follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds five more direct unit tests in the recent known-block replay / exact-frontier helper slice so the remaining explicit-target and local-only frontier branches are exercised directly.
- Added a deduplicated explicit-target vote replay case for `maybe_replay_known_block_commit_evidence(...)` that proves duplicate and local peers collapse to one outbound `QcVote` send per unique remote target.
- Added a cached commit-QC replay case for `maybe_replay_known_block_commit_evidence(...)` that proves the helper returns `false` when the explicit target set collapses to the local peer only and therefore the QC replay path has no outbound work to schedule.
- Added three more `frontier_body_next_due(...)` checks: unarmed exact-fetch slots stay idle even with cached targets, leader-stage retries stay idle when a single-peer topology yields no remote leader or voters, and voter-stage retries stay armed when at least one cached remote voter survives leader filtering.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib known_block_commit_evidence_replay_deduplicates_explicit_vote_targets -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_qc_replay_returns_false_for_local_only_explicit_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_exact_fetch_disabled_slot_even_with_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_leader_stage_stays_idle_without_remote_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_uses_cached_remote_voters_after_leader_filter' --nocapture`

## 2026-04-23 Sumeragi main_loop coverage tail follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds four more direct unit tests in the recent known-block replay / exact-frontier helper slice instead of relying on broader regressions to incidentally hit those branches.
- Added a per-block cooldown case for `maybe_replay_known_block_commit_evidence(...)` that proves the second replay attempt is suppressed before any duplicate network work is scheduled.
- Added a local-only explicit-target case for `maybe_replay_known_block_commit_evidence(...)` that proves the helper returns `false` when the explicit peer set collapses to self and therefore cannot emit outbound recovery traffic.
- Added two exact-frontier cached-target shape checks for `frontier_body_next_due(...)`: leader-stage retries remain armed when only cached voters exist, and voter-stage retries stay idle when the cached voter set collapses to the cached leader only.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib known_block_commit_evidence_replay_skips_during_cooldown -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_evidence_replay_returns_false_for_local_only_explicit_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_leader_stage_uses_cached_voters_without_leader' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_ignores_cached_leader_only_voter_set' --nocapture`

## 2026-04-22 Torii zk roots coverage sweep

- `crates/iroha_torii/src/routing.rs` now covers the remaining `/v1/zk/roots` selector, gas-default, and `Accept` negotiation branches, including blank selectors, trimmed and canonical gas-asset handling, custom-vs-pipeline fallback precedence, malformed `Accept` handling, vendor `+json`, wildcard, and zero-quality Norito fallback cases.
- `crates/iroha_torii/tests/zk_endpoints.rs` now pins the route-level registered-asset empty-state `200`, parseable-but-missing asset alias `404`, missing asset `404`, and blank/invalid selector `403` regressions.
- `crates/iroha_torii/tests/zk_roots_handler_integration.rs` now covers bounded nonzero `max`, zero-cap empty windows that preserve `latest` and `height`, trimmed alias resolution against non-empty shielded state, Norito decoding for non-empty roots windows, and forwarded unsupported `Accept` headers returning `406`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --lib zk_roots_selector_tests -- --nocapture`
  - `cargo test -p iroha_torii --test zk_endpoints --test zk_roots_handler_integration -- --nocapture`
  - `git diff --check`

## 2026-04-22 Sumeragi main_loop coverage follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds direct unit coverage for the recent collector / vote-replay / exact-frontier branches instead of only relying on the earlier regression fixtures.
- Added a collector-disabled precommit case that proves `emit_precommit_vote(...)` falls back to the full view-aligned topology when no seeded collector set exists.
- Added a local accepted commit-vote case that proves `handle_vote(...)` records the vote without triggering known-block commit-evidence replay or payload recovery fanout.
- Added direct `frontier_body_next_due(...)` coverage for both derived-target paths: leader-stage scheduling now stays armed when live-roster derivation can supply fetch targets, while voter-stage scheduling still stays idle when derivation yields only the local leader and no remote voters.
- Added direct `known_block_commit_qc_recovery_targets(...)` coverage for both empty-input fallback branches: cached vote-roster recovery for far-future rounds with preserved roster evidence, and live commit-topology recovery when no vote-roster evidence exists.
- Added two more exact-frontier scheduler checks: far-future slots now prove the final fallback to `effective_commit_topology()`, and body-present slots now prove exact fetch stays idle when no same-round commit-QC repair is active.
- Added direct `deterministic_elected_roster_from_candidates(...)` coverage for two narrow roster-election guards: zero requested roster length now proves the helper still elects one candidate, and NPoS `max_validators` now proves candidate truncation happens before the requested roster-size cap.
- Added direct `frontier_body_next_due(...)` deadline checks for the two timing branches that were still only covered indirectly: before ingress grace elapses the helper returns `observed_at + authoritative_body_ingress_fetch_grace()`, and after grace elapses it returns `last_fetch_at + retry_window`.
- Added direct `maybe_replay_known_block_commit_evidence(...)` coverage for the “no new progress” guard: once the same round’s replay state has already been sent and neither vote count nor commit-QC state changes, the helper now has a focused test proving it returns `false` without scheduling more traffic.
- Added direct NPoS roster-unavailability source coverage for the empty-commit-topology path: one test proves the helper stays on the multi-peer local lane when that lane is available, and another proves it falls back to the full published active validator roster when the local validator has no lane assignment.
- Added two more exact-frontier due checks: body-present slots still schedule retries when same-round commit-QC repair remains actionable, and passive-catchup slots stay excluded from exact body retry scheduling even when cached targets exist.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib precommit_vote_falls_back_to_topology_when_collectors_disabled -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::local_accepted_commit_vote_does_not_replay_known_block_evidence' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_derives_targets_from_live_roster_when_slot_cache_is_empty' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_requires_remote_voters_after_derivation' --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_recovery_targets_fall_back_to_cached_vote_roster -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_qc_recovery_targets_fall_back_to_effective_commit_topology' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_falls_back_to_effective_commit_topology_when_live_roster_is_empty' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_body_present_without_commit_qc_repair' --nocapture`
  - `cargo test -p iroha_core --lib deterministic_roster_election_keeps_one_candidate_when_target_len_is_zero -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::deterministic_roster_election_truncates_npos_candidates_to_max_validators' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_returns_ingress_grace_deadline_before_grace_elapses' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_returns_retry_deadline_after_grace_elapses' --nocapture`
  - `cargo test -p iroha_core --lib roster_unavailability_candidate_source_npos_uses_local_lane_when_commit_topology_empty -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::roster_unavailability_candidate_source_npos_uses_full_active_roster_when_local_lane_missing' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_evidence_replay_skips_without_new_progress' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_keeps_retry_armed_when_body_present_but_commit_qc_repair_active' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_passive_catchup_slot_even_with_targets' --nocapture`

## 2026-04-22 Sumeragi targeted main_loop regression sweep

- `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now keeps the initial local commit/precommit emit on the seeded collector set plus explicit parallel fanout, instead of widening that very first send through the generic commit-evidence replay path.
- `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now limits automatic known-block commit-evidence replay to non-local accepted commit votes, so a locally emitted precommit is not redundantly fanned out to the whole topology before the collector retry logic takes over.
- `crates/iroha_core/src/sumeragi/main_loop.rs` now lets `frontier_body_next_due(...)` account for derived voter fallback when the leader lane has no usable remote leader target, and NPoS roster-unavailability candidate selection now scopes lane discovery from the active topology rather than the raw cached snapshot.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now uses PRF-aligned leader/view discovery for the affected proposal and vote fixtures, searches RBC subset cases without assuming the local peer is outside the deterministic rebroadcast base set, and expects partial-session payload rebroadcasts to queue only the chunks that are still locally present.
- This clears the reported `sumeragi::main_loop` failure cluster around collector targeting, idle frontier recovery, RBC rescue/rebroadcast fixtures, same-slot vote-collision validation, exact-frontier retry scheduling, and roster-unavailability source selection.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib commit_vote_targets_collectors_or_topology -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::assemble_proposal_height_two_records_inline_frontier_manifest' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::commit_vote_targets_collectors_or_topology' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::force_view_change_if_idle_arms_nonleader_empty_frontier_recovery_after_pacemaker_attempt' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::maybe_emit_rbc_deliver_prefers_targeted_ready_rescue_when_subset_skips_local' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::pending_validation_preserves_same_slot_signature_collisions_until_identity_validation' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::precommit_vote_skips_payload_broadcast_for_aborted_pending' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::precommit_vote_targets_collectors_without_broadcast' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::rbc_payload_rebroadcast_allows_derived_roster' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::retry_missing_block_requests_defers_view_change_when_queue_blocks_seen' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::roster_unavailability_candidate_source_matches_consensus_mode' --nocapture`

## 2026-04-22 Roadmap Audit

- `roadmap.md` was re-audited against the current repo and rewritten around repo-backed unfinished issues instead of speculative or stale reminders.
- Removed the obsolete `iroha_torii_shared` `AccountLabel` import blocker; the current file no longer imports `AccountLabel`.
- Removed the old trigger `get_numeric(...)` follow-up; the free helper has already been removed in `kotodama_lang`, so any future trigger work must be scoped to a concrete repo-local test gap.
- Promoted concrete open gaps into the roadmap where evidence exists today, including the `ivm_prebuild.rs` / `integration_tests/build.rs` sample-list drift and the lack of a repo-wide translation metadata audit.

## 2026-04-22 Roadmap Cleanup

- `roadmap.md` now tracks unfinished work only. Completed history was removed from the roadmap so it stops acting like a second status log.
- Historical completed roadmap-only epics were archived here at summary level instead of staying in the roadmap:
  - Kagami NPoS-ready network generation, explicit consensus-mode cutover tooling, peer/block signature hardening, and bootstrap-from-trusted-peer genesis fetch are complete.
  - Kagami/Mochi Iroha3 profiles, account-identity / alias unification, CLI output normalization, CI TODO cleanup, and offline QR storm follow-up work are complete.
  - Soracloud platform MVP and Soracloud model-training / weight-lifecycle work are complete.
  - Kotodama / IVM developer-experience observability work is complete.
  - Asset ID, account-literal, and asset-alias lease cleanup follow-ups are complete.
  - The tracked repository TODO inventory had no actionable code/runtime/SDK/test/tooling/docs TODOs at the time of the scan; only reference-only TODO mentions remained.

## 2026-04-22 Recent Completed Follow-ups

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now salts harness-generated peer seeds per instance and folds the elected signer into the helper heartbeat seed, which eliminates helper block-hash collisions across parallel RBC roster tests and closes the stale-cache replacement coverage gaps around same-epoch roster recovery.
- `crates/iroha_torii/src/zk_attachments.rs` and `crates/iroha_torii/tests/zk_attachments_subprocess.rs` now cover the remaining sanitizer subprocess branches exercised in the latest pass: launcher fallback, timeout handling, response decoding, malformed compressed payloads, child-entry validation, spawn failures, and reader timeout/error paths.
- The recent `iroha_core` frontier-slot helper slices are complete: pending-wrapper liveness, active-owner-state helpers, validation-inflight preservation, commit-inflight preservation, and later-view lock-state guard coverage were all added with focused tests.
- The recent `iroha_torii` manifest-routing slices are complete: helper coverage, direct handler coverage, malformed-payload coverage, and direct-routing coverage are all in place with focused test runs.
- The recent `iroha_data_model` bridge helper / verifier / serialization follow-up slices are complete with focused crate validation.

## 2026-04-22 Completed Follow-up Archive

- Permissioned preserved-peer stable soak: green on the patched tree; the remaining follow-up is the fresh-binary rerun from the current branch.
- Permissioned missing-QC recovery: landed, covered by focused regressions, and aligned with the current reschedule / local-vote behavior.
- Retained RBC summary refresh: landed; the previously failing large-payload NPoS regression is green.
- Permission-cache replay: landed and green.
- Integration failure sweep: the reported targeted regressions were fixed and covered by focused validation.
- MCP writer-profile alignment: landed; mutation-capable MCP endpoint tests now opt into the documented writer profile and the targeted test file is green.
