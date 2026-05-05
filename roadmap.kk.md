---
lang: kk
direction: ltr
source: roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8b9a2aedf3373db18b457089332fa311ce03bf4e580343405cbd996fbc132ae
source_last_modified: "2026-02-04T18:15:01.089956+00:00"
translation_last_reviewed: 2026-02-07
---

# Roadmap (Open Work Only)

This roadmap enumerates the outstanding efforts required to ship the optional
NPoS Sumeragi mode and keep the broader Nexus transition on track. For every task listed here we are preparing the first public release, so teams can design and implement with a clean slate. Completed
items continue to live in `status.md`; only tasks that still need engineering
work appear here.
SDK parity note: Swift offline receipt vectors are now synced to the canonical artifacts; no new SDK open items were added.
Latest sync: Pipeline events now batch via `EventBox::PipelineBatch` (Torii WS/SSE consumers expand batches); DA shard cursor journal persistence now runs in a background worker during block apply; consensus payload frames now use the block-sync cap (defaults raised to 16 MiB) with the RBC chunk default at 256 KiB, and BlockSyncUpdate trimming preserves NPoS stake snapshots to avoid QC stalls; NPoS on-chain parameters now drive mode-aware timing (block/commit timeouts), collector fan-out, election/reconfig policy, and activation lag, with the genesis consensus fingerprint capturing the full NPoS election payload plus epoch seed and docs/unit coverage updated; Governance ZK ballot hint formats are standardized to `root_hint`/`nullifier` across public inputs and BallotProof hex strings (optional `0x`/`blake2b32:` prefixes) with deprecated alias keys rejected, SDK/CLI/bridge updates, and refreshed docs; Torii now rejects deprecated ZK V1 alias keys directly from JSON payloads and enforces canonical owner hints for ZK public inputs with regression coverage; canonical i105 owner hints are now required across CLI, JS SDK (torii client + builders), Rust/JS bridges, Swift SDK, Python SDKs, and mock client paths with new tests; governance portal + JS SDK ZK examples updated to canonical owners; RBC INIT/READY/DELIVER now bind roster hashes and INIT rebroadcasts go out even without cached chunks; RBC persistence now derives missing roster snapshots and READY rebroadcasts enforce roster-hash matching; RBC READY/DELIVER handling now accepts derived rosters when roster hashes match (refreshing derived snapshots on mismatches when available; otherwise stashing until INIT) and local emission uses computed chunk roots when INIT is missing, READY/DELIVER now seed missing expected chunk roots and persisted sessions adopt computed roots even when INIT metadata is missing, manifests without commit hashes can still reload sessions, the RBC store retains delivered chunk bytes for sampling with zero-sized chunk configs rejected, payload rebroadcasts continue while chunks are missing even after READY quorum, and pending RBC session caps no longer evict active stashes; exec-witness fallback added for missing pre-vote roots; DA gate now treats locally available payload matches as availability evidence (missing_local_data clears without waiting on RBC delivery); asset integration tests re-run (all six) passed; `cargo test -p iroha_core` still timing out after 3h; integration-test revalidation still pending (see `status.md` for details); NFT register/unregister now reject missing domains without panicking and include regression coverage; offline balance proof builders now reject invalid blinding/scalar lengths without panicking; tx admission compares signature/instruction/bytecode limits using u64 counts to avoid usize overflow panics; block sync now avoids decode panics on invalid payloads and validates block sequences at handling time, plus NPoS share batches drop roster hints without stake snapshots to avoid invalid roster evidence; NPoS QC validation now enforces stake-quorum snapshots for commit and block-sync QCs with unit coverage; DA/RBC defaults now keep `sumeragi.da.enabled=true` to match the v3 requirement; commit QC persistence now records committed/known-block certificates into world/journal/sidecar with regression coverage; block sync now processes commit votes for known blocks without roster hints, with regression coverage; Kagami localnet now supports `--sora-profile` for Nexus/dataspace presets with enforced peer minimums and Sora startup flags; permissioned block sync now requires roster evidence, rejects commit QCs whose view mismatches the block header, enforces validated commit-quorum gating, and block-sync share filtering validates commit QCs before letting them bypass signature checks; permissioned view rotation uses u64 indices to stay deterministic across architectures, and validator checkpoints embed the view index plus parent/post state roots so receivers can validate them without commit-QC history (invalid QC hints are ignored) and satisfy block-sync quorum gating even when block signatures are trimmed; P2P tx gossip now budgets for encrypted frame overhead to avoid frame-too-large disconnects (Izanami validation pending); block sync ShareBlocks batches now trim to P2P frame caps to avoid oversize disconnects. Torii `/v1/node/capabilities` now includes `data_model_version`; Swift `ToriiClient` enforces it for submissions; JS + Python SDKs enforce it for submissions; Rust `Client` enforces it for submissions. Nexus fee sponsorship now requires `CanUseFeeSponsor` grants and contract calls require `gas_limit` metadata; JS SDK + CLI contract simulation now enforce the required gas limit with updated docs/tests. Sumeragi replay/QC tests now use valid non-genesis Ed25519 signers and mode-aware timing with targeted unit passes. Sumeragi block-sync and BlockCreated unit tests now use valid signed blocks and pre-seeded pending payloads; RBC payload bundles now emit INIT when expected chunk digests are available without cached bytes; BLS blstrs verify now rejects non-canonical/identity encodings with parse-error tests; Sumeragi/localnet idle worker-loop ticks now throttle to a block/commit-derived minimum gap to reduce idle CPU churn; the worker loop now refreshes drain budgets and tick gaps each iteration to follow on-chain timing updates; the worker loop now wakes on queue enqueues, suppresses ticks when fully idle, and blocks until new work or the next tick deadline to avoid fixed polling; tick deadlines now honor missing-block retry/view-change windows and commit-inflight timeouts to avoid idle polling between scheduled work; tick deadlines now cover pending-block timeouts/retention, RBC rebroadcast/TTL, and idle view-change windows, with commit/RBC persist workers waking the loop, RBC stale scheduling covered by unit tests, and new coverage for commit/RBC persist wake signals; `cargo test --workspace` timed out after 60m while running `integration_tests/tests/mod.rs` (tests completed before the timeout passed); test-network log drains now exit on shutdown notify to avoid peer-task timeout warnings with unit coverage (`cargo test -p iroha_test_network log_drain_exits_on_shutdown_notify -- --nocapture` passed); Android Norito fixture manifest tests now decode signature fields and re-encode signed payloads to guard against serialization drift, with fixture authorities normalized to canonical payloads and signed fixtures updated to the 4-field layout with refreshed hashes. Pacemaker bootstrap now bypasses online-peer deferral before the first successful proposal (unit coverage added) to avoid localnet stalls while peer connectivity is still initializing. BlockCreated/FetchPendingBlock/RBC INIT now route through the block queue with block draining ahead of block-payload work to avoid missing-block stalls, while BlockSyncUpdate stays on the block-payload queue. Empty-child fallback configuration is removed so pacemaker stays idle without queued work. Proposal assembly now defers when all queued transactions exceed the payload budget, avoiding heartbeat-only blocks. Kagami localnet NPoS bootstrap stake now raises to the configured min_self_bond to keep validator registration valid. Stale-view RBC handling now accepts messages when the payload is only present via an aborted pending block so READY quorum can still form after view changes. NEW_VIEW missing-block recovery now falls back to commit-QC history rosters when the commit topology is empty, and proposal assembly defers when the highest QC block is missing with unit coverage added. Android SDK instruction payloads are now wire-only; legacy InstructionBuilders removed and InstructionBox.fromNorito now requires wire payloads. Offline allowances now re-derive escrow bindings from `offline.enabled` metadata when the escrow map is empty so escrow-required registrations succeed after metadata updates.
Latest sync (integration tests): seven-peer consistency now waits for submitter mint visibility and requires RBC delivery from commit quorum.
Latest sync (proposal perf): cached confidential feature digests + queued tx size/gas to reduce proposal assembly overhead, and proposal gating now reuses the tip snapshot for stale-pending checks.
Latest sync (NFT metadata): detached parallel apply now performs delta-aware permission checks for NFT metadata edits so they stay in the detached path without bypassing authz; unit coverage added and the integration lifecycle scenario revalidated.
Latest sync (detached permissions): detached permission deltas now apply last-write-wins semantics for account/role permission changes; account/role grant→revoke ordering validated in integration tests (detached metrics asserted only when Nexus is enabled because permissioned networks suppress lane metrics).
Latest sync (roster/activation): pacemaker proposals and live votes now use the active validator roster only (no commit-QC fallback), online-peer gating counts only validators, validator activation now defers to the next epoch boundary, and genesis bootstrap falls back to the genesis roster if commit/world rosters are empty.
Latest sync (worker loop): tick scheduling now uses a derived busy gap (clamped below the idle gap) when queues are backlogged so consensus progress is not throttled by pending drains; unit coverage added. The worker loop now supports a parallel ingress mode (per-queue threads + ticketed gate) controlled by `sumeragi.advanced.worker.parallel_ingress` (default true), with docs/config updates and new unit coverage.
Latest sync (fixtures): SDK transaction payload fixtures now store wire-only instruction entries (no legacy kind/arguments), exporter strips legacy instruction fields on regen, legacy instruction-schema/double-payment JSON fixtures and Norito blobs were removed across Android/Swift/Python (and the root Norito manifest), and fixture regen scripts no longer sync legacy schema files.
Latest sync (consensus timing): on-chain `pacing_factor_bps` now scales the effective block/commit targets and is surfaced via status/params outputs alongside the derived timeouts.
Latest sync (NPoS phase scaling): NPoS per-phase timeouts are normalized to the default pipeline budget so the sum of propose/DA/prevote/precommit/commit matches the target block time (without changing overrides).
Latest sync (consensus liveness): quorum reschedules and idle view changes now proceed after the availability timeout even if RBC backlog persists, avoiding DA deadlocks under partial payload loss.
Latest sync (genesis): genesis transaction validation now uses the block timestamp during static checks to avoid clock-skew rejections.
Latest sync (config): NPoS pacemaker timeouts now derive from on-chain `SumeragiParameters.block_time_ms` with optional `sumeragi.advanced.npos.timeouts` overrides, `sumeragi_npos_parameters` no longer carries timeout fields, and the `sumeragi.npos.block_time_ms` fallback is removed; docs/templates refreshed and minimal config fixtures now include PoP coverage, with the snapshot refreshed to drop the removed NPoS block-time/timeouts fields and match current crypto defaults.
Latest sync (CLI tests): updated `integration_tests/tests/iroha_cli.rs` to use `iroha ops executor` and `iroha ledger domain` command groups.
Latest sync (tests): `cargo test -p iroha_core --tests --features "zk-tests halo2-dev-tests" zk` timed out after 1800s while running `gov_auto_close_zk_requires_tally`; ZK unit tests passed, including `kaigi_usage_backend_accepts_valid_proof`.
Latest sync (tests): Sumeragi main-loop harness now times out P2P startup and falls back to a closed handle to avoid hanging `cargo test -p iroha_core`.
Latest sync (tests): `cargo test -p iroha_core rbc_backlog_ -- --nocapture` and `cargo test -p iroha_core bypasses_background_queue -- --nocapture` (ok).
Latest sync (tests): `cargo test -p iroha_core commit_pipeline_defers_reschedule_until_availability_timeout -- --nocapture` and `cargo test -p iroha_core rbc_chunk_commit_pipeline_runs_on_completion -- --nocapture` (ok).
Latest sync (tests): `cargo test -p iroha_core commit_pipeline_ -- --nocapture` (ok).
Latest sync (telemetry): Sumeragi phase EMA gauges are seeded on startup and `collect_da` is recorded once `BlockCreated` payloads arrive, with unit coverage added.
Latest sync (state commit): Sumeragi state commit drops the tiered-backend lock before taking `view_lock` to avoid lock-order inversion with lane lifecycle updates; unit coverage added.
Latest sync (tests): `cargo test -p iroha_core force_view_change_if_idle_skips_when_consensus_queue_backpressure -- --nocapture` (ok).
Latest sync (consensus relay): RBC chunk traffic now uses a dedicated `ConsensusChunk` queue promoted to high priority to avoid low-priority stalls while keeping chunk flow isolated from other control-plane traffic.
Latest sync (consensus ingress/P2P): critical caps now apply to votes/VRF/proposals/fetch-pending + BlockCreated + RBC INIT/READY/DELIVER/CHUNK + control-flow evidence (penalty cooldowns skipped, RBC-session tracked), bulk payloads (BlockSyncUpdate/ExecWitness/BlockSync) use a separate bulk bucket seeded from the standard caps (penalty cooldowns skipped), critical caps fall back to standard caps when unset with penalty cooldowns enabled, and defaults are 300 msg/s.
Latest sync (consensus telemetry): consensus_message_handling now tracks proposals, votes, QCs, VRF, RBC init/ready/chunk, fetch-pending requests, and evidence drops/deferrals with unit coverage.
Latest sync (storage budgets): Kura now persists oversized blocks directly into `da_blocks/` (indexed as evicted) while budget accounting treats oversized pending blocks at the evicted footprint; unit tests and docs updated.
Latest sync (roster/PoP): PoP filtering now skips incomplete PoP maps and falls back to the BLS baseline roster to avoid divergent validator topologies.
Latest sync (NPoS roster): canonicalize active-validator roster ordering for NPoS to keep signer indices stable across peers even when commit topology order diverges; roster unit coverage updated.
NPoS block sync now accepts explicitly requested missing-block updates using uncertified rosters with stake snapshots; unit coverage added.
Latest sync (queue): queue telemetry now splits active/queued/inflight, queued length is derived from the hash queue minus stale markers, and Torii high-load gating aligns to active queue depth.
Latest sync (tests): checkpoint view regression tests passed in `iroha_data_model` and `iroha_core`; permissioned QC view mismatch regression passed in `iroha_core`; activation-height NPoS vote tests now pass (`handle_vote_uses_activation_height_mode_tag`, `emit_precommit_vote_uses_activation_height_mode_tag`); `cargo test --workspace` failed due to merge conflict markers in `crates/iroha_core/src/sumeragi/main_loop.rs`; `cargo check -p iroha_executor` (ok; invoice_id accessor avoids private-field access); `cargo test -p iroha_core sanitize_block_sync_qc_drops_view_mismatch -- --nocapture` timed out after 120s during compilation; `cargo test -p iroha_core --lib handle_qc_ -- --nocapture` (pass; network bind unavailable warnings); `cargo test -p iroha_core --lib block_created_updates_locked_status_when_lock_missing -- --nocapture` (pass); `cargo test -p iroha_core --lib pacemaker_updates_highest_qc_status_from_new_view -- --nocapture` (pass); `cargo test -p iroha_core --lib qc_extends_locked_if_present_allows_missing_lock_locally -- --nocapture` (pass); `cargo test -p iroha_core --lib locked_qc_subject_updates_for_same_height_view -- --nocapture` (pass); `cargo test -p iroha_core --lib pacemaker_bootstraps_with_commit_qc_when_active_roster_empty -- --nocapture` (pass); `cargo test -p iroha_core --lib prune_descendants_keeps_view_seen_after_divergent_proposal_drop -- --nocapture` (pass); `cargo test -p iroha_core --lib commit_inflight_timeout_triggers_view_change_and_retains_aborted_pending -- --nocapture` (pass); `cargo test --workspace` timed out after 1200s while running tests (warnings: unused `MissingQc` variant in `mochi/mochi-ui-egui/src/main.rs`, unused `base64::Engine` import in `crates/iroha_cli/src/contracts.rs`, unused assignment in `integration_tests/tests/asset.rs`); `cargo test -p iroha_core --lib idle_wait_duration_tracks_min_gap -- --nocapture` (pass); `cargo test -p iroha_core --lib try_incoming_block_message_wakes_worker_on_accept -- --nocapture` (pass); `cargo test -p iroha_core --lib run_worker_iteration_skips_tick_when_actor_disables_tick -- --nocapture` (pass); `cargo test -p iroha_core --lib actor_should_tick_tracks_missing_block_requests -- --nocapture` (pass); `cargo test -p iroha_core --lib push_wakes_sumeragi_when_configured -- --nocapture` (pass); `cargo test -p iroha_core --lib run_worker_iteration_respects_tick_deadline -- --nocapture` (pass); `cargo test -p iroha_core --lib actor_next_tick_deadline_tracks_missing_block_windows -- --nocapture` (pass); `cargo test --workspace` timed out after 1200s while running tests (warning: unused trigger metadata constants in `crates/iroha_core/src/smartcontracts/isi/triggers/mod.rs`); `cargo test -p iroha_core --lib commit_worker_wakes_on_result -- --nocapture` (pass); `cargo test -p iroha_core --lib rbc_persist_worker_wakes_on_result -- --nocapture` (pass); `cargo test --workspace` aborted by user after ~15m (no failures observed before abort); `IROHA_TEST_NETWORK_PARALLELISM=5 IROHA_SOAK_TARGET_BLOCKS=100 cargo test -p integration_tests --test sumeragi_localnet_smoke permissioned_localnet_soak_thousands -- --ignored --nocapture` skipped network startup due to loopback bind denial, so the soak did not run; `IROHA_TEST_NETWORK_PARALLELISM=5 IROHA_SOAK_TARGET_BLOCKS=100 cargo test -p integration_tests --test sumeragi_localnet_smoke permissioned_localnet_soak_thousands -- --ignored --nocapture` failed: localnet soak stalled for 40s at min_non_empty=59/target 101 with status timeout for `aspirant_perch`.

The repository now serves two release lines:
- **Iroha 2** — the self-hosted deployment track for organisations running
  independent permissioned or consortium networks.
- **Iroha 3 (SORA Nexus)** — the single global network backed by multi-lane
  Nexus infrastructure where operators join by registering data spaces.

Both release lines share the same Iroha Virtual Machine and Kotodama toolchain, so smart contracts and bytecode
artifacts are portable between self-hosted deployments and the SORA Nexus ledger.

Unless stated otherwise, roadmap items call out which release line they affect.

> **Status Legend:** 🈴 Completed · 🈺 In Progress · 🈯 Planned · 🈸 Drafting · 🈳 Not Started · 🈲 At Risk

## Current Open Work

- Norito streaming math upgrades: motion-compensated prediction, rANS rate-model RDO, adaptive quantization + deterministic deblock, deterministic audio LPC/MDCT path, and SIMD/GPU acceleration for fixed-point DCT.
- GPU zstd (Metal): full GPU entropy coding + decode (bit-for-bit parity with CPU zstd).
 - [x] Architecture: document the full GPU zstd pipeline (frame/block layout, buffer lifetimes, deterministic bitstream packing, and kernel responsibilities) with explicit parity constraints.
 - [x] Bitstream core: implement Metal bitstream writer/reader utilities (little-endian bit packing, byte-aligned flush, overflow checks) with CPU golden vectors for validation.
 - [x] Huffman literals: implement Metal Huffman encoder (code length generation + canonical table build + literal encoding) and decoder with parity tests.
 - [x] FSE sequences: implement Metal FSE encoder/decoder for LL/ML/OF (table build + state machine) with parity tests.
 - [x] Block/frame encoder: emit standard zstd frames (headers, raw/RLE/compressed blocks, optional checksum) using GPU entropy output.
- [x] Full helper-side decode: parse frames/blocks on the host and decode literals + sequences into output buffers (CPU zstd fallback for unsupported frames; GPU block decode still pending).
 - [x] Verification: add corpus parity + fuzz tests across CPU/GPU, including determinism checks and error handling edge cases.
 - [x] Performance + integration: benchmark vs CPU zstd, set thresholds for GPU offload, and update gpuzstd_metal runtime gating/docs.

- Localnet 1 Hz: re-run the NPoS 1 Hz soak with the fast-finality gas cap enabled (`sumeragi.block.fast_gas_limit_per_block`) and confirm block spacing falls under 1s on localhost. 2026-01-31 run (4 peers, NPoS, block/commit 1000ms, fast_gas_limit_per_block=1_000_000) used `/private/tmp/iroha-localnet-npos-1hz-fastgas-20260131b` with status sampling in `sumeragi_status_1hz_fastgas_20260131d.jsonl`: commit_qc height 183->250 (+67) over 181.8s (~0.368 blocks/s, ~2.7s/block), view_change_install_total +0, pending_rbc max 0 bytes, missing_block_fetch +1; effective block/commit time 2000/1500 with pacing_factor_bps=20000 (still above 1s target). 2026-01-31 rerun after localnet pacing clamp (block/commit 1000ms, fast_gas_limit_per_block=1_000_000) used `/private/tmp/iroha-localnet-npos-1hz-fastgas-clamped-20260131T185723Z` with status sampling in `sumeragi_status_1hz_fastgas_clamped_20260131T185723Z.jsonl`: commit_qc height 1->48 (+47) over 179s (~0.263 blocks/s, ~3.81s/block), view_change_install_total +2, pending_rbc max 5563 bytes, missing_block_fetch +17; effective block/commit time 1000/750 with pacing_factor_bps=10000 (still above 1s target).

1. **NEXUS-LANE-RELAY-RECOVERY — Emergency validator restore for dataspaces** (Consensus/Governance, Line: Iroha 3, Owner: Nexus Core WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Define the emergency admin multisig message to add validators when a dataspace pool falls below `3f+1`.
 - [x] Implement validation/admission plumbing, telemetry, and regression tests; update operator docs/runbooks.
 - [x] Enforce lane relay QC committee membership + aggregate signature validation and expose dataspace `fault_tolerance` in telemetry/status.

2. **OFFLINE-SECURITY-HARDENING — Close remaining offline receipt threat gaps** (Core/SDK, Line: Shared, Owner: Offline WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Bind receipt challenges to `chain_id` across core, SDKs, and FFI helpers.
 - [x] Enforce `policy.max_tx_value` during settlement and surface `max_tx_value_exceeded` rejections.
 - [x] Require allowance-scale consistency across receipts, policy limits, and balance proofs in core + SDK validators.
 - [x] Add range-proof verification for `resulting_commitment` (non-negative/bounded balance) and update docs/tests.
 - [x] Document operational guardrails for front-running/rollback risk and rooted-device privacy exposure.
 - [x] Add receipt timestamps + `max_receipt_age_ms` guardrails to reduce front-running and replay risk; update SDK UX and docs.
 - [x] Mitigate memory exposure for commitment witness material (zeroization in the Norito bridge).

3. **INTEGRATION-TEST-REVALIDATION — Re-run integration tests after Sumeragi drain-order fix** (QA/Consensus, Line: Shared, Owner: QA WG, Priority: High, Status: 🈺 In Progress, target TBD)
- [x] Re-run the previously failing suites (connected_peers, unstable_network, sumeragi_da, time_trigger, npos_pacemaker) with the now more conservative default test-network parallelism (peers-per-network divisor 64) to confirm the resource-contention fix holds (connected_peers ok; unstable_network 5/8/9-2/9-3 ok; time_trigger ok; sumeragi_da payload-loss/eviction/kura eviction + commit-certificate reruns ok; npos_pacemaker ok).
- [x] Re-run remaining `tmp_failures.txt` cases (proof events/queries, extra_functional suites, sumeragi_da) after seeding the genesis commit roster on commit to confirm consensus recovery. Re-ran `proofs` + `events::proof::proof_event_scenarios` + `queries::proof::proof_query_scenarios`, full `extra_functional::` suite with `--test-threads=1` (32/33 pass; restart_peer fixed and re-ran), and `sumeragi_da_*` with `--test-threads=1`.
- [x] Re-run the `sumeragi_da_*` failures after aligning proposal block signatures with the local validator index (RBC INIT leader signature should now match non-zero leaders).
- [x] Add 4-fault coverage to the unstable-network integration suite (raised 12-peer case to `n_rounds=3` to match the restored baselines).
 - [x] Re-run `cargo test -p integration_tests sumeragi_rbc_da_large_payload_four_peers -- --nocapture` to confirm DA large-payload RBC flow completes after READY/DELIVER queue routing.
 - [x] Re-run `cargo test -p integration_tests sumeragi_rbc_da_large_payload_six_peers -- --nocapture` after wiring `sumeragi.debug.rbc.force_deliver_quorum_one` to confirm the 6-peer large-payload scenario stays within delivery budgets.

4. **IZANAMI-NPOS-VALIDATION-LATENCY — Reduce pre-vote validation latency to meet 1 TPS targets** (Consensus/Perf, Line: Iroha 3, Owner: Core WG, Priority: Medium, Status: 🈺 In Progress, target TBD)
- [x] Instrument `validate_block_for_voting` with stage timings (stateless checks vs execution) and surface per-block validation latency in Izanami logs (debug log + testing guide note).
- [ ] Optimize validation hot path (signature batching, trigger execution cost, or cached stateless results) so `pending_age_ms` stays below `fast_timeout_ms` at 1 TPS.
- [ ] Re-run Izanami 1 TPS (300s/200 blocks) and confirm `commit pipeline defers validation` no longer appears and block height reaches target (2026-01-31 run with 300s/120 blocks reached min height 121 in 261.7s; no defers-validation logs; 200-block target still pending). 2026-01-31 rerun (existing binaries: `target/debug/izanami` + `target/release/iroha3d`) stopped before target at committed height 137; network dir `/tmp/iroha-test-network-izanami-1tps-20260131-run1/irohad_test_network_al0y15`. 2026-01-31 rerun (`/tmp/iroha-codex-izanami-1tps-run2/release/izanami` + `target/release/iroha3d`) stopped before target at committed height 117 (nationwide_zorilla; others at 116); network dir `/tmp/iroha-test-network-izanami-1tps-20260131-run2/irohad_test_network_vQxKSK`. 2026-01-31 rerun (`CARGO_TARGET_DIR=/Users/mtakemiya/dev/iroha/target-izanami-1tps-200`) stopped before target with min_height 99 at 300s; no `commit pipeline defers validation` in the Izanami log; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_wTeoI6`, log `/tmp/izanami_1tps_200_20260131_run2.log`. 2026-02-01 rerun (`/tmp/iroha-codex-izanami-1tps-run2/release/izanami` + `target/release/iroha3d`) stopped before target at committed height 62 (calm_accentor/diplomatic_smew/advisable_planarian; affluent_reindeer at 61); no `commit pipeline defers validation` in the Izanami log; network dir `/tmp/iroha-test-network-izanami-1tps-20260201-run1/irohad_test_network_X9ruRf`, log `/tmp/izanami_1tps_20260201_run1.log`. 2026-02-01: enabled `sumeragi.advanced.pacemaker.da_fast_reschedule` in Izanami to shorten DA reschedule stalls; 2026-02-01 rerun (release build, 4 peers, 300s, target 200) stopped before target at min_height 136; no `RegisterBox::Trigger invoke` errors or plan submission failures observed; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_O6hWla`, log `/tmp/izanami_1tps_200_20260201T092010Z.log`. 2026-02-01 rerun with `kura.fsync_mode=off` in Izanami config layers (release build, 4 peers, 300s, target 200) stopped before target at committed height 164-165 (min 164); network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Pa4Vny` (peer logs `run-1-stdout.log`), no mint-trigger missing errors observed in peer logs.
 - [x] Re-run `cargo test -p integration_tests --test address_canonicalisation -- --nocapture` to confirm the suite completes without timeouts (latest attempt with `IROHA_TEST_NETWORK_PARALLELISM=5` timed out after ~12m with tests still running and permit waits; sandbox run with `IROHA_TEST_NETWORK_PARALLELISM=4` skipped network startup due to loopback bind denial; rerun timed out after ~16m with network guard stuck at 4/4 permits; serialized rerun failed to compile `iroha_genesis` due to missing Norito JSON traits for `RegisterPublicLaneValidator`/`ActivatePublicLaneValidator`; fixes landed, but the subsequent rerun terminated by SIGKILL).
 - [x] Re-run `cargo test -p integration_tests --test asset client_add_asset_quantities_should_increase_asset_amounts -- --nocapture` to confirm DA delivery gating no longer stalls decimal mint flows.
 - [x] Re-run `cargo test -p integration_tests --test asset client_add_asset_quantities_should_increase_asset_amounts -- --nocapture` and `cargo test -p integration_tests --test asset find_rate_and_make_exchange_isi_should_succeed -- --nocapture` after the sync retry budget + asset test sync updates to confirm asset flows no longer time out on tx confirmation.
 - [x] Re-run asset integration tests after the exec-witness fallback for missing pre-vote roots to confirm commit votes and confirmations no longer stall.
 - [x] Re-run `cargo test -p integration_tests --test asset -- --nocapture` after the asset client-rotation fallback to confirm Torii timeouts no longer stall exchange/spec flows (sandbox run with `IROHA_TEST_NETWORK_PARALLELISM=4` skipped network startup due to loopback bind denial).
 - [x] Re-run `cargo test -p integration_tests --test asset -- --nocapture` after broadcasting asset submissions to all peers and widening asset-test consensus timing (6s pipeline, DA timeout multipliers) to confirm integer transfer no longer stalls (pass: `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo test -p integration_tests --test asset -- --nocapture --test-threads=1`).
 - [x] Re-run `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_recovers_after_peer_restart -- --nocapture --test-threads=1` after replay-roster fixes to confirm restart recovery completes.
 - [x] Re-run `cargo test -p integration_tests --test sumeragi_da sumeragi_idle_view_change_recovers_after_leader_shutdown -- --nocapture` to validate idle view-change recovery after leader shutdown (sandbox run with `IROHA_TEST_NETWORK_PARALLELISM=4` skipped network startup due to loopback bind denial; rerun with `CARGO_TARGET_DIR=/tmp/iroha-codex-target-tests` + `IROHA_TEST_NETWORK_PARALLELISM=1` + `IROHA_TEST_SKIP_BUILD=1` + `TEST_NETWORK_BIN_IROHAD=target/debug/iroha3d` passed with `trusted_peers_pop` warnings and signal 6 on shutdown; updated the test to submit to all running peers and extend the recovery window to 90s, then reran with `CARGO_TARGET_DIR=/tmp/iroha-codex-roadmap-target` + `IROHA_TEST_NETWORK_PARALLELISM=1` + `IROHA_TEST_NETWORK_KEEP_DIRS=1` + `IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d)` (pass; `trusted_peers_pop` warnings during profile scan)).
 - [x] Re-run `IROHA_TEST_NETWORK_KEEP_DIRS=1 cargo test -p integration_tests --test sumeragi_da sumeragi_da_eviction_rehydrates_block_bodies -- --nocapture` after NPoS roster canonicalization to confirm DA eviction rehydration (pass).
 - [x] Re-run `cargo test -p integration_tests --test sumeragi_localnet_smoke permissioned_localnet_soak_thousands -- --ignored --nocapture` to confirm the thousands-block soak stays stable (2026-01-31 run with `IROHA_SOAK_TARGET_BLOCKS=200` + serialized network finished with min_non_empty 201; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_3cjuPe`).
 - [x] Logger: avoid `TestWriter` panics on broken pipes by using a lossy stdout writer in `iroha_logger` (unit coverage added) so spawned peers don't abort during shutdown.
 - [x] Limit integration-test network concurrency in the sandbox harness (CPU-scaled default + `IROHA_TEST_NETWORK_PARALLELISM`) so `--test-threads=1` is no longer required.
 - [x] Add a restart regression that simulates a crash after persisting a finalized VRF epoch record but before writing the seed-only next-epoch snapshot.

4. **NPOS-LOCALNET-1HZ — Restore 1 Hz localnet block cadence** (Consensus/Localnet, Line: Iroha 3, Owner: Consensus WG, Priority: Medium, Status: 🈺 In Progress, target TBD)
 - [x] Identify pacemaker backpressure sources holding proposal cadence (~7.5s/block) under 1 Hz ping load; capture per-stage timings (added per-reason deferral counters/ages/durations plus pacemaker eval/propose timing histograms; docs updated).
 - [x] Tune NPoS timeouts and commit pipeline thresholds for localnet to reach 1 block/sec without triggering view changes (propose/prevote/precommit/commit/DA/agg = 300/400/500/650/600/100 ms; DA timeout multipliers 1/1; commit inflight 4–10s with 6x multiplier).
 - [x] Re-run the 1 Hz / 100-block soak and confirm view changes remain zero and RBC queues stay under cap (rerun 2026-01-21 on `/tmp/iroha-localnet-npos-1hz-run2`; 100x1 Hz `transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `status-1hz-20260121T192416Z.jsonl`: `commit_qc.height` +12 over 118s (~0.10 blocks/s), `view_change_install_total` +3 (`stake_quorum_timeout_total` +2), pending RBC max 1040 bytes / 1 session (cap 8 MiB / 256); 2026-01-23 debug rerun on `/tmp/iroha-localnet-npos-1hz-20260123T132039Z`: `commit_qc.height` 1->14 over 363 samples, `view_change_install_total` +3 (stake_quorum_timeout/missing_qc), `pending_rbc.bytes` max 1260 / 2 sessions; 2026-01-23 release rerun on `/tmp/iroha-localnet-npos-1hz-20260123T133537Z`: `commit_qc.height` 1->17 over 136 samples, `view_change_install_total` +71 (missing_qc), `pending_rbc.bytes` max 2220 / 1 session; 2026-01-23 tuned rerun on `/tmp/iroha-localnet-npos-1hz-20260123T140944Z` with `redundant_send_r=3` + `availability_timeout_multiplier=2`: `commit_qc.height` 0->7 over 146 samples, `view_change_install_total` +3 (missing_qc), `pending_rbc.bytes` max 0; 2026-01-23 rerun with RBC INIT routed via block-payload queue on `/tmp/iroha-localnet-npos-1hz-20260123T181930Z`: 120x1 Hz `transaction ping --no-wait` with `ops sumeragi status` sampling in `sumeragi_status_1hz_fix_status_20260123T184602Z.jsonl` (ping log `ping_1hz_fix_nowait_20260123T184602Z.log`), `commit_qc.height` 83->95 (+12 over 120 samples), `view_change_install_total` +4 (`missing_qc_total` +1, `stake_quorum_timeout_total` +3), `missing_payload_total` +0, `pending_rbc.bytes` 0, `stash_ready_init_missing_total` +8; 2026-01-23 tuned rerun on `/private/tmp/iroha-localnet-npos-1hz-20260123T190128Z` with `redundant_send_r=3` + `availability_timeout_multiplier=2` + `quorum_timeout_multiplier=2`: 120x1 Hz `transaction ping --no-wait` with `ops sumeragi status` sampling in `sumeragi_status_1hz_tuned_20260123T190128Z.jsonl` (ping log `ping_1hz_tuned_20260123T190128Z.log`), `commit_qc.height` 1->14 (+13 over 120 samples), `view_change_install_total` +1 (`stake_quorum_timeout_total` +1), `missing_qc_total` +0, `missing_payload_total` +0, `missing_block_fetch.total` +6, `pending_rbc.bytes` max 220 (sessions max 1); 2026-01-23 tuned rerun on `/private/tmp/iroha-localnet-npos-1hz-20260123T195213Z` with `redundant_send_r=4`, `availability_timeout_multiplier=3`, `quorum_timeout_multiplier=3`, `rbc.payload_chunks_per_tick=256`, and `worker.tick_work_budget_cap_ms=1000`: 120x1 Hz `transaction ping --no-wait` with `ops sumeragi status` sampling in `sumeragi_status_1hz_tuned2_20260123T195213Z.jsonl` (ping log `ping_1hz_tuned2_20260123T195213Z.log`), `commit_qc.height` 1->28 (+27 over 120 samples), `view_change_install_total` +30 (`missing_qc_total` +22), `stake_quorum_timeout_total` +0, `missing_payload_total` +0, `missing_block_fetch.total` +27, `pending_rbc.bytes` max 4189 (sessions max 1); cadence improved but still below 1 Hz; 2026-01-24 rerun on `/private/tmp/iroha-localnet-npos-1hz-20260124T065327Z` with NPoS timeouts propose/prevote/precommit/commit/da/agg=300/500/700/900/800/100, collectors_k=4, redundant_send_r=4, and `sumeragi.advanced.da.availability_timeout_multiplier=2`/`quorum_timeout_multiplier=2`: 100x1 Hz `transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `sumeragi_status_1hz_timeouts_collectors_20260124T065327Z.jsonl` (ping log `ping_1hz_timeouts_collectors_20260124T065327Z.log`), `commit_qc.height` 1->42 (+41 over 120 samples), `view_change_install_total` +7 (`missing_qc_total` +6), `stake_quorum_timeout_total` +0, `missing_payload_total` +0, `missing_block_fetch.total` +14, `pending_rbc.bytes` max 220 (sessions max 1); cadence still below 1 Hz; 2026-01-24 rerun on `/private/tmp/iroha-localnet-npos-1hz-20260124T113118Z` with `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, `sumeragi.advanced.da.availability_timeout_multiplier=3`, `sumeragi.advanced.da.quorum_timeout_multiplier=3`: 100x1 Hz `transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `sumeragi_status_1hz_20260124T113118Z_retry2.jsonl` (ping log `ping_1hz_20260124T113118Z_retry2.log`), `commit_qc.height` 53->53 (+0 over 120 samples), `view_change_install_total` +0 (start had `missing_payload_total=13`, `missing_qc_total=3`, `stake_quorum_timeout_total=1`), `missing_block_fetch.total` +1433, `pending_rbc.bytes` max 0 (sessions max 0); stalled on missing payload at height 54; 2026-01-24 rerun on `/private/tmp/iroha-localnet-npos-1hz-20260124T121035Z` with `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, `sumeragi.advanced.da.availability_timeout_multiplier=3`, `sumeragi.advanced.da.quorum_timeout_multiplier=3`, `sumeragi.advanced.rbc.pending_ttl_ms=120000`, `sumeragi.advanced.rbc.session_ttl_ms=240000`, `sumeragi.recovery.missing_block_signer_fallback_attempts=0`: 100x1 Hz `transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `sumeragi_status_1hz_20260124T121035Z.jsonl` (ping log `ping_1hz_20260124T121035Z.log`), `commit_qc.height` 1->64 (+63 over 120 samples), `view_change_install_total` +0, `missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +15, `pending_rbc.bytes` max 2708 (sessions max 1); cadence improved but still below 1 Hz; 2026-01-24 rerun on `/private/tmp/iroha-localnet-npos-1hz-20260124T130140Z` with `block_time_ms=1000`, `commit_time_ms=1500`, NPoS timeouts propose/prevote/precommit/commit/da/agg=500/800/1100/1500/1400/200, `sumeragi.advanced.da.availability_timeout_multiplier=3`, `sumeragi.advanced.da.quorum_timeout_multiplier=3`, `sumeragi.advanced.rbc.pending_ttl_ms=120000`, `sumeragi.advanced.rbc.session_ttl_ms=240000`, `sumeragi.recovery.missing_block_signer_fallback_attempts=0`: 100x1 Hz `transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `sumeragi_status_1hz_20260124T130140Z.jsonl` (ping log `ping_1hz_20260124T130140Z.log`), `commit_qc.height` 1->60 (+59 over 120 samples), `view_change_install_total` +3 (`missing_qc_total` +1), `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +7, `pending_rbc.bytes` max 0 (sessions max 0); 2026-01-24 rerun after validation inflight fast-timeout fix on `/private/tmp/iroha-localnet-npos-1hz-20260124T130140Z`: 100x1 Hz `transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `sumeragi_status_1hz_fix_20260124T200000Z_20260124T194233Z.jsonl` (ping log `ping_1hz_fix_20260124T200000Z_20260124T194233Z.log`), `commit_qc.height` 1->58 (+57 over 120 samples), `view_change_install_total` +1 (no view_change_causes deltas), `missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +11, `pending_rbc.bytes` max 0 (sessions max 0); 2026-01-24 rerun after DA fast-path disable on `/tmp/iroha-localnet-npos-1hz-20260124T172906Z`: 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `sumeragi_status_1hz_fastpath_fix_20260124T173233Z.jsonl` (ping log `ping_1hz_fastpath_fix_20260124T173233Z.log`), `commit_qc.height` 50->104 (+54 over 120 samples), `view_change_install_total` +0, `missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +15, `pending_rbc.bytes` max 440 (sessions max 1); cadence still below 1 Hz; 2026-01-24 rerun on `/private/tmp/iroha-localnet-npos-1hz-20260124T190127Z`: 100x1 Hz `ledger transaction ping --no-wait` with `/v1/sumeragi/status` sampling in `sumeragi_status_1hz_missing_init_debug_20260124T190127Z.jsonl` (ping log `ping_1hz_missing_init_debug_20260124T190127Z.log`), `commit_qc.height` 2->58 (+56 over 100 samples), `view_change_install_total` +0, `missing_qc_total` +0, `missing_payload_total` +0, `stake_quorum_timeout_total` +0, `missing_block_fetch.total` +24, `pending_rbc.bytes` max 220 (sessions max 1), `stash_ready_init_missing_total` 18.
 - [ ] Re-run `cargo test -p integration_tests -- --nocapture` after the targeted suite completes cleanly (ran 60m; timed out while running `integration_tests/tests/mod.rs` with network guard stuck at 4/4 permits and unstable-network relay connection-refused warnings).
 - [x] Re-run `cargo test -p integration_tests --test sumeragi_localnet_smoke -- --nocapture` to confirm localnet tx-status fallbacks no longer emit WARN noise.
- [x] Capture a localnet stall run with the new `sumeragi_localnet_smoke` status snapshots to pinpoint any remaining consensus races (saw repeated `dropping vote: empty commit topology` and RBC READY quorum stalls).
- [x] Add P2P queue depth telemetry and RBC store pressure logging to diagnose localnet soak stalls.
- [x] Reproduce 4-peer DEBUG 1s localnet subscriber-queue drops with bulk ping load after relay worker-queue/backpressure gating, expanded relay backpressure drop counters (post/broadcast drops, per-peer post overflows, topic-cap violations), unresolved-RBC backlog deferrals plus relay backpressure/pending-block proposal gating, stale-view RBC acceptance limited to active pending/inflight blocks and inactive-session rebroadcast/backlog gating suppression, queue hash compaction + queued-len gating, localnet NPoS bootstrap stake/gas defaults, NPoS roster bootstrap from active validators, Kagami genesis sign auto-bootstrap of NPoS stake/topology, consensus-ingress RBC session limit scaling, raised consensus-ingress rate/burst defaults + `QcVote` exemption, NEW_VIEW votes fanning out to collectors/topology fallback (leader included), transaction gossip resend deferral (`network.transaction_gossip_resend_ticks`), per-tick RBC chunk broadcast pacing (`sumeragi.advanced.rbc_payload_chunks_per_tick`), ConsensusChunk topic + low-priority RBC chunk routing, MissingLocalData quorum-reschedule deferral, and NPoS aggregator timeout aligned to base; capture per-topic drop counters and Torii tx-rate-limit behavior to isolate any remaining flood sources (run2 with `--count 3000 --parallel 16` showed no queue-full logs; run20 2h soak with 2 lanes/peer `--count 15000 --parallel 48` saw 0 queue drops/view-changes/429s; rerun after preauth cap clamp + localnet RLIMIT uplift: RLIMIT 16384, `--count 50000 --parallel 128`, Torii accepted ~11.6k tx on peer0, no subscriber drops, blocks stuck at height 1 due to `not enough stake collected for QC`; localnet now mints stake in a prior genesis tx, registers/activates NPoS validators in a follow-up tx, enables Nexus for staged NPoS cutovers, and start scripts pass `--sora` when Nexus is enabled; 4-peer DEBUG load rerun (ports 46180/50337, `--count 20000 --parallel 128`) shows `commit_qc.validator_set_len=4` but still no tx admission/commit, with stake quorum timeouts and missing payload/QC view changes; rerun with peer1 submission target (ports 46380/50537, `--count 20000 --parallel 128`, backgrounded) shows commit heights 3-5 and `commit_qc.validator_set_len=4` on all peers, no subscriber-queue drops, and `vote_validation_drops` limited to `duplicate`/`stale_height` while peer1 still leads in stake-quorum/missing-QC counts; latest debug run (ports 18180/13400, peer1 `--count 5000 --parallel 32 --no-wait`) reached `commit_qc.height=2` with `validator_set_len=4`, `stake_quorum_timeout_total=5`, `missing_payload_total=2`, `missing_qc_total=1`, and `vote_validation_drops` still `stale_height` only, while peer0 logged early `not enough stake collected for QC` with `voting_signers=1`; 2026-01-18 rerun with `kagami localnet --consensus-mode npos --peers 4 --out-dir /private/tmp/iroha-localnet-npos-debug-1s-run25 --base-api-port 48180 --base-p2p-port 53337`, `iroha --config /private/tmp/iroha-localnet-npos-debug-1s-run25/client-peer1.toml transaction ping --count 20000 --parallel 128 --no-wait`: `/v1/sumeragi/status` shows `commit_qc.height=0` on all peers with peer1 `view_change_install_total=72`/`missing_qc_total=59`, worker-loop drops 0, and peer1 log still reports `not enough stake collected for QC`).
- [ ] Re-run a 7-peer localnet load after the BlockSyncUpdate + block-payload blocking enqueue change, background post fallback, queue-pop lock reduction, bounded proposal queue scan (`sumeragi.block.proposal_queue_scan_multiplier`), higher localnet TEU caps (`nexus.fusion.exit_teu`), and epoch-roster widening when commit topology omits active validators to confirm block height advances under block-queue pressure and updates are no longer dropped. Latest run (release, 7 peers, 100k ping txs, `--parallel 128 --with-index`) reached height 32 (blocks_non_empty +22) with ~20 tps admitted and ~31 tps committed; queue depth climbed and missing-payload view-change warnings persisted. 2026-01-18: `scripts/run_localnet_throughput.sh --release --artifact-dir ./artifacts/localnet-throughput --keep-dirs --env IROHA_TEST_NETWORK_PARALLELISM=1 --env IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d)` timed out after 20m during `permissioned_localnet_throughput_10k_tps` (release build completed; run started with 7 peers, block/commit 1s, total_blocks=40); logs in `/var/folders/7l/w31n0ppj4zg874c4szhllss00000gn/T/irohad_test_network_hQttXn`, artifacts dir empty.
 - [x] Re-run integration tests that previously stalled on tx confirmation streams to validate the poll-priority + queued/approved tracking, base64 rejection decoding, and listener-connect gating for early events (pipeline/notification/sse_smoke cases passed).
- [x] Re-run `cargo test -p integration_tests --test mod multiple_blocks_created -- --nocapture` (and unregister/soft-fork cases) after the merge QC view wiring (lane-tip view) and commit-certificate/roster sidecar persistence fix to confirm block-sync roster validation no longer drops updates (multiple_blocks_created, network_stable_after_add_and_after_remove_peer, soft_fork passed).
 - [x] Re-run `cargo test -p integration_tests --test sumeragi_kagami_localnet -- --nocapture` after the BlockSyncUpdate backpressure, missing-parent fetch, RBC chunk background drop, periodic RBC payload rebroadcast, and proposal/RBC broadcast-order fixes; ran outside the sandbox with `IROHA_KAGAMI_LOCALNET_KEEP=1` (passed).
 - [x] Re-run `cargo test -p integration_tests --test mod network_stable_after_add_and_after_remove_peer -- --nocapture` after the RBC roster fallback (pass).
- [x] Re-run `cargo test -p integration_tests --test mod -- --nocapture` with `API_ADDRESS`/`PUBLIC_KEY` env overrides set to confirm test-network peers ignore host env config overrides (2026-02-01: `API_ADDRESS=not_a_socket PUBLIC_KEY=deadbeef IROHA_TEST_BUILD_PROFILE=release IROHA_TEST_SKIP_BUILD=1 IROHA_TEST_NETWORK_PARALLELISM=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) TEST_NETWORK_TMP_DIR=/Users/mtakemiya/dev/iroha/tmp/test_network TEST_NETWORK_BIN_IROHAD=/Users/mtakemiya/dev/iroha/target/release/iroha3d TEST_NETWORK_BIN_IROHA=/Users/mtakemiya/dev/iroha/target/release/iroha cargo test -p integration_tests --test mod -- --nocapture --test-threads=1` passed).
- [x] Re-run `cargo test -p integration_tests --test mod` after the payload-hash stabilization fix (strip results/extra signatures from DA/RBC payload bytes) and the block-sync seen-block filtering; attempt 2025-12-31 timed out after 5m with pipeline event failures, peers waiting for block 1, and status endpoint connection refused—confirm the gating condition is resolved (2026-02-01 run above passed).
- [x] Re-run `cargo test -p integration_tests --test mod` after stabilizing NPoS PRF seed handling (seed fixed within epoch + next-epoch record persisted at rollover + replay PRF rotation) to confirm event/connected-peers suites no longer hang (2026-02-01 run above passed).
- [x] Re-run `cargo test -p integration_tests --test mod` after preserving `proposals_seen` across membership changes (prevents re-proposing the same view during roster updates) to confirm peer membership tests no longer stall consensus (2026-02-01 run above passed).
- [ ] Re-run `cargo test --workspace` after the Sumeragi gap fixes; latest attempt (`cargo test -p iroha_core --lib sumeragi::main_loop::tests`) timed out after 600s; active-topology world-peer ordering and locked-QC status flake were fixed afterward.
- [x] Fix block-sync seen-block tracking to avoid marking uncommitted payloads (prevents catch-up stalls), add unit coverage, and re-run `extra_functional::connected_peers::register_new_peer` (pass; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_y8a4wO`).

4. **ADAPTIVE-PACING-FLOOR — Genesis minimum finality + adaptive timing governor** (Consensus, Line: Shared, Owner: Core WG, Priority: High, Status: 🈴 Completed, target TBD)
- [x] Data model + genesis: add `min_finality_ms` to `SumeragiParameters` (Norito + JSON) and consensus genesis fingerprint; update on-chain defaults to start at the floor (100ms) and refresh config templates/docs.
- [x] Validation + clamping: enforce `commit_time_ms >= block_time_ms >= min_finality_ms` in parameter parsing and clamp runtime timing resolution (permissioned + NPoS) so commit/quorum/availability/pacemaker derive from the floored values.
- [x] Deterministic pacing governor: define window size + thresholds (view-change pressure + commit spacing), implement hysteresis (fast up/slow down) with a bounded factor, and emit `SetParameter` updates only at height boundaries.
- [x] Telemetry + tests: add pacing-factor fields to `/v1/sumeragi/status` and cover governor scale up/down decisions (effective timing fields + Norito/JSON round-trip coverage landed).

5. **SUMERAGI-PARAM-STREAMLINE — Reduce operator-facing consensus knobs** (Consensus, Line: Shared, Owner: Core WG, Priority: Medium, Status: 🈴 Completed, target TBD)
- [x] Spec: `docs/source/sumeragi_parameter_streamlining.md`.
- [x] Parameter surface: keep only the minimal on-chain/operator knobs (block/commit/min_finality/clock_drift, collectors, DA enable) and treat all other timing knobs as derived-by-default.
- [x] Config shape: introduce `sumeragi.advanced` (or equivalent) for override-only knobs (pacemaker backoff/jitter caps, DA multipliers/floors, RBC TTL/storage, worker budgets); ensure deterministic fallbacks.
- [x] NPoS overrides: move timeout overrides to the advanced section (or remove them) and derive from `block_time_ms` when unset; update validation to allow 0/None as "derive".
- [x] Observability: extend `/v1/sumeragi/status` with the remaining effective derived fields not covered by the pacing task and document the new surface.
- [x] Docs/templates: refresh config templates and docs to match the streamlined surface (no backward-compat required for first release).
- [x] Fix snapshot deserialization on restart so `extra_functional::restart_peer::restarted_peer_should_restore_its_state` can rely on persisted snapshots (2026-01-24: install domain-selector resolver from snapshot domains before parsing, remove cleanup; `cargo test -p iroha_core snapshot_read_installs_domain_selector_resolver -- --nocapture` and `IROHA_TEST_NETWORK_PARALLELISM=1 cargo test -p integration_tests --test mod restarted_peer_should_restore_its_state -- --nocapture` passed).
- [x] Resolve build-directory lock timeouts so full-workspace test runs complete reliably (standardize `CARGO_TARGET_DIR` or serialize builds).

6. **SUMERAGI-LIVENESS-GAPS — Close remaining idle view-change + pacing-governor gaps** (Consensus/Perf, Line: Shared, Owner: Core WG, Priority: High, Status: 🈺 In Progress, target TBD)
- [x] Idle view-change: allow leader rotation after the idle timeout even when consensus queues stay saturated; add logging/telemetry for the override path. (touch `crates/iroha_core/src/sumeragi/main_loop.rs`)
- [x] Idle view-change tests: add relay-backpressure branch coverage (pre-timeout blocked, post-timeout allowed) and update the consensus-queue backpressure test to assert the post-timeout override. (touch `crates/iroha_core/src/sumeragi/main_loop/tests.rs`)
- [x] Pacing governor always-on: remove/ignore the `sumeragi.advanced.pacing_governor.enabled` toggle across config defaults/user/actual parsing and runtime application; update fixtures/tests and docs/config templates to match. (touch `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`, `crates/iroha_core/src/state.rs`, config templates)
- [x] Liveness bottlenecks: cap block-queue drain per iteration under backlog (adaptive drain cap) and add coverage to keep worker-loop drain latency bounded. (touch `crates/iroha_core/src/sumeragi/mod.rs`)
- [x] Re-run Izanami 1 TPS + NPoS localnet 1 Hz after the drain-cap change and capture results in `status.md`. (2026-02-01: Izanami 1 TPS run `/tmp/iroha-test-network-izanami-1tps-20260201T054440/irohad_test_network_56VhGX` + log `/tmp/izanami_1tps_20260201T054440.log`; NPoS localnet 1 Hz run `/private/tmp/iroha-localnet-npos-1hz-20260201T101516` with `sumeragi_status_1hz_20260201T130626.jsonl` + `ping_1hz_20260201T130626.log`.) (touch `status.md`)

7. **PERF-100TPS-PIPELINE — Tx pipeline & consensus hot-spot cleanup** (Consensus/Perf, Line: Shared, Owner: Core WG, Priority: High, Status: 🈴 Completed, target TBD)
- [x] Norito encode: direct encode-into-writer for owned payloads when exact length is known; use seq size hints to reserve packed payload buffers.
- [x] P2P/gossip: reuse outbound frames, avoid cloning large tx payloads by arcing gossip transactions, and keep cached encoded bytes for retransmit.
- [x] BLS verification: reuse verified vote/QC caches and prepared-key cache; skip per-vote checks on aggregate-pass for NPoS.
- [x] Consensus payload churn: cache pre-encoded BlockMessage/RBC chunk bytes and compact RBC chunk headers for repeated fields.
- [x] Backpressure tuning: raise RBC store caps/backlog soft limits and NPoS timeout defaults; update docs/templates.

4. **LOCALNET-10K-TPS — 7-peer localnet throughput + stall resilience** (Consensus/Performance/Tooling, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈺 In Progress, target TBD)
- [ ] Use the new pending-block/commit-inflight metrics to isolate pacemaker backpressure during 100 TPS localnet runs (run151 NPoS: avg slot 5.68s, view changes 11, proposal_gap 1, pending/inflight <=4/1; run152 permissioned + commit_time_ms=300: avg slot 10.69s, view changes 18, pending/inflight <=4/1; run153 NPoS 7-peer 753ms soft limits: height 5, view changes 42, tx_queue depth 4796, pending max 5/inflight 0, missing_qc/stake_quorum timeouts; run154 permissioned 7-peer 753ms: height 5, view changes 9, tx_queue depth 11522, pending/inflight 0, missing_qc/quorum timeouts). Identify the proposal-gap root cause and re-test.
- [ ] Validate worker-iteration drain-cap impact with a load generator that sustains 50 TPS (current CLI batch loop undershoots per-second pacing) and correlate view-change spikes with proposal gaps vs queue backpressure.
- [x] Re-run the Izanami 1 TPS (300s) profile after RBC seed-worker offload to confirm BlockCreated handling latency drops and capture the updated metrics in `status.md` (2026-01-25 run recorded).
- [x] Re-run the Izanami 1 TPS (300s) profile after increasing RBC seed worker concurrency and queue caps to confirm additional BlockCreated seeding latency reduction; update `status.md` (2026-01-29 attempt: commit height 53 in ~308s, avg interval 6.05s; missed target 200; logs `/private/tmp/iroha-izanami-logs/irohad_test_network_W2m842`).
- [x] Re-run the Izanami 1 TPS profile after the pipeline shared-pool reuse + stateless cache + stateless signature batching to confirm vote/block drain improvements and update `status.md`.
- [x] Re-run the Izanami 1 TPS profile with auto-scaled `pipeline.workers` and `sumeragi.advanced.worker.validation_*` (2026-01-28 run stalled with committed height 26; vote_drain_ms mean 266ms p95 1156 max 3378; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Wk33iK`).
- [x] Investigate per-peer commit persistence stalls (Izanami `irohad_test_network_o6jN6V`: swaying_deer never logged `stored committed block to kura` for height 40 while other peers did); added commit->Kura stage breakdown metrics (kura_store/state_apply/state_commit) plus state commit view_lock wait/hold histograms to correlate persistence stalls with lock contention.
- [x] Re-run Izanami 1 TPS with expanded block-validation sub-stage timings (stateless vs execution breakdown); execution is dominated by `execution_tx_ms` (mean ~740ms, p95 ~4011ms, max ~5313ms) while other sub-stages are ~0ms (run 2026-01-28, logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_mwreSi`).
- [x] Re-run Izanami 1 TPS with tx sub-stage timings; `execution_tx_apply_ms` dominates (mean ~508ms, p95 ~3875ms, max ~5862ms) with occasional DAG spikes (max 1803ms) in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_FE6T7a`.
- [x] Split `execution_tx_apply_ms` into apply sub-stages (detached exec vs merge vs sequential fallback vs trigger execution vs finalization) to pinpoint the dominant apply hot spot before optimizing.
- [x] Re-run Izanami 1 TPS after forcing missing highest-QC fetch when the only local payload is an aborted pending block (proposal/proposal-hint paths) to confirm height progress and clear missing-highest stalls. 2026-01-30 run (4 peers, 300s, target 200, tps 1, `--max-inflight 8`) reached committed height 104–105 and stopped before the target; no missing-highest logs observed; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_LaHP2G`. 2026-01-31 debug run (4 peers, 120s, target 80, tps 1, vote/qc debug) reached committed height 55–56; votes were recorded for all signers and QCs aggregated, but tick-loop lagging showed commit_pipeline_ms spikes (~3.9s); no missing-highest logs observed; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_OnNAOB`. 2026-01-31 main_loop debug run (4 peers, 120s, target 80, tps 1, validation timing debug) reached committed height 23; `execution_tx_dag_ms` spiked to 2052ms at height 10 with tx_count=4 and DAG key_count=10 (decoded sidecar), suggesting pipeline sidecar fsync I/O in the DAG stage; `execution_tx_time_triggers_ms` spiked to 817ms at height 23; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Fv0sDg`. 2026-01-31 main_loop debug run (4 peers, 300s, target 200, tps 1) reached committed height 69; validation timings mean total_ms 8.6 p95 20; tick-loop lagging p95 tick_cost_ms ~2476 and commit_pipeline_ms max 3965; worker-iteration slow lines dominated by block drains (post_tick_blocks_drain_ms p95 ~4071); network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_xL0j16`. 2026-01-31 main_loop debug run with commit-pipeline substage timings (4 peers, 300s, target 200, tps 1) reached committed height 56; commit interval avg 5.52s p95 11.82s; tick-loop lagging p95 tick_cost_ms ~1488, commit_pipeline_ms max 4488 with drain dominating (commit_pipeline_drain_ms mean 163 p95 904) and validation mean 27 p95 110; worker iteration slow p95 elapsed ~6074 with pre/post block drains p95 3917/2915 and block_payload_drain p95 1064; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_5uSRtP`. 2026-01-31 rerun after lowering Izanami block/commit to 500/750ms with pacing_factor_bps=10000, collectors_k=3, redundant_send_r=3, and pacing governor clamped to 1x: min height 122 in 300s (target 200), no missing-highest logs observed; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_pSftTI`, log `/tmp/izanami_1tps_20260131_192200_run6.log`.
- [x] Gate pipeline/roster sidecar fsync on `kura.fsync_mode=on` (batched/off skips fsync) and rerun the 300s 1 TPS profile to confirm height progress. 2026-01-31 rerun after sidecar-fsync gating (4 peers, 300s, target 200, tps 1) stopped before the target with lane_000_core `blocks.hashes` count 130 across peers; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_99DZ8m`.
- [x] Profile/trim time-trigger execution by short-circuiting data-trigger DFS when no data triggers exist, then rerun the 300s 1 TPS profile. 2026-01-31 rerun (4 peers, 300s, target 200, tps 1) stopped before the target with lane_000_core `blocks.hashes` size 4480 bytes (~140 entries); network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Zkkr3A`.
- [x] Move pipeline recovery sidecar writes to a background worker (no consensus-path I/O) and rerun the 300s 1 TPS profile. 2026-01-31 rerun (4 peers, 300s, target 200, tps 1) stopped before the target with lane_000_core `blocks.hashes` size 4000 bytes (~125 entries); network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_3nsRop`.
- [x] Memoize commit-QC/checkpoint roster validation results (hash + stake snapshot keyed) to skip repeated BLS verification; add unit coverage.
- [x] Re-run the Izanami 1 TPS profile to confirm queue-latency reduction after roster-validation memoization (2026-01-31 run; 4 peers, 300s, target 200, tps 1) stopped before the target with lane_000_core `blocks.hashes` size 3104 bytes (~97 entries); network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_oVDDBU`.
- [x] Reduce vote-drain hotspots now that vote-verify offload + batching still leave `vote_drain_ms` dominant (latest 1 TPS run: mean 432–490ms, p95 2182–2344ms, 49–50 blocks in 300s; 2026-01-27 run after SumeragiParameters timing injection: p50 0, p90 734, max 3242, height 41/300s, logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_wWvhSZ`; 2026-01-27 run after vote-inbound deferral: p50 0, p90 778, max 3485, height 44/300s, logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_p4poFN`); vote/QC verify workers now auto-size threads/queue caps when config values are 0, buffering more work before synchronous fallback.
- [x] Investigate Izanami 1 TPS QC fast-path run `irohad_test_network_bLPG1H`: peer exit status 3 and Torii connection refusals; stabilize startup and re-run the profile. 2026-01-31 rerun: no Torii connection refusals or exit status 3 observed; stopped before target at min height 122; network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_pSftTI`, log `/tmp/izanami_1tps_20260131_192200_run6.log`.
- [x] Enable expensive-metrics capture for throughput runs (telemetry profile/config) or surface pending-block + commit-inflight gauges in `/v1/sumeragi/status`; current `/metrics` scrapes are empty under the test-network defaults.
- [ ] Investigate skewed NEW_VIEW participation during throughput stalls (recent NPoS/permissioned runs show `new_view_slots` max < quorum with per-peer NEW_VIEW vote traffic heavily skewed).
 - [x] NPoS roster selection now appends active validators missing from the commit topology so the full validator set participates; unit coverage added.
 - [x] Seed commit topology from the checkpoint topology when world peers are incomplete (prevents genesis roster shrink/QC stalls); unit coverage added.
 - [x] Add a Kagami/localnet regression that asserts validator count + commit QC validator-set length match the peer count on startup.
 - [x] Scale bulk consensus-ingress caps for 1s block times, emit per-topic drop/penalty metrics for payload traffic, and update localnet defaults (no legacy compatibility toggles).
 - [x] Refactor `iroha transaction ping` to cap parallel workers (`--parallel-cap`, default 1024; 0 disables) and reuse a single `Client` per batch; add unit coverage for the cap logic.
 - [x] Extend `scripts/tx_load.py` to shard load across Torii peers (`--peer-urls`/`--peer-count`), report rate-limit hits, and keep per-shard stats (tests added).
 - [x] Add an ignored 7-peer localnet throughput regression (target 10k tps, 1s block time) that asserts block height advances, commit time stays near target, and no consensus stalls occur.
 - [x] Define 1s-finality SLO thresholds for permissioned and NPoS (p95/p99 commit-QC latency, view-change rate, queue backpressure).
 - [x] Specify pass/fail criteria and measurement windows (warm-up, steady-state duration, no-height-advance timeout).
 - [x] Document the SLO acceptance table for both modes in the performance docs (see `docs/source/sumeragi_localnet_throughput.md`).
 - [x] Enumerate required status/telemetry fields for throughput runs (status endpoints, metrics names) and list any gaps to add.
 - [x] Add a throughput report template (run metadata, config fingerprint, hardware, commands, logs/metrics paths).
 - [x] Add a script or harness output to emit the report artifacts (status snapshots, metrics export, log paths).
 - [x] Add explicit 10k TPS / 1s finality config profile for permissioned mode (block/commit times, collectors_k/r, payload/queue caps) in Kagami templates and docs.
 - [x] Add explicit 10k TPS / 1s finality config profile for NPoS mode (NPoS timeouts, collectors_k/r, payload/queue caps, stake bootstrap knobs) in Kagami templates and docs.
 - [x] Wire a profile selector into the localnet runner so permissioned/NPoS perf profiles are easy to switch.
 - [x] Extend the load harness with a deterministic 10k TPS recipe (tx type/size, batch, parallelism, peer count, fixed RNG seed).
 - [x] Add warm-up + steady-state phases and stall detection (no-height-advance timeout) to the harness run flow.
 - [x] Emit standardized throughput/finality metrics from the harness (commit-QC latency p95/p99, view-change rate, queue backpressure, submitted/committed TPS) for comparison runs.
 - [x] Investigate pending-block backpressure gating (blocking proposals until quorum reschedule) that keeps localnet cadence near commit-quorum timeout; design a safe fast-path to reach sub-1s finality without speculative execution (added fast-path unblock when no votes/QCs arrive by `min(block_time, commit_time)`, applied in proposal backpressure + quorum reschedule; unit coverage added and pacemaker docs updated).
- [ ] Run the ignored 7-peer localnet throughput regression in permissioned mode and capture throughput/commit-time metrics in `status.md` after the oversized BlockSyncUpdate fetch fallback fix (attempts failed: submit queue stalled at ~22k/42k with min_non_empty=1-2; release runs timed out after 20m; latest debug run timed out after 20m with height stuck at 2 and repeated view-change/missing-QC logs; logs in `/var/folders/7l/w31n0ppj4zg874c4szhllss00000gn/T/irohad_test_network_uDjmli`, `/var/folders/7l/w31n0ppj4zg874c4szhllss00000gn/T/irohad_test_network_cO9vgq`, `/var/folders/7l/w31n0ppj4zg874c4szhllss00000gn/T/irohad_test_network_TY25OZ`, `/var/folders/7l/w31n0ppj4zg874c4szhllss00000gn/T/irohad_test_network_hMZDGM`; 2026-01-22 release run failed with submit queue not draining below 20000 within 180s, artifacts in `/private/tmp/iroha-throughput-permissioned-20260122b/throughput-1769106279251`, network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_gIn3el`; 2026-02-02 release run failed with submit queue stuck ~22.5k (min_non_empty=1), not draining below 20000 within 180s; artifacts `/tmp/iroha-throughput-permissioned-20260202b`, network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_q2bRUM`).
- [ ] Run the ignored 7-peer localnet throughput regression in NPoS mode and capture throughput/commit-time metrics in `status.md` after the above gates and harness outputs are in place (2026-01-22 release run failed with submit queue not draining below 20000 within 180s; artifacts in `/private/tmp/iroha-throughput-npos-20260122/throughput-1769107672300`, network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_Xq0l0o`; 2026-02-02 release run failed with submit queue stuck ~22.5k (min_non_empty=1), not draining below 20000 within 180s; artifacts `/tmp/iroha-throughput-npos-20260202b`, network dir `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_c6Gw2W`).
 - [x] Re-run the NPoS localnet 1 Hz / 100-block check with `/v1/sumeragi/status` sampling after the progress-age quorum timeout change; capture any stalls in `status.md` (2026-01-21 run on `/tmp/iroha-localnet-npos-1hz-run2`: `commit_qc.height` +12 over 118s, `view_change_install_total` +3, pending RBC max 1040 bytes / 1 session).
 - [x] Re-run the NPoS localnet 1 Hz / 100-block check with `/v1/sumeragi/status` sampling after the progress-age quorum timeout change and commit-pipeline backlog-bypass tweaks; capture any stalls in `status.md` (2026-01-23 run at `/private/tmp/iroha-localnet-npos-1hz-20260123153657`: `commit_qc.height` 0->6 over ~140s, `view_change_install_total` +3, `pending_rbc.bytes` max 2960 with sessions max 4; cadence still below 1 Hz).
 - [x] Re-run the NPoS localnet 1 Hz soak after the FetchPendingBlock missing-INIT response fix to confirm INIT recovery without waiting on full blocks. (ran 2026-01-24 on `/private/tmp/iroha-localnet-npos-1hz-20260124T190127Z`, see `sumeragi_status_1hz_missing_init_debug_20260124T190127Z.jsonl`).

4. **DA-TORII-REFACTOR — Decompose Torii DA ingestion + erasure coding** (Torii/DA, Line: Shared, Owner: Torii WG, Priority: High, Status: 🈺 In Progress, target TBD)
 - [x] Split Torii DA into `crates/iroha_torii/src/da/` with `mod.rs`, `ingest.rs`, `persistence.rs`, `taikai.rs`, `rs16.rs`, and `tests.rs`; keep the public API in `mod.rs` and allow internal breakage for the first release.
 - [x] Extract rs16 erasure coding into a shared module (prefer `iroha_primitives`); update Torii ingestion + `crates/sorafs_car/src/bin/da_reconstruct.rs` to use the shared implementation, delete duplicated fixture logic, and add shared unit tests/fixtures.
 - [x] Add SIMD backends (NEON, AVX2) for rs16 matrix/field ops behind feature flags with deterministic fallback; add parity tests across backends and document the accel knobs.
 - [x] Add a `torii_da_chunking_seconds` histogram around erasure coding/segment assembly, wire it into telemetry exports, and document it in telemetry docs with unit coverage for metric registration.
 - [x] Finish module boundary cleanup (visibility tweaks, test relocation, and lint fixes).
 - [x] Verify Torii DA unit tests pass.
 - [ ] Run `cargo test --workspace` and update `status.md` with the results (attempted; sandbox timed out).

5. **LOCALNET-DEMO-FLOW — Verify training-script localnet bootstrap** (Consensus/Tooling, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Add an ignored localnet soak integration test that drives thousands of blocks/transactions and document how to run it.
 - [x] Align consensus rebroadcast cooldowns (RBC/pending/precommit) to on-chain `block_time` with a 200ms floor and 2x base multiplier, keeping payload replays at an additional 2x to avoid queue saturation on 1s targets.
 - [x] Widen Sumeragi worker loop drain budgets and block `BlockSyncUpdate` enqueue under backpressure to prevent DA-localnet asset test hangs.【crates/iroha_core/src/sumeragi/mod.rs】
 - [x] Switch localnet + defaults to per-queue Sumeragi channel caps (votes/payload/RBC/block) tuned for 20k TPS and 1s block times.【crates/iroha_kagami/src/localnet.rs】【crates/iroha_config/src/parameters/defaults.rs】
 - [x] Run the updated `training_script_2` workflow against a fresh 4-peer localnet and confirm block 2+ finalize quickly with DA on and no dev bypasses (full script completed; peer0 shows height 2 committed at 10:18:12.75 and height 10 at 10:18:20.84 in ~1s steps; localnet shut down).
 - [x] If Torii readiness or CLI timeouts still stall the run, adjust the script with bounded retries/timeouts and capture the needed Torii readiness checks.
 - [x] Remove empty-child fallback proposals so pacemaker stays idle without queued work; unit tests + docs updated.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】
 - [x] Validate the block-sync lock realignment path (commit-certificate override) against the localnet stall scenario; 20-run sweep shows no height-49 view-change loop and ~1s block deltas at height 49.
 - [x] Let validated block-sync precommit QCs realign locks even without commit certificates to cover QC-only commits; regression ensures non-extending QC caches and advances the lock.【crates/iroha_core/src/sumeragi/main_loop/block_sync.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】
 - [x] Repeat `training_script_2` runs (>=3) on a fresh 4-peer localnet to confirm 1s cadence stability and capture any hangs/timeouts after pacemaker proposal gating changes (17/20 completed; 3 tx confirmation timeouts captured).
 - [x] Catch up on higher NEW_VIEW frames and fan out NEW_VIEW votes to deterministic collectors with topology fallback (leader included) so quorum converges without broadcast storms; tests/docs updated.【crates/iroha_core/src/sumeragi/main_loop/commit.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】【docs/source/p2p.md】【docs/source/references/peer.template.toml】
 - [x] Drop stale-view consensus traffic (proposal/block, votes/Prevote QCs, block sync, RBC) to prevent old-view payloads from forming conflicting commits during localnet retries, while allowing availability/precommit QCs to clear DA/commit gates; tests added.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/main_loop/proposal_handlers.rs】【crates/iroha_core/src/sumeragi/main_loop/votes.rs】【crates/iroha_core/src/sumeragi/main_loop/qc.rs】【crates/iroha_core/src/sumeragi/main_loop/rbc.rs】【crates/iroha_core/src/sumeragi/main_loop/block_sync.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【sumeragi.md】
 - [x] Emit availability votes even when validation is deferred (block ahead of local height) so DA/QC formation stays live while lagging peers catch up; tests added.【crates/iroha_core/src/sumeragi/main_loop/commit.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】
 - [x] Include commit-inflight + processing blocks in locked-chain ancestry lookups so precommit/QC aggregation and proposals don't stall while a parent is mid-commit; tests added.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【crates/iroha_core/src/sumeragi/main_loop/commit.rs】
 - [x] Drop precommit votes that are below or conflicting with the lock on receipt and prune stale non-extending precommit votes as the lock advances to avoid non-extending QC churn in localnet; tests added.【crates/iroha_core/src/sumeragi/main_loop/votes.rs】【crates/iroha_core/src/sumeragi/main_loop/qc.rs】【crates/iroha_core/src/sumeragi/main_loop/commit.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】
 - [x] Increase DA commit quorum timeout slack, align availability gate timeout with the shared helper, and allow stale-view RBC payload messages for known pending blocks to clear availability after view changes; tests/docs updated.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/main_loop/commit.rs】【crates/iroha_core/src/sumeragi/main_loop/reschedule.rs】【crates/iroha_core/src/sumeragi/main_loop/rbc.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】
 - [x] Accept stale precommit votes for known pending blocks after view changes so late votes can still form precommit QCs; tests/docs updated.【crates/iroha_core/src/sumeragi/main_loop/votes.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】
- [x] Anchor pipeline recovery sidecars to the block hash (v1) so DAG fingerprint checks only apply to the exact block and stale sidecars from forks/restarts are ignored; tests/docs updated.【crates/iroha_core/src/kura.rs】【crates/iroha_core/src/block.rs】【docs/source/pipeline.md】
 - [x] Realign divergent locked QCs to the committed tip before proposing so leaders don't stall on forked locks; unit test + docs updated.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/main_loop/propose.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】
 - [x] Re-run `training_script_2` (20x) after the precommit vote lock + proposal deferral to confirm no forked commits, no long stalls, and ~1s block cadence across runs (20/20 succeeded; height10 8-9s; no long stalls).
 - [x] Re-run `training_script_2` with the fast localnet pipeline defaults, DA availability timeout tuning, and redundant-send fanout to confirm tx status timeouts and rare >40s gaps are gone (20/20 succeeded; no tx status timeouts or >40s gaps; ~0.95s cadence).【crates/iroha_kagami/src/localnet.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Re-run localnet after the availability-vote emission fixes (including deferred-validation votes) to confirm missing-availability QC warnings and DAG fingerprint mismatch warnings are gone (0 missing-availability warnings; 0 DAG fingerprint mismatches).
 - [x] Re-run the seven-peer DA consistency test after restoring the DA quorum timeout multiplier to confirm view-change stalls no longer stretch commits (pass: `cargo test -p integration_tests --test mod seven_peer_cross_peer_consistency_basic`).【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Eliminate remaining tx status timeouts (approve/propose/register) and rare >40s block gaps observed during the 20-run localnet sweep.【crates/iroha/src/client.rs】【crates/iroha_kagami/src/localnet.rs】【crates/iroha_kagami/README.md】
 - [x] Seed peer telemetry with explicit Torii URLs in localnet configs so peer monitors stop hitting P2P ports during demos.【crates/iroha_torii/src/lib.rs】【crates/iroha_kagami/src/localnet.rs】【docs/source/telemetry.md】

6. **INTEGRATION-TEST-STABILITY — Replace transient skips with deterministic readiness** (QA/Consensus, Line: Shared, Owner: QA WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Audit integration tests that now skip on timeouts (events pipeline, sumeragi_da, unstable_network, genesis/offline peers, triggers/time triggers) and replace with deterministic readiness probes or tuned deadlines.
 - [x] Investigate serial-guard port contention and long startup waits; tighten network teardown or add cleanup hooks to reduce startup stalls.
 - [x] Re-run `cargo test -p integration_tests --test mod` without skip paths and confirm stability across at least three consecutive runs.

7. **SUM-ACTOR-SPLIT-OBSERVABILITY — Decompose Sumeragi actor and harden inflight commit safety** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Split DA/VRF/commit state into sub-actors/state modules with explicit interfaces (DA state isolated, VRF actor extracted, commit inflight tracking isolated).
 - [x] Added backpressure/queue diagnostics plus inflight duration metrics and `/v1/sumeragi/status` hooks for queue depths/diagnostics + commit inflight snapshots.
 - [x] Added inflight timeout/abort logic (requeue stalled commits) with unit coverage to prevent permanent stalls.

8. **OFFLINE-FLOW-BLOCKERS — Complete issuer APIs, FASTPQ proofs, and platform proof plumbing** (Core/Torii/SDK, Line: Shared, Owner: Offline WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Issuer API spec: define OfflineWalletCertificate issuance/renewal endpoints (operator-signed), request/response payloads, attestation nonce flow, and spend-key custody rules (wallet-generated keypair, public-only submission; define any import/rotation path); update `docs/source/offline_allowance.md` + Torii OpenAPI.
 - [x] Issuer service implementation: add Torii app_api routes or a dedicated issuer service, add `iroha_config` knobs for operator keys/allowed controllers, and enforce controller/operator authorization; include issue/renew/revoke integration tests.
 - [x] SDK issuance helpers: add Swift/Android/JS clients for issue/renew endpoints; model `OfflineWalletCertificateDraft`/responses; document spend-key custody (wallet-generated keypair, public-only submission) and any import/rotation path.
 - [x] FASTPQ bridge (Poseidon): expose receipts-root hashing via `connect_norito_bridge`, wire `NoritoNativeBridge`/Swift helpers, and keep deterministic CPU fallback while documenting Metal/CUDA acceleration hooks.
 - [x] Aggregate proof envelope wiring: build `AggregateProofEnvelope` from witness payloads, define metadata keys for circuit/parameter set selection, and add regression fixtures (Rust + Swift) for proof bytes + receipts_root parity.
 - [x] Proof-mode enforcement: add `settlement.offline.proof_mode` in `iroha_config` (optional vs required), enforce missing/invalid proofs in admission, and update docs/tests.
 - [x] Platform counters storage: add SDK-side `OfflineCounterState` persistence with summary-hash parity to `OfflineCounterSummary`, sync from `/v1/offline/summaries`, and validate monotonic increments before bundle submission.
 - [x] Play Integrity / HMS wrappers: implement Android SDK helpers for token acquisition/caching, nonce + TTL handling, and metadata validation; add Swift/JS models for token snapshot ingest/validation; align required metadata keys with Torii validation.
 - [x] App Attest/KeyMint counter flow: add SDK APIs to read last counter checkpoints (App Attest/KeyMint + provisioned), validate monotonic increments, surface mismatch errors, and add tests against fixtures/Torii summary payloads.

8. **SUM-RBC-ROSTER-PERSISTENCE — Persist RBC session rosters across restarts** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Persist per-session commit-topology snapshots in the RBC store so READY/DELIVER validation survives restarts and roster flips; include versioned migration handling and restart coverage.
 - [x] Add a 4-peer DA integration test that restarts a validator during an in-flight RBC while unregistering another peer, asserting recovery succeeds with persisted rosters.【integration_tests/tests/sumeragi_da.rs】

9. **GOV-API-WIRING — Implement governance API + execution pipeline** (Core/Torii/Governance, Line: Shared, Owner: Governance WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Define signature schemes for `GovernanceEnactment` and `ParliamentEnactment`; update data model + docs.
 - [x] Replace Torii governance endpoint stubs with real validation, transaction construction, and execution wiring.
 - [x] Persist governance proposals/ballots/enactments in WSV and model full state transitions.
 - [x] Add end-to-end tests for proposal/ballot/enactment flows, authorization gates, and rejection paths.
 - [x] Update governance docs and Torii OpenAPI to reflect the real API surface.

10. **OFFLINE-QR-STREAM — Animated QR transport for offline payloads** (Core/SDK/CLI, Line: Shared, Owner: Offline WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Spec: define `QrStreamEnvelope` + `QrStreamFrame` wire format (Norito), payload kinds, CRC32 rules, chunking, replay guards, and size caps; include deterministic hashing + schema binding in `docs/source/qr_stream.md`.
 - [x] Spec: define frame scheduling (header cadence, data/parity frames), binary QR requirements, and capacity profile (ECC level + chunk size bounds) with a compatibility matrix for iOS/Android/JS scanners.
 - [x] Parity frames: select a deterministic erasure scheme (reuse `iroha_primitives::rs16` if possible) and define parameters/seed derivation + target drop tolerance.
 - [x] Parity frames: implement encode/decode + recovery tests (out-of-order, dropped frames, duplicates) in Rust helpers.
 - [x] Fixtures: generate deterministic fixtures under `fixtures/qr_stream/` (receipt + transfer payloads, envelope bytes, frame bytes, parity frames) and document regeneration flow.
 - [x] Data model: add `qr_stream` types + helpers to `iroha_data_model` (encode/decode, CRC32, assembler state machine) with unit coverage.
 - [x] Data model: add assembler caps (max payload bytes/frames, timeout policy) and tests for rejection paths.
 - [x] CLI tooling: add `iroha offline qr encode` (binary QR mode, chunk sizing, ECC/fps controls, SVG/PNG + animated GIF/APNG export) with tests.
 - [x] CLI tooling: add `iroha offline qr decode` (frames/dir input, hash verification, Norito/JSON output) with tests.
 - [x] Swift SDK: add QrStream encoder/decoder + ScanSession core with unit tests.
 - [x] Swift SDK: integrate Vision/AVFoundation scan pipeline to feed raw bytes into ScanSession + fixture tests.
 - [x] Android SDK: add QrStream encoder/decoder + ScanSession core with unit tests.
 - [x] Android SDK: integrate CameraX/ZXing scan pipeline to feed raw bytes into ScanSession + fixture tests.
 - [x] JS SDK: add QrStream encoder/decoder + ScanSession core with unit tests.
 - [x] JS SDK: add scan-loop integration (browser/Node) + fixture tests.
 - [x] SDK theme palette + frame-style helpers for sakura animation (colors, pulse, petal phase).
 - [x] Streaming animation: ship on-brand playback skins (petal drift, warm gradient, progress overlay), plus reduced-motion and low-power variants for accessibility/perf.
 - [x] UX guidance: add scan timing + progress UI guidelines and fallback flows to `docs/source/offline_allowance.md` + SDK guides.
 - [x] Optional Petal Stream transport: add petal-grid framing + CLI encode/decode + JS/Swift/Android scan helpers + docs for custom scanners.

10. **ZK-VERIFY-COMPLETE — Full ZK verification + prover** (ZK/Core/Torii, Line: Shared, Owner: ZK WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Route `halo2/ipa` OpenVerifyEnvelope decoding into the IPA verifier, remove placeholder acceptance, and refresh tiny-add fixtures/tests to use real proofs + VK bytes.
 - [x] Resolve `vk_ref` to key bytes for `VerifyProof` before verification.
 - [x] Replace Torii ZK prover stub with a real proving pipeline (or remove the feature if unsupported).
 - [x] Define prover scope + plumbing: supported backends/circuits, proving-key storage, attachment-to-circuit mapping, and Torii report schema fields (backend, vk_ref, proof hash).
 - [x] Decide Groth16/STARK support: implement verifiers or drop stub tags; document the supported backend matrix.
 - [x] Expand deterministic verification fixtures/tests beyond tiny-add (public-instance and multi-row circuits).
 - [x] Resolve merge-conflict markers in `crates/iroha_core/src/zk.rs` that currently break builds/tests.
 - [x] Update ZK docs/configs to reflect the expanded fixture set and supported test circuits.

11. **SUMERAGI-ACTOR-REFACTOR — Replace ad-hoc worker loop with a deterministic actor model** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Document the current worker loop/queue topology, ordering rules, backpressure behavior, and determinism constraints; capture known failure modes and desired invariants.
 - [x] Define a new message envelope with explicit priority classes and deterministic tie-breakers; map `BlockMessage`, `ControlFlow`, RBC, lane relay, and background requests to priority tiers.
 - [x] Implement a single scheduling layer (priority mailbox + bounded budgets) that replaces multi-queue draining while preventing starvation and preserving deterministic processing.
 - [x] Move background network posting into a dedicated worker with backpressure + metrics; remove inline-send fallback paths from the actor loop.
 - [x] Upgrade the current FIFO dedup caches to bounded LRU/TTL structures partitioned by message kind; add eviction metrics and duplicate-suppression tests.
 - [x] Split `Actor` state into explicit subcomponents with minimal shared mutable state (commit, propose/pacemaker, DA/RBC, VRF, merge/lane relay) and unit-testable interfaces.
 - [x] Add deterministic replay tests: given a fixed message trace, the scheduler produces identical state transitions and outputs across runs; include saturation and fairness scenarios.
 - [x] Update `docs/source/sumeragi.md`, `/v1/sumeragi/status` telemetry notes, and `status.md`/`roadmap.md` migration guidance to reflect the new actor model.

12. **SUMERAGI-BCERT-REFLOW — Replace QC commit gating with B-Chain commit certificates** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Update API/telemetry surfaces (`/v1/sumeragi/status`, `/v1/status`, Prometheus labels) to reflect commit certificates and quorum tracking.
 - [x] Add integration coverage (commit-vote counters via `/v1/sumeragi/status` + metrics, stake quorum, commit-cert block sync, Kagami 4+ peer localnet bootstrap).
 - [x] Capture commit-certificate migration notes in `status.md` once integration coverage is in place.

13. **QUERY-DSL-IMPLEMENTATION — Replace lightweight query DSL stubs** (Data Model/Core/SDK, Line: Shared, Owner: Data Model WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Implement real predicate/projection semantics for `query::dsl` and `query::dsl_fast`.
 - [x] Ensure Norito serialization captures predicate payloads and remains deterministic.
 - [x] Wire Torii config limits into query post-processing and require config in request limit validation.
 - [x] Add unit/integration tests; update builder APIs and examples.

15. **IZANAMI-SUMERAGI-FRAME-CAP — Validate consensus frame cap + RBC chunk clamp** (Consensus/QA, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈺 In Progress, target TBD)
- [x] Re-run Izanami 4-peer DA runs to confirm no `FrameTooLarge` disconnects and consensus reaches target blocks (2026-01-23 run: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami -- --allow-net --nexus --peers 4 --faulty 0 --duration 300s --target-blocks 20 --progress-interval 10s --progress-timeout 180s --tps 5 --max-inflight 8 --workload-profile stable` completed with 112 successes/0 failures; no `FrameTooLarge` observed; duplicate metric registration warnings persisted).
 - [ ] Re-run `cargo test --workspace` (latest attempt timed out after 600s during compilation; see `status.md`).
 - [x] Size consensus frames using NetworkMessage wire lengths, trim BlockSyncUpdate payloads to fit caps, and guard background consensus sends against oversize frames.
 - [x] Trim proposal tx batches in bulk when BlockCreated frames exceed consensus caps to avoid O(n^2) assembly stalls; add unit coverage.
 - [x] Harden Norito length decoding to reject u64->usize overflows across core and AoS columnar views; add regression coverage.
 - [x] Tighten Norito StreamMapIter packed-seq offset validation and payload length accounting; add regression coverage.
 - [x] Validate NCB name/opt-str offsets for monotonicity and use canonical DecodeFromSlice in Norito streaming types; add regression coverage.
 - [x] Harden NCB columnar views with checked length multipliers, enum dict code validation, and u32-delta bounds; add regression coverage.
 - [x] Validate NCB dict codes, harden opt-column length parsing, and guard SIMD bundle length handling; add regression coverage.
 - [x] Reject id-delta underflow in NCB views and add regression coverage for delta-coded ids.
 - [x] Guard opt-column presence-bitset masking against 32-bit overflow in optstr/optu32 NCB views.
 - [x] Enforce ABI v1 only (remove Experimental policy paths) and update related tests/docs.
- [x] Remove Norito toggles and length/offset decode paths (update `norito.md`).
 - [x] Standardize Norito length-prefix flag scoping (COMPACT_LEN/COMPACT_SEQ_LEN/VARINT_OFFSETS) and update the spec/docs.
- [x] Sweep docs/translations for remaining references and clean up tooling flags.

15. **SUMERAGI-UNIFIED-QC — Merge commit + execution certificates into a single QC** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Define unified `QC`/`QcVote` types (rename Commit*), add `parent_state_root` + `post_state_root`, update Norito schema, and bump `PROTO_VERSION`.
 - [x] Update vote preimage/signing rules to include roots for all phases (zero roots for non-Commit) and refresh signature tests/fixtures.
 - [x] Remove ExecVote/ExecutionQC wire messages and handlers; decide ExecWitness scope (pipeline-only) and update `BlockMessage`/event plumbing.
 - [x] Capture execution roots during pre-vote validation (witness capture) and store them with pending blocks.
 - [x] Require commit votes to use stored roots and remove commit-phase recomputation.
 - [x] Refactor QC aggregation/validation to verify roots + signers, and remove exec_vote_log/execution_qc_cache tracking and telemetry.
 - [x] Evidence updates: drop conflicting-root evidence variants, treat conflicting roots as double-commit evidence, and update evidence validation/tests.
 - [x] Storage updates: remove execution-QC/root storage, persist QC roots for audit/bridge, update tiered storage segments/snapshots.
 - [x] Block sync + bridge: adapt QC derivation/validation and finality proof paths to the unified QC layout (roots included).
 - [x] Config cleanup: remove execution-QC gating toggles, collapse ProofPolicy to QC (+ optional ZK), update defaults.
 - [x] Docs/telemetry/tests: pipeline/sumeragi/telemetry docs, status/metrics fields, SDK clients/tests, and perf fixtures updated for unified QC; evidence language refreshed.

16. **SUMERAGI-RBC-MISMATCH-COVERAGE — Fill remaining JSON coverage gaps** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: Low, Status: 🈴 Completed, target TBD)
- [x] Add a JSON roundtrip assertion for `rbc_mismatch` in `sumeragi_status_wire_roundtrip_to_json_preserves_fields` (client JSON payload).
- [x] Add a consensus routing JSON test for `rbc_mismatch` in `crates/iroha_torii/src/routing/consensus.rs`.
- [x] Run targeted tests for the new assertions (client + torii routing).
- [x] Add unit coverage that stashes a mismatched RBC chunk with a sender before INIT, flushes pending RBC, and asserts per-peer mismatch counters increment.
- [x] Include sender context in hydrated payload mismatch logs when available (payload hash/digest/root mismatch paths).
- [x] Run targeted unit tests for the mismatch attribution gap fixes and record results in `status.md`.

17. **SUMERAGI-STATE-MACHINE-FIXES — Resolve aborted-pending recovery and mode-flip edge cases** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
- [x] Define and implement the recovery path for aborted pending blocks (allow BlockCreated/BlockSync to re-admit payloads, refresh tracking state, and preserve validation status for safe finalize).
- [x] Ensure missing-block requests are cleared when BlockCreated/BlockSync is accepted via duplicate/processing/inflight fast paths to avoid stale view-change triggers.
- [x] Gate runtime mode flips while commit pipeline work is active (pending_processing/commit inflight) and surface the blocked reason via status/telemetry.
- [x] Add unit coverage for aborted-pending recovery, mode-flip with inflight/pending_processing, and missing-block request clearing on duplicate/processing/inflight BlockCreated paths.
- [x] Suppress idle view-change timeouts while a commit is inflight so local execution does not trigger spurious view rotations.

17. **BRIDGE-RECEIPTS-TYPED — Replace bridge log stub with typed events** (Core/Executor/CLI, Line: Shared, Owner: Core Protocol WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Emit typed Data/event payloads for bridge receipts (remove log stub).
 - [x] Update CLI to submit real bridge receipts; add tests + docs.

18. **SORAFS-GATEWAY-LIVE — Replace fixture gateway stub with live integration** (SoraFS/Core, Line: Iroha 3, Owner: SoraFS WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Wire gateway to the real storage/pinning pipeline and remove fixture-only logic.
 - [x] Replace `proof-stub` header acceptance with real proof verification and update SoraFS gateway tests/fixtures.
 - [x] Retire `sorafs_manifest_stub`/`sorafs_provider_advert_stub` usage once real manifest/proof pipelines land; update CLI helpers and docs.
 - [x] Add integration tests and update runbooks.

19. **SORANET-RELAY-LIVE — Implement SoraNet relay** (Networking, Line: Iroha 3, Owner: Networking WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Implement relay protocol, persistence, and telemetry; remove stub designation.
 - [x] Add tests and docs for relay operations.

21. **P2P-NOISE-HANDSHAKE — Implement noise handshake path** (P2P/Core, Line: Shared, Owner: Networking WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Implement `noise_handshake` feature behavior (not just the default handshake).
 - [x] Add alignment tests and update docs.

23. **TESTS-FLAKY-UNBLOCK — Re-enable ignored/failing tests** (QA/Consensus, Line: Shared, Owner: QA WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Re-run `integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_recovers_after_restart_with_roster_change` after the ExecutionQC stale-view handling update and confirm strict ExecQC gating clears.
 - [x] Verify relay-based unstable network tests after serializing startup (`start_network_under_relay` now holds the global serial guard); investigate remaining timeouts if any.
 - [x] Stabilize and re-enable `connected_peers_with_f_2_1_2`.
 - [x] Fix trigger executor path in `integration_tests/tests/upgrade.rs` and re-enable the test.
 - [x] Rewrite trigger integration tests to async with shorter timings.
 - [x] Re-run `integration_tests/tests/sumeragi_*` after restoring full-roster QC quorum counting (Set B signatures count) to confirm quorum acceptance behavior.

24. **DOCS-I18N-STUBS — Replace auto-generated translation stubs** (Docs, Line: Shared, Owner: Docs WG, Priority: Low, Status: 🈺 In Progress, target TBD)
 - [x] Inventory stubbed translations by locale + directory (current scan: ~17.5k stubs across ar/es/fr/he/ja/pt/ru/ur + unknown); publish a tracker under `docs/i18n/` with counts and ownership.
 - [x] Replace repo-root `MAINTAINERS.*` stubs with translations (ar/es/fr/pt/ru/ur).
 - [x] Replace repo-root `integrated_test_framework.es.md` stub with a Spanish translation.
 - [x] Replace repo-root `integrated_test_framework.fr.md`, `integrated_test_framework.pt.md`, and `integrated_test_framework.ru.md` stubs with translations.
 - [x] Replace repo-root `integrated_test_framework.ar.md` stub with an Arabic translation.
 - [x] Replace `docs/source/soranet_gateway_hardening.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations.
 - [x] Replace `docs/automation/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations.
 - [x] Replace `docs/automation/android/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations.
 - [x] Replace `docs/automation/da/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations.
 - [x] Replace `docs/bindings/README.*` stubs (ar/es/fr/he/ja/pt/ru/ur) with translations.
 - [x] Replace `docs/devportal/try-it.*` stubs (he/ja) with translations.
 - [x] Replace `docs/settlement-router.*` stubs (he/ja) with translations.
 - [x] Replace `docs/space-directory.*` stubs (he/ja) with translations.
 - [x] Replace `docs/amx.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/governance_playbook.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/taikai_cache_hierarchy.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet_vpn.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soracles_evidence_retention.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/p2p_trust_gossip.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/compute_lane.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soracles.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soradns_ir_playbook.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet_gateway_billing.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet_gateway_billing_m0.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/mochi/packaging.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/mochi/index.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/mochi/quickstart.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/mochi/troubleshooting.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/benchmarks/history.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/agents.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/agents/missing_docs_inventory.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/agents/env_var_migration.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/android_support_playbook.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/compliance/android/checklists/and8_ga_2027-10_rehearsal.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/android/and7_governance_hotlist.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/android/android_support_playbook.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/android/partner_sla_discovery.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/compliance/android/eu/README.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/compliance/android/jp/README.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/android/norito_fixture_alignment.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/index.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/project_tracker/norito_streaming_post_mvp.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/compliance/android/device_lab_contingency.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/swift/connect_risk_tracker.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/crypto/attachments/sm_openssl_provenance.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/governance_pipeline.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/kagami_profiles.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/android/telemetry_override_log.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/release_dual_track_schedule.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/swift/readiness/screenshots/2026-03-05/README.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/swift/readiness/screenshots/2026-02-28/README.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/nsc-55-legal.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/nsc-42-legal.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/reports/pow_resilience.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/snnet15_m3_runbook.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/reports/circuit_stability.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/snnet15_m2_runbook.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/swift_xcframework_procurement_request.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/project_tracker/sorafs_pin_registry_tracker.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/torii/norito_rpc_tracker.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/connect_architecture_followups.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/gar_cdn_policy_bus.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/crypto/sm_wg_sync_template.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/templates/downgrade_communication_template.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/swift/ios5_dx_completion.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/norito_stage1_cutover.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sns/reports/steward_scorecard_2026q1.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/status/soranet_testnet_weekly_digest.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sdk/swift/readiness/reports/202603_and7_quiz.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/torii/api_versioning.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/zk/proof_retention.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/sorafs/migration_ledger.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet/lane_profiles.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet_billing_m0.*` stubs (he/ja) with translations.
 - [x] Replace `docs/source/soranet_gateway_bug_bounty.*` stubs (he/ja) with translations.
 - [x] Replace stub translation files across repo root, `docs/source`, and `docs/portal` (preserve front matter, keep `source_hash`/`translation_last_reviewed` aligned).
   - [x] Portal OpenAPI README stubs translated across ar/es/fr/he/ja/pt/ru/ur (`docs/portal/static/openapi/README.*`).
   - [x] Portal versioned reference stubs translated for README, publishing checklist, and Norito codec (`docs/portal/versioned_docs/version-2025-q2/reference/*.{ar,es,fr,he,ja,pt,ru,ur}.md`).
   - [x] Portal versioned Norito streaming roadmap stubs translated (`docs/portal/versioned_docs/version-2025-q2/norito-streaming-roadmap.{ar,es,fr,he,ja,pt,ru,ur}.md`).
   - [x] Portal versioned Norito overview + getting-started stubs translated (`docs/portal/versioned_docs/version-2025-q2/norito/{overview,getting-started}.{ar,es,fr,he,ja,pt,ru,ur}.md`).
   - [x] Portal versioned intro + devportal try-it stubs translated (`docs/portal/versioned_docs/version-2025-q2/{intro,devportal/try-it}.{ar,es,fr,he,ja,pt,ru,ur}.md`).
   - [x] Portal root README translated for Hebrew + Japanese (`docs/portal/README.he.md`, `docs/portal/README.ja.md`).
   - [x] Portal i18n mirrors synced for all completed translations across reference/intro/norito/devportal/quickstart sets (`docs/portal/i18n/*/docusaurus-plugin-content-docs/**` sources updated from completed docs).
   - [x] Portal SDK recipe JavaScript ledger flow translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/recipes/javascript-ledger-flow.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/recipes/javascript-ledger-flow.md`).
   - [x] Portal SDK recipe Python ledger flow translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/recipes/python-ledger-flow.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/recipes/python-ledger-flow.md`).
   - [x] Portal SDK recipe Swift ledger flow translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/recipes/swift-ledger-flow.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/recipes/swift-ledger-flow.md`).
   - [x] Portal SDK recipe Rust ledger flow translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/recipes/rust-ledger-flow.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/recipes/rust-ledger-flow.md`).
   - [x] Portal SDK recipe Java ledger flow translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/recipes/java-ledger-flow.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/recipes/java-ledger-flow.md`).
   - [x] Portal SDK recipe JavaScript Connect preview translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/recipes/javascript-connect-preview.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/recipes/javascript-connect-preview.md`).
   - [x] Portal SNS training collateral translated across locales and synced to i18n mirrors (`docs/portal/docs/sns/training-collateral.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sns/training-collateral.md`).
   - [x] Portal SNS regulatory memo EU DSA 2026-03 translated across locales and synced to i18n mirrors (`docs/portal/docs/sns/regulatory/eu-dsa-2026-03.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sns/regulatory/eu-dsa-2026-03.md`).
   - [x] Portal SNS regulatory memo EU DSA 2027-01 translated across locales and synced to i18n mirrors (`docs/portal/docs/sns/regulatory/eu-dsa-2027-01.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sns/regulatory/eu-dsa-2027-01.md`).
   - [x] Docs/source Org stubs for LTS selection + release procedure replaced by source-mirrored content with updated translation metadata (`docs/source/lts_selection.*.org`, `docs/source/release_procedure.*.org`).
   - [x] Portal SDK Rust quickstart translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/rust.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/rust.md`).
   - [x] Portal SDK Python quickstart translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/python.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/python.md`).
   - [x] Portal AI Moderation Runner specification translated across locales and synced to i18n mirrors (`docs/portal/docs/ministry/ai-moderation-runner.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/ministry/ai-moderation-runner.md`).
   - [x] Portal SDK Android telemetry redaction translated across locales and synced to i18n mirrors (`docs/portal/docs/sdks/android-telemetry-redaction.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/android-telemetry-redaction.md`).
   - [x] Portal SDK JavaScript quickstart stubs replaced across locales and synced to i18n mirrors (mirrors the canonical source content; `docs/portal/docs/sdks/javascript.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/javascript.md`).
   - [x] Portal SDK JavaScript governance & ISO examples stubs replaced across locales and synced to i18n mirrors (mirrors the canonical source content; `docs/portal/docs/sdks/javascript-governance-iso.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/javascript-governance-iso.md`).
   - [x] Portal SDK recipe JavaScript governance & ISO stubs replaced across locales and synced to i18n mirrors (mirrors the canonical source content; `docs/portal/docs/sdks/recipes/javascript-governance-iso.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sdks/recipes/javascript-governance-iso.md`).
   - [x] Portal finance settlement ISO mapping stubs replaced across locales and synced to i18n mirrors (mirrors the canonical source content; `docs/portal/docs/finance/settlement-iso-mapping.*`, `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/finance/settlement-iso-mapping.md`).
   - [x] SoraFS gateway Spanish stubs translated (`docs/source/sorafs_gateway_dns_design_agenda.es.md`, `docs/source/sorafs_gateway_operator_playbook.es.md`, `docs/source/sorafs_gateway_profile.es.md`).
   - [x] SoraFS Spanish stubs translated for chunker registry/profile authoring, portal publish plan, node plan/storage, and storage capacity marketplace (`docs/source/sorafs/chunker_registry.es.md`, `docs/source/sorafs/chunker_profile_authoring.es.md`, `docs/source/sorafs/portal_publish_plan.es.md`, `docs/source/sorafs/sorafs_node_plan.es.md`, `docs/source/sorafs/sorafs_node_storage.es.md`, `docs/source/sorafs/storage_capacity_marketplace.es.md`).
   - [x] Remaining portal/reference/versioned stubs now mirror canonical sources (all `translation_last_reviewed: null` stubs cleared in versioned portal docs).
   - [x] Portal i18n stub copies under `docs/portal/i18n/*/docusaurus-plugin-content-docs` now mirror their source content with `translation_last_reviewed` set (stubs removed).
   - [x] Repo root + `docs/source` stub translations now mirror canonical sources with `translation_last_reviewed` set (stubs cleared).
 - [x] Complete translations for core policy/docs (CODE_OF_CONDUCT, PATENTS, README, AGENTS, status/roadmap, configuration/runbooks).
 - [x] Prioritize portal docs: `docs/portal/i18n/*/docusaurus-plugin-content-docs` (especially SoraFS/SoraNet) and mark completion per locale (stubs mirrored and marked complete across locales).
 - [x] Portal SoraFS quickstart translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS capacity simulation runbook translations completed across all locales in `docs/portal/docs`, `docs/portal/i18n`, and `docs/source/sorafs/runbooks`.
 - [x] Portal SoraFS runbooks index translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS manifest pipeline translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS orchestrator ops runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS orchestrator tuning guide translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS multi-source rollout runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS orchestrator configuration guide translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.

25. **SUBSCRIPTION-TRIGGER-API — Trigger-based subscriptions for arrears + fixed billing** (Core/DataModel/Contracts/Torii, Line: Shared, Owner: App Platform WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - Design: Plans are `AssetDefinition` entries with `billing`/`pricing` metadata; subscriptions are `Nft`s owned by the subscriber; billing uses time triggers with deterministic UTC calendar-month scheduling and `bill_for` set to `previous_period` (arrears) or `next_period` (advance); usage plans accumulate counters in subscription metadata; fixed plans charge a constant amount; optional invoice metadata is recorded on the subscription NFT for the latest attempt (apps can mint invoice NFTs for full history).
 - [x] Specify the canonical metadata schema for plan/subscription/invoice objects (Norito JSON), including usage accumulator keys and deterministic UTC calendar helpers; document in `docs/source/subscriptions_api.md`, update `docs/source/data_model_and_isi_spec.md`, and add typed schema in `crates/iroha_data_model/src/subscription.rs`.
 - [x] Implement deterministic calendar-period helper (UTC month boundaries, anchor day/time, arrears/advance) shared by contracts and tests.【crates/iroha_primitives/src/calendar.rs:1】
 - [x] Add an IVM billing syscall/contract wrapper that computes the bill, transfers assets, updates subscription state, writes the latest `subscription_invoice`, and reschedules or retries on failure while enforcing `max_failures`/`grace_ms`.
 - [x] Add a usage recorder syscall path (by-call trigger or contract) to increment subscription usage counters; gate with `CanExecuteTrigger` permissions.
- [x] Add Torii endpoints for plans/subscriptions (create/list/get/pause/resume/cancel/charge-now) and expose subscription listing queries.
 - [x] Add CLI/SDK helpers for plan publishing, subscribe/cancel, usage reporting, and subscription management.
 - [x] Tests: unit coverage for calendar math + subscription billing/usage syscalls.
 - [x] Tests: integration coverage with 4 peers for arrears usage billing, fixed-price advance billing, and retry/grace failure paths (`integration_tests/tests/subscriptions.rs`).
 - [x] Telemetry/events: counters for billing attempts/outcomes and TriggerCompleted filters for subscription billing; update docs/runbooks.

26. **IVM-QUERY-SYSCALLS — Norito query syscall with gas metering** (Core/IVM/Query, Line: Shared, Owner: VM WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - Design: `SMARTCONTRACT_EXECUTE_QUERY` accepts `&NoritoBytes(QueryRequest)` and returns `&NoritoBytes(QueryResponse)` executed under the contract authority; iterable queries run in ephemeral cursor mode so results do not outlive the VM run.
- [x] Implement CoreHost query syscall, deterministic gas model (base + per-item + per-byte, sort multiplier), and unit coverage; update syscall spec/docs for Norito query requests.
- [x] Abort query execution early when the remaining gas budget is exhausted (meter during materialization) to avoid long scans.
- [x] Add `pipeline.query_max_fetch_size` for IVM query validation and document the knob alongside syscall semantics.
- [x] Reject responses that exceed per-byte gas budgets before encoding when exact Norito sizing is available, and add targeted coverage.
 - [x] Add a Kotodama `execute_query` builtin that bridges SMARTCONTRACT_EXECUTE_QUERY for Norito-encoded queries.

25. **NEXUS-STORAGE-BUDGETS — Enforce global storage budgets + DA offload** (Storage/Core, Line: Iroha 3, Owner: Nexus Core WG, Priority: High, Status: 🈴 Completed, target TBD)
- [x] Add Nexus storage budget configuration with per-component weight splits, clamp Kura/WSV cold tier/SoraFS/SoraNet spool caps, and wire defaults into the Nexus profile config.
- [x] Account for merge-ledger + sidecar + roster journal bytes (including queued blocks + retired lanes) in Kura budget enforcement.
- [x] Surface operator-facing warnings/telemetry when caps trigger.
- [x] Implement live hot-tier eviction using measured WSV memory usage (not snapshot size estimates).
- [x] Wire dedicated SoraVPN spool caps for Iroha 3 (budgets now map to `streaming.soravpn`).
- [x] Wire DA-backed cold/WSV retrieval for Iroha 3 while keeping Iroha 2 full-replica behavior.
 - [x] Add operator guidance + metrics for sizing `nexus.storage` budgets and monitoring cap pressure.
 - [x] Portal SoraFS node operations runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS node storage design translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS node implementation plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS provider admission policy translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS multi-source provider advert translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS reserve ledger digest translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS docs stubs cleared across remaining locales in `docs/portal/docs/sorafs/**` and `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/sorafs/**`.
 - [x] Portal SoraFS migration ledger translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS migration roadmap translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS pin registry implementation plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS pin registry operations runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS pin registry validation plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.

25. **RUST-1-92-ROLLOUT — Adopt Rust 1.92 lints and APIs** (Tooling/Core, Line: Shared, Owner: Release Eng, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Run `ci/check_rust_1_92_lints.sh` and fix any new deny-by-default findings (never-type fallback, macro-export arguments).
 - [x] Sweep std `RwLock` write-then-read handoffs and use `RwLockWriteGuard::downgrade` where it preserves semantics.
 - [x] Audit const helpers for rotate usage now that `slice::rotate_left`/`rotate_right` are const-stable.
 - [x] Track additional Rust 1.92 API adoption in `docs/source/rust_1_92_adoption.md`.
 - [x] Portal SoraFS portal publish plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS signing ceremony translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS storage capacity marketplace translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS provider advert rollout translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS priority snapshot (2025-03) translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Taikai monitoring dashboards translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS SF-6 security review translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS SF1 determinism dry-run translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS Orchestrator GA parity report translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS SF-2c capacity accrual soak report translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal AI moderation calibration report (2026-02) translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS capacity marketplace validation report translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal GAR jurisdictional review (SNNet-9) translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet PQ primitives overview translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet constant-rate profiles translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet transport overview translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet testnet rollout (SNNet-10) translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet PQ ratchet runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet PQ rollout plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet puzzle service operations guide translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraNet docs stubs cleared across all locales in `docs/portal/docs/soranet/**` and synced to `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/soranet/**`.
 - [x] Portal SoraNet privacy metrics pipeline translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal intro page translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito Streaming roadmap translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal reference index translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito codec reference translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito overview translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito getting started translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito quickstart translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito streaming translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito ledger walkthrough translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito examples index translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito Hajimari entrypoint example translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito call-transfer-asset example translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito register-and-mint example translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito transfer-asset example translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito NFT flow example translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito Try-It console translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Norito examples stubs cleared across all locales in `docs/portal/docs/norito/examples/**` and synced to `docs/portal/i18n/*/docusaurus-plugin-content-docs/current/norito/examples/**`.
 - [x] Portal Sora Nexus overview translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus operations runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus settlement FAQ translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus default lane quickstart translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus elastic lane provisioning translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus technical specification translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus fee model updates translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus telemetry remediation plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal confidential gas calibration ledger translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus routed-trace audit report (2026 Q1) translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus data-space operator onboarding translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus bootstrap & observability plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus ledger refactor plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus transition notes translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus lane model translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Sora Nexus confidential assets & ZK transfers translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal account address compliance reference translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal publishing checklist translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal address safety & accessibility reference translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS address checksum incident runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS registry schema translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS local-to-global address toolkit translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS metrics & onboarding kit translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Connect session preview runbook translations completed across all locales in `docs/runbooks`.
 - [x] Connect chaos & fault rehearsal plan translations completed across all locales in `docs/runbooks`.
 - [x] SNS training templates (slides, workbook, evaluation, invite email) translations completed across all locales in `docs/examples`.
 - [x] Docs preview templates (invite, request, feedback digest/form) translations completed across all locales in `docs/examples`.
 - [x] DA manifest review template translations completed across all locales in `docs/examples`.
 - [x] Taikai anchor lineage packet translations completed across all locales in `docs/examples`.
 - [x] Pen-test remediation report template translations completed across all locales in `docs/examples`.
 - [x] SoraFS release notes translations completed across all locales in `docs/examples`.
 - [x] SoraFS CI cookbook translations completed across all locales in `docs/examples`.
 - [x] Android device lab reservation request template translations completed across all locales in `docs/examples`.
 - [x] Android partner SLA discovery notes translations completed across all locales in `docs/examples`.
 - [x] SNS arbitration transparency report template translations completed across all locales in `docs/examples`.
 - [x] Nexus steering priority snapshot (2025-03 wave) translations completed across all locales in `docs/examples`.
 - [x] SoraNet GAR intake template translations completed across all locales in `docs/examples`.
 - [x] SoraNet testnet operator kit templates (readme, checklist, telemetry, incident playbook, verification report) translations completed across all locales in `docs/examples`.
 - [x] SoraNet testnet stage-gate report template and sample report translations completed across all locales in `docs/examples`.
 - [x] SoraFS CI sample fixtures README and SoraFS capacity simulation toolkit README translations completed across all locales in `docs/examples`.
 - [x] SoraGlobal gateway billing reconciliation report template translations completed across all locales in `docs/examples`.
 - [x] SoraNet relay incentive parliament packet translations (README, economic analysis, rollback plan) completed across all locales in `docs/examples`.
 - [x] Finance repo custodian ack and governance packet templates translations completed across all locales in `docs/examples`.
 - [x] Ministry volunteer brief example moderation stub cleared in `docs/examples`.
 - [x] Nexus overview translations completed across all locales in `docs/source`.
 - [x] Nexus transition notes translations completed across all locales in `docs/source`.
 - [x] Nexus lane model translations completed across all locales in `docs/source`.
 - [x] Nexus technical design spec translations completed across all locales in `docs/source`.
 - [x] Nexus lane compliance policy engine translations completed across all locales in `docs/source`.
 - [x] Nexus fee model translations completed across all locales in `docs/source`.
 - [x] Nexus operations runbook translations completed across all locales in `docs/source`.
 - [x] Nexus public lane staking translations completed across all locales in `docs/source`.
 - [x] Nexus cross-lane commitments translations completed across all locales in `docs/source`.
 - [x] Nexus telemetry remediation plan translations completed across all locales in `docs/source`.
 - [x] Nexus routed-trace audit report (2026 Q1) translations completed across all locales in `docs/source`.
 - [x] Nexus ledger refactor plan translations completed across all locales in `docs/source`.
 - [x] Nexus SDK quickstarts translations completed across all locales in `docs/source`.
 - [x] Nexus settlement FAQ translations completed across all locales in `docs/source`.
 - [x] Nexus elastic lane provisioning translations completed across all locales in `docs/source`.
 - [x] Nexus privacy commitments translations completed across all locales in `docs/source`.
 - [x] Testing and troubleshooting guide translations completed across all locales in `docs/source`.
 - [x] Account address compliance status translations completed across all locales in `docs/source`.
 - [x] Release artifact selection translations completed across all locales in `docs/source`.
 - [x] Docker builder image translations completed across all locales in `docs/source`.
 - [x] Bridge proofs translations completed across all locales in `docs/source`.
 - [x] Bridge finality proofs translations completed across all locales in `docs/source`.
 - [x] Runtime upgrades translations completed across all locales in `docs/source`.
 - [x] Sumeragi NPoS task breakdown translations completed across all locales in `docs/source`.
 - [x] Sumeragi evidence audit API translations completed across all locales in `docs/source`.
 - [x] Sumeragi randomness evidence runbook translations completed across all locales in `docs/source`.
 - [x] Sumeragi aggregators translations completed across all locales in `docs/source`.
 - [x] Sumeragi pacemaker translations completed across all locales in `docs/source`.
 - [x] Portal Torii app API parity audit translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal GAR operator onboarding brief translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS dispute & revocation runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS capacity reconciliation runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS gateway & DNS kickoff runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Taikai anchor observability runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS operations playbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS observability plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS node-client protocol translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS staging manifest playbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS chunker registry rollout checklist translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS chunker registry translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS chunker registry charter translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS chunker conformance translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS chunker profile authoring translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS deal engine translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS developer index translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS developer CI recipes translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS developer CLI cookbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS deployment notes translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS deployment guide translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS KPI dashboard translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS address display guidelines translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS governance playbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS suffix catalog translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS registrar API translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS payment settlement plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SNS bulk onboarding toolkit translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal DA replication policy translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal DA ingest plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal DA commitments plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal DA threat model translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal Nexus SDK quickstarts translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal governance API endpoint translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS release process translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS direct-mode fallback pack translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS SDK index translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS Rust SDK snippets translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] TODO: Continue SoraFS/SoraNet portal translations beyond the completed SoraFS docs set (quickstart, runbooks, pipeline/orchestrator guides, node plan/storage, node operations, provider admission policy, multi-source provider advert, provider advert rollout, priority snapshot 2025-03, taikai monitoring dashboards, sf6 security review, sf1 determinism dry-run, orchestrator GA parity report, sf2c capacity soak report, ai moderation calibration report 2026-02, capacity marketplace validation report, GAR jurisdictional review, GAR operator onboarding, pq primitives, pq ratchet runbook, pq rollout plan, puzzle service operations guide, privacy metrics pipeline, constant-rate profiles, transport overview, testnet rollout, reserve ledger digest, migration ledger, migration roadmap, pin registry plan, pin registry ops, pin registry validation plan, portal publish plan, signing ceremony, storage capacity marketplace, chunker docs, developer docs, SDK docs), then fill remaining portal stubs per locale (mirrored canonical sources for first release coverage).
 - [x] Ensure Akkadian translations are semantic and written in cuneiform (no transliteration).
 - [x] Extend CLI i18n coverage (remaining messages and clap help output).
 - [x] Fill governance schedule placeholders in scripts/templates that currently say TBD.
 - [x] Add checks to prevent stub-only translations from shipping (CI guard + `scripts/sync_docs_i18n.py --dry-run` gate).

35. **SEC-TELEMETRY-REDACTION — Enforce redaction + tamper-evident telemetry** (Observability/Security, Line: Shared, Owner: Observability WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Define sensitive-field taxonomy and expected redaction behavior (keywords + explicit annotations).
 - [x] Make redaction mandatory for operator/release telemetry profiles (config default + build guard).
 - [x] Add CI/static checks to prevent new unredacted sensitive fields and update allowlist policy.
 - [x] Implement tamper-evident telemetry export (hash chaining + optional signing key rotation).
 - [x] Emit redaction/audit metrics for skipped or truncated fields.
 - [x] Add unit/integration tests + update `docs/source/telemetry.md` and threat model notes.

36. **SEC-TIME-HARDENING — Enforce NTS/time bounds** (Runtime/Ops, Line: Shared, Owner: Runtime WG + Ops, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Add config for min samples, max offset, max confidence, and enforcement mode (warn/reject).
 - [x] Extend network time status with sample counts and health flags.
 - [x] Gate time-sensitive validations on NTS health (offline receipts, attestations, governance windows).
 - [x] Emit telemetry/status for drift, unhealthy states, and fallback usage.
 - [x] Add alerting guidance + runbook updates for NTS drift/unsynced states.
 - [x] Add tests for insufficient samples, out-of-bounds offsets, and enforcement behavior.

37. **SEC-UPGRADE-PROVENANCE — Require SBOM/SLSA attestations for runtime upgrades** (Core/Governance/Security, Line: Shared, Owner: Security WG + Governance WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Extend runtime-upgrade manifests with provenance payloads (SBOM digests, SLSA attestation bytes, signer metadata).
 - [x] Add config-driven enforcement for provenance policy and trusted signers/thresholds.
 - [x] Enforce provenance in runtime-upgrade ISIs; reject missing or invalid attestations.
 - [x] Surface telemetry + error codes for provenance failures.
 - [x] Wire Torii/governance APIs to accept provenance and emit rejection telemetry.
 - [x] Add tests + update `docs/source/runtime_upgrades.md` and threat model tracking.

38. **SEC-TORII-OPERATOR-AUTH — WebAuthn/mTLS hardening for operator endpoints** (Torii/Security, Line: Shared, Owner: Torii WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Define operator auth policy in `iroha_config` (WebAuthn required, mTLS gating, token fallback rules).
 - [x] Implement WebAuthn credential enrollment + storage (WSV or config-backed).
 - [x] Add challenge/verify flow and gate operator endpoints behind auth policy.
 - [x] Add rate limits and audit telemetry for auth failures/lockouts.
 - [x] Add tests for enrollment, rollover, and fallback paths; update OpenAPI + runbooks.

39. **SEC-ATTACHMENT-SANITISATION — Safe ingest pipeline for Torii attachments** (Torii/Runtime, Line: Shared, Owner: Runtime WG + Torii WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Add config for allowed MIME types, max expanded size, and archive depth.
 - [x] Implement magic-byte sniffing + strict allowlist enforcement before persistence.
 - [x] Enforce deterministic decompression/expansion limits and reject unknown formats.
 - [x] Sandbox risky parsers with strict CPU/memory limits (bounded worker).
 - [x] Add subprocess sanitizer mode (`torii.attachments_sanitizer_mode`), OS-level rlimits, and re‑sanitize exports on download.
 - [x] Store provenance metadata (sniffed type, hashes, sanitizer verdict) alongside attachment records.
 - [x] Add telemetry counters + malicious fixture/subprocess tests; update `docs/source/security_hardening_requirements.md`.

40. **SEC-MEMBERSHIP-MISMATCH — Detect and alert on roster drift** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Propagate membership view-hash in consensus gossip/status payloads.
 - [x] Compare peer hash vs local roster and record mismatch counters + active gauges.
 - [x] Add config for alert thresholds and optional fail-closed behavior.
 - [x] Clear mismatch on alignment and expose last-mismatch context via `/v1/sumeragi/status`.
 - [x] Add tests for mismatch detection + update Sumeragi runbook with remediation steps.

26. **JDG-SIGNATURE-SCHEMES — Expand jurisdiction signature support** (Core/Governance, Line: Shared, Owner: Governance WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Define policy surface for additional JDG signature schemes and encode it in config/data model.
 - [x] Implement verification for new schemes and add alignment tests.
 - [x] Update governance/jurisdiction docs with scheme support and limits.

29. **KOTODAMA-TERNARY-LOWERING — Implement ternary IR lowering** (IVM, Line: Shared, Owner: IVM WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Implement IR lowering for ternary expressions (beyond parse/typecheck).
 - [x] Add compiler regression tests and update Kotodama docs/examples.

30. **IVM-CONFORMANCE-SUITE — Fill TBD conformance tests** (IVM, Line: Shared, Owner: IVM WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Implement the conformance test files listed as TBD in `crates/ivm/docs/conformance.md` (decoder roundtrip, op semantics, gas golden, crypto vectors, zk trace).
 - [x] Wire CI coverage for the new test suite and update docs.

31. **MERGE-COMMITTEE-SIGNATURES — Wire merge-committee signatures** (Consensus/Core, Line: Shared, Owner: Consensus WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Define merge quorum pipeline requirements and signature payloads.
 - [x] Implement merge-committee signature handling in state/consensus.
 - [x] Add tests and update consensus/governance docs.

32. **CODE-HEALTH-REFACTOR — Remove remaining refactor TODOs** (Core/Tooling, Line: Shared, Owner: Core Protocol WG, Priority: Low, Status: 🈴 Completed, target TBD)
 - [x] Split large helpers flagged in `irohad/src/main.rs` and `iroha/src/client.rs`.
 - [x] Decompose Sumeragi loops (`main_loop.rs`, `mode.rs`, `validation.rs`, `pending_block.rs`, `qc.rs`, `reschedule.rs`, `votes.rs`) and `block_sync.rs` validation flow.
 - [x] Refactor `account_admission.rs`, `settlement.rs` error payloads, and multisig ownership/borrowing to remove lint suppressions.【crates/iroha_core/src/smartcontracts/isi/account_admission.rs:1】【crates/iroha_core/src/smartcontracts/isi/settlement.rs:1】【crates/iroha_core/src/smartcontracts/isi/multisig.rs:1】
 - [x] Consolidate boolean policy knobs in `state.rs` into enums/bitflags and split large validation helpers.

34. **EVENTS-PIPELINE-HANG — Unblock mod.rs integration event tests** (QA/Consensus, Line: Shared, Owner: QA WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Reproduce `cargo test -p integration_tests --test mod` hangs and confirm DA-enabled single-peer networks stall on post-genesis commits.
 - [x] Enforce a 4-peer minimum in the integration-test harness and update direct peer-start paths to avoid single-peer stalls.
 - [x] Allow pending-block quorum reschedules to drop/requeue after a retry even with partial precommit votes; add unit test coverage.
 - [x] Re-run the full `cargo test -p integration_tests --test mod` suite to confirm stability.

41. **NEXUS-STORAGE-DA-RETENTION — DA-backed storage eviction under disk caps** (Core/DA/WSV, Line: Iroha 3, Owner: Nexus Core WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Define deterministic eviction order across Kura block bodies, tiered-state cold snapshots, and storage spools once `nexus.storage.max_disk_usage_bytes` is reached (no hardware-dependent heuristics).
 - [x] Implement DA-backed rehydration for evicted blocks/WSV cold shards with cache hit/miss + churn telemetry.
 - [x] Add 4-peer DA integration tests covering eviction + rehydrate with strict disk caps.

43. **IZANAMI-LONGRUN-STABILITY — Validate long-run multi-peer execution-root QC flow** (Consensus/Tooling, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈺 In Progress, target TBD)
- [ ] Run Izanami stable profile on 4+ peers for 2000+ blocks (no faults) and confirm commit QCs aggregate with matching execution roots (2026-01-23 run: `RUST_LOG=izanami::summary=info,izanami::workload=warn IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` failed with no block height progress for 300s at min height 98; workload submissions hit `transaction.status_timeout_ms=600s`, peer logs show repeated `missing_qc` view changes and "no proposal observed" errors; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_eMeaCB`; no `execution roots`/QC split or deadlock warnings in peer logs. 2026-01-24 run: same command failed with no block height progress for 300s at min height 12; plan submissions timed out (`transaction.status_timeout_ms=600s`) plus `transaction queued for too long`, duplicate metric registration warnings persisted; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_XT3mKF`. 2026-01-24 run: `RUST_LOG=iroha_p2p=debug,iroha_core=debug IROHA_TEST_NETWORK_KEEP_DIRS=1 IROHA_TEST_NETWORK_PERMIT_DIR=$(mktemp -d) cargo run -p izanami --release --locked -- --allow-net --nexus --peers 4 --faulty 0 --duration 3600s --target-blocks 2000 --progress-interval 10s --progress-timeout 300s --tps 5 --max-inflight 8 --workload-profile stable` failed with no block height progress for 300s at min height 36; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_KrCc5b`. 2026-01-24 run: same command ran full 3600s but stopped before target blocks (concise_cardinal committed height 307); logs show block sync frames + RBC chunk ingress with missing BlockCreated requests while awaiting INIT and later INIT rebroadcasts; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_kDdEkr`. 2026-01-24 run: same command (tool timed out after ~65m; no Izanami summary) shows committed height 319 across peers and 1888 txs in lane_000_core; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_IElRk7`. 2026-01-25 ramp runs (900s, info logs) at tps 2/3/5 reached heights 94/86/79 with 505/483/467 txs in lane_000_core; view-change/missing_qc low and consensus backlog cleared, but throughput stayed ~0.5 tps; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_ydcz68`, `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_NGrVys`, `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_RvgeXq`. 2026-01-25 run: tps 5 with `--max-inflight 64` (900s, info logs) timed out in harness and reached height 28-33 with 739 txs in lane_000_core; view-change/missing_qc/no-proposal/stake-quorum timeouts resurfaced; logs in `/var/folders/n2/xxntlr312qbfdnp0j1xp52hw0000gn/T/irohad_test_network_lkyzd2`).
- [x] Hydrate INIT-created RBC sessions from pending payloads so missing-chunk stalls clear after view changes; unit coverage added.
- [x] Izanami: raise pipeline parallelism, enable signature batching, and increase validation worker throughput defaults for local validation/execution; add config coverage.
 - [ ] Investigate and resolve any execution-root divergence or consensus stalls surfaced during the run.

## Archived

33. **TRYBUILD-REL-PATHS — Normalize trybuild manifest paths** (Tooling/Tests, Line: Shared, Owner: Tooling WG, Priority: Low, Status: 🈴 Completed, target TBD)
 - [x] Rewrite trybuild-generated `Cargo.toml` path entries in `norito_derive` UI tests to be relative so target-codex artifacts avoid developer-specific absolute paths; status.md updated.

15. **EMPTY-BLOCK-HEARTBEAT — Restore empty-block rejection with heartbeat tx** (Consensus/Core, Line: Shared, Owner: Consensus WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Implement a lightweight heartbeat transaction or alternative liveness mechanism.
 - [x] Reinstate empty-block rejection and add regressions for liveness.

16. **TORII-OPENAPI-FULL — Generate complete OpenAPI spec** (Torii/Docs, Line: Shared, Owner: Docs WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Replace the stub spec with full endpoint coverage and schema sections.
 - [x] Add tests validating expected paths/schemas; update docs.

22. **DEP-STUBS-REMOVAL — Remove stubbed dependency patches** (Build/Tooling, Line: Shared, Owner: Tooling WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Replace `rust_decimal_stub` and `vergen_git2_stub` patches with real crates or full local implementations.
 - [x] Validate builds/tests and update CI/tooling docs as needed.

5. **NORITO-AUDIT-FIXES — Lock v1 layout, header framing, and canonical JSON** (Serialization/Norito, Line: Shared, Owner: Data Model WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] V1 layout policy: set `V1_LAYOUT_FLAGS`/`default_encode_flags()` to `0x00`, require `VERSION_MINOR` match, and accept explicit header flags within the supported mask (no heuristic flag guessing); document the v1 table in `norito.md`.
 - [x] Header-only decode: remove heuristic flag guessing and headerless block decoding; require framed `SignedBlockWire` and add negative regressions.
 - [x] Feature defaults: keep layout-affecting feature toggles from changing on-wire v1 semantics.
 - [x] Packed-struct derive semantics: align self-delimiting classification and add coverage for packed struct headers/sizes.
 - [x] Streaming length caps + guard precedence: enforce `max_archive_len` in streaming readers, ensure header flags override any `DecodeFlagsGuard`, and add regressions.
 - [x] Canonical JSON floats + strict-safe policy: non-finite -> `null`, stable float formatting, `guarded_try_deserialize` honors `strict-safe` with tests.
 - [x] NSC streaming scope + docs: clarify `norito::streaming` as data-only and update portal/spec docs; translations flagged for update.

4. **DOCS-ALIGNMENT-GUARDRAILS — Keep documentation synced with code and portal paths** (Docs/DevRel, Line: Shared, Owner: Docs WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Confirmed Torii address guidance (`torii.address` in config) across translations and portal docs so env shims stay retired.
 - [x] Verified SoraFS storage docs align with config + `SORAFS_STORAGE_*` dev/test notes across portal/localized copies.
 - [x] Replaced lingering `serde_json` usage in docs with Norito JSON helpers.
 - [x] Extended `scripts/check_md_links.py` for Docusaurus heading IDs, relative doc-id resolution, README/index directory links, and docs-bundle allowlisting; added unit coverage.
 - [x] Re-ran link scans across docs/portal bundles (including artifacts previews) to confirm local links resolve.

2. **MOCHI-BINARY-COMPAT-CHECKS — Guard profile/binary mismatches and bootstrap builds** (Tooling/Supervisor, Line: Shared, Owner: Tooling WG, Priority: Medium, Status: 🈴 Completed)
 - [x] Binary discovery: record paths from env/CLI/config and check `irohad`/`kagami`/`iroha_cli` availability + version/build-line (iroha2 vs iroha3, DA/RBC support) before launch; block on mismatches with actionable errors.
 - [x] Genesis verification: run `kagami verify --profile …` in prepare, parse fingerprint/chain id/VRF status, and surface results in UI logs; refuse launch when verify fails.
 - [x] Auto-build: optional `--build-binaries` path that runs `cargo build -p irohad -p iroha_cli -p kagami`, caches success, and short-circuits future runs; surface failures in UI.
 - [x] Telemetry/UX: emit an alignment summary in the UI (detected profile, binary versions, build-line, verify status, hash); add help text for overrides.
 - [x] Tests/docs: regressions for missing/old binaries, profile mismatch, auto-build success/failure; document alignment checks and escape hatches.

3. **MOCHI-READINESS-SMOKE — Gate readiness on real transaction/block flow** (Tooling/Supervisor, Line: Shared, Owner: Tooling WG, Priority: High, Status: 🈴 Completed)
 - [x] Implement a post-boot smoke: submit a signed tx, wait for commit via block/event streams, and gate readiness on success with retries/backoff and bounded timeouts.
 - [x] Snapshot/restore validation: check Kura/genesis hash before restore, refuse mismatches, and re-run the smoke after restore (with status surfaced).
 - [x] Status/UX: expose per-peer readiness, last smoke result/timestamp/error in UI and status API; add CLI flag to disable smoke for debugging.
 - [x] Tests: MockTorii-driven happy/timeout/fail cases plus snapshot/restore smoke rerun coverage.

1. **SUM-KURA-ATOMIC-COMMIT — Keep WSV/Kura updates atomic on commit** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Commit staging design: define a staging buffer for WSV mutations (tx index, DA shard/receipt cursors, pin intents, AXT policy install, merge metadata) that applies after Kura success; keep exec witness handling aligned and avoid double-emitting events.
 - [x] Pipeline refactor: split `finalize_pending_block` into Kura-first persistence and a post-Kura stage apply; ensure staged data is discarded on failure and not inserted into live WSV until persistence succeeds.
 - [x] Retry/abort hygiene: retries rebuild or reuse staging without replaying side effects; abort path clears staging, pending entries, QC caches, RBC sessions, and requeue lists without advancing cursors/indexes and without losing the ability to fetch/rehydrate.
 - [x] Tests: unit + integration for Kura failure (retry + abort) proving no WSV drift, no cursor/index advancement before Kura success, requeued txs don’t double-apply, and recovered state matches Kura; happy-path regression to ensure telemetry/events still fire after the refactor.
 - [x] Telemetry/docs: add “Kura staged/rollback” counters + last-error/height fields; document sequencing and operator remediation in Sumeragi runbook/config notes.

2. **SUM-KURA-ABORT-LIVENESS — Release locks and gates after persistence aborts** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Lock reset policy: on Kura abort that drops pending/RBC state, choose a deterministic rollback target for `locked_qc`/`highest_qc` (committed ancestor or highest cached QC) and/or force a view change with evidence so proposals can resume.
 - [x] Cache/pending cleanup: when QC/RBC caches are purged, also clear or reschedule pending entries and emit a fetch/rebroadcast plan for a fresh tip; ensure DA/exec gates don’t reopen on stale pending blocks.
 - [x] Liveness/tests: simulate Kura abort with live lock/QC caches; assert locks reset or view change triggers, no commits happen without payload, and the node accepts/rebuilds a replacement block (DA on/off, with/without QC caches).
 - [x] Telemetry/status: add “lock reset after Kura abort” counters and last {height, view, reason}; document operator steps for repeated abort-driven resets and expected status snapshots after a reset.

1. **IMPLICIT-ACCOUNTS — Domain-scoped implicit account receive** (Core/Executor/Data Model/SDK, Line: Shared, Owner: Core Protocol WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Finalize on-chain `AccountAdmissionPolicy` struct + deterministic wire format (domain metadata key `iroha:account_admission_policy`, chain-parameter fallback `iroha:default_account_admission_policy`).
 - [x] Enforce optional per-tx/per-block implicit-creation caps (domain policy).
 - [x] Extend executor permission model to support self-scope baseline rights for newly implicit accounts (or domain-configurable `default_role_on_create`), with deterministic deny rules.
 - [x] Core ISI semantics: update `Mint<Asset>` / `Transfer<Asset>` / `Transfer<Nft>` to `ensure_receiving_account` and auto-create accounts in `ImplicitReceive` domains; preserve explicit-only failure modes.
 - [x] Emit typed admission errors (`ImplicitAccountCreationDisabled`, `QuotaExceeded`, `AlgorithmNotAllowed`) and guarantee event ordering (`AccountCreated` before receipt events).
 - [x] Tests: add integration tests for open vs restricted domains, multi-receipt in one tx, cap enforcement, and determinism/event ordering (unit tests exist).
 - [x] SDK/CLI UX: remove destination-prevalidation requiring account existence; add helper to “register-if-missing”/atomic send for open domains; surface policy errors distinctly.
 - [x] Docs/migration: update `docs/source/data_model_and_isi_spec.md`, `rbac.md`, and CLI/SDK quickstarts; add upgrade checklist and sample policy blobs.

1. **DA-SHARD-CURSOR-MAPPING — Map lanes to WSV/Kura shards with durable cursors** (Nexus/Core, Line: Iroha 3, Owner: Core Protocol WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - Mapping: DONE — lane metadata `da_shard_id` (defaulting to `lane_id`) now drives `(lane, epoch, sequence) -> shard ID` mapping, resharded/unknown lanes are dropped on load, and Sumeragi persists per-lane cursors into `da-shard-cursors.norito` beside the DA spool.
 - Admission: DONE — block-and-revert paths rewind DA commitment/receipt/shard indexes to the pre-tip height before validation, rebuild the cursor journal from Kura when checkpoints regress, and keep touched-lane gating bound to the rewound shard cursors so soft forks require fresh DA commitments.
 - Recovery: enforce DA cursors during catch-up; telemetry for lagging/missing shard commitments; operator runbook for remediation (including Byzantine reshard attempts). Telemetry/runbook: DONE — `da_shard_cursor_lag_blocks{lane,shard}` now reports cursor gaps (missing/stale/unknown lanes set lag to the required height/delta; healthy advances reset to zero) and the DA ingest plan documents operator remediation for non-zero lags.【crates/iroha_core/src/state.rs】【crates/iroha_telemetry/src/metrics.rs】【docs/source/da/ingest_plan.md】
 - Tests: DONE — regressions cover resharding, DA journal persistence, Kura replay truncation/rewind, and soft-fork admission rejecting touched lanes without a matching shard cursor at the rewritten height.【crates/iroha_core/src/state.rs】

2. **SUM-BLOCK-SYNC-ROSTER-PERSISTENCE — Certified roster persistence and fetch** (Consensus/Sumeragi/Core, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - Kura persistence: DONE — commit certificates and validator checkpoints are journaled to `commit-rosters.norito` beside the block store and preloaded into Sumeragi status caches on startup so block sync can reuse persisted rosters. Retention/GC is configurable (`kura.roster_sidecar_retention`) and Kura now prunes roster sidecars to the configured window.
 - Roster fetch path: block sync now attaches commit certificates/checkpoints from persisted sidecars or the commit-roster journal for `(height, hash)` and validates them before caching, keeping uncertified hints out of the pipeline.
 - Control-flow hardening: uncertified rosters are dropped, `handle_block_sync_update` skips QC handling on payload failure, and roster selection prefers persisted snapshots over hints/history.
 - Integration/tests: coverage exercises journal fallbacks and missing-sidecar cases so long-range catch-up reuses persisted rosters without relying on raw roster hints.

3. **SUM-PENDING-BLOCK-FLOW — Stop view changes from marooning payloads** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Leader replay: view changes and retry exits rebroadcast the highest-QC `BlockCreated` payload to the current topology with per-(height,view) dedupe so pending blocks hydrate without roster hints.【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Pending-block metadata: `FetchPendingBlock` replies return `BlockCreated` for pending blocks and `BlockSyncUpdate` for committed blocks; block sync ignores raw roster hints in favour of certified commit certificates/checkpoints from durable snapshots.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/commit_roster_journal.rs】
 - [x] View-change visibility: view-change causes (commit failure/quorum timeout/DA availability/missing payload or QC/validation reject) export labeled counters plus last-cause/timestamp through status/telemetry for operator alerts (DA availability is advisory).【crates/iroha_core/src/sumeragi/status.rs】【crates/iroha_core/src/telemetry.rs】
 - [x] Localnet tuning: pacemaker backoff is capped at 10s and 1s block-time defaults keep DA-off localnets responsive; the NPoS pacemaker latency smoke holds 4-node nets under ~1.5s spacing even with 250 ms RTT.【crates/iroha_config/src/parameters/defaults.rs】【integration_tests/tests/sumeragi_npos_pacemaker_latency.rs】
 - [x] Tests: roster selection/fetch regressions cover commit-roster journal and sidecar recovery, and pacemaker/view-change telemetry tests pin the counters and rebroadcast path so missing payloads are repaired deterministically.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/status.rs】

4. **SUM-VOTE-PREVALIDATION — Gate votes on stateless+stateful checks** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Pre-vote guard (code): votes are gated until stateless+stateful validation succeeds; pending blocks track validation status, mark invalid payloads aborted, requeue transactions, and clean proposal/RBC caches before proceeding.【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Telemetry/docs: validation rejects record labeled counters (`sumeragi_validation_reject_total{reason=*}`) with last height/view/block/timestamp surfaced via `/v1/sumeragi/status`; view-change cause telemetry includes a `validation_reject` bucket and the Sumeragi runbook documents alert guidance.【crates/iroha_core/src/sumeragi/status.rs】【crates/iroha_core/src/telemetry.rs】【crates/iroha_data_model/src/block/consensus.rs】【docs/source/sumeragi.md】
 - [x] Evidence/view-change: invalid proposals emit evidence tied to the payload hash/parent QC and trigger view-change realignment without advancing Highest/Locked QCs; reasons are logged for operators.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/evidence.rs】
 - [x] Coverage: status/telemetry/unit tests lock reason buckets and last-reject snapshots, and invalid-proposal evidence validation covers height/view/parent mismatches to keep negative paths deterministic.【crates/iroha_core/src/sumeragi/status.rs】【crates/iroha_core/src/sumeragi/evidence.rs】

5. **SUM-PACEMAKER-LIVENESS-GAP — Keep quorum/DA timers running without inbound traffic** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
   - [x] Wire pacemaker tick -> commit path (both lines): `tick` now drives `process_commit_candidates` with an early return on empty pending sets and records status/telemetry counters (`commit_pipeline_tick_total`, `sumeragi_commit_pipeline_tick_total{mode,outcome}`) so timer-driven commits stay visible; DA-off/permissioned and DA-off NPoS smoke runs keep quorum-timeout liveness.
   - [x] DA availability rescheduler (deprecated; DA is advisory): `reschedule_stale_pending_blocks` reused the availability timeout plus DA abort hooks to reschedule `MissingLocalData` blocks, requeueing payloads, purging RBC state, bumping view, and tagging mode-labeled counters (`sumeragi_rbc_da_reschedule_by_mode_total{mode}`); consensus no longer reschedules on missing availability.
   - [x] Prevote-only recovery policy (both lines, mode-aware): prevote-quorum timeouts requeue payloads, rebroadcast `BlockCreated` + `PrevoteQC`, clear stale HighestQC, and trigger view change to avoid precommit starvation across permissioned and NPoS (DA on/off) runs.
   - [x] Observability/docs/tests: added unit coverage for prevote timeout detection and commit-pipeline tick status, telemetry label assertions for DA reschedules, and refreshed the Sumeragi runbook with the liveness path and operator signals (Iroha 2 vs Iroha 3 / DA-on vs DA-off).【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/telemetry.rs】【crates/iroha_telemetry/src/metrics.rs】【docs/source/sumeragi.md】

6. **MULTISIG-AUTH-HARDENING — Enforce controller semantics and eliminate derived-key bypass** (Core/Executor/SDK/Torii, Line: Shared, Owner: Security WG, Priority: High, Status: 🈴 Completed, target TBD)
 - Admission crash guard: (A) patch `tx::validate`/`SignedTransaction::verify_signature` to handle `AccountController::Multisig` without panic and emit stable reject codes **DONE**; (B) add unit/integration coverage for multisig authorities (accept/reject matrix) and Torii error/header plumbing **DONE**.
 - Policy verification path: (A) **DONE** — signature bundles carry multi-alg member signatures with signer ids; (B) **DONE** — deduped weight/quorum verification against `MultisigPolicy`; (C) **DONE** — signature-limit rejects now increment Torii telemetry with authority labels; (D) **DONE** — regressions for single-key alignment and signature-count overflow cases.【crates/iroha_data_model/src/transaction/signed.rs】【crates/iroha_core/src/tx.rs】
 - Controller id enforcement: (A) **DONE** — multisig registration requires an explicit controller id (JSON decoding rejects missing `account`, builder helpers randomise the controller in the signatory domain); (B) **DONE** — direct signing with multisig accounts is rejected in admission/execution with regression coverage.【crates/iroha_executor_data_model/src/isi.rs】【crates/iroha_core/src/smartcontracts/isi/multisig.rs】【crates/iroha_core/src/tx.rs】【docs/source/references/multisig_policy_schema.md】
 - TTL enforcement:
 - [x] Remove the “downward proposal” TTL bypass created by multisig-role propagation; relayer proposals now cap expiry per multisig policy to prevent longer-lived nested approvals.【crates/iroha_core/src/smartcontracts/isi/multisig.rs】【crates/iroha_executor/src/default/isi/multisig/transaction.rs】
 - [x] Enforce `transaction_ttl_ms` caps for proposers/relayers including nested multisig with regression coverage for nested relayer expiry.【crates/iroha_core/src/smartcontracts/isi/multisig.rs】
 - [x] Refresh CLI UX to surface the capped TTL behaviour and expiry expectations.
 - [x] Refresh SDK builders/fixtures (JS/Swift/Android) to preview and enforce the capped TTL before submission; JS now ships `buildProposeMultisigInstruction` with TTL-cap enforcement plus README/tests, and Swift/Android builders retain the preview/enforce scaffolding with refreshed fixtures.
 - SDK/CLI/docs alignment: **DONE** — Android/OkHttp transports surface Torii reject headers with regression coverage, SDK docs now call out `rejectCode()` handling alongside derived-key/TTL migration guidance, and status.md records the shipped guards.【java/iroha_android/android/src/test/java/org/hyperledger/iroha/android/client/HttpClientRejectCodeOkHttpTests.java】【java/iroha_android/README.md】【status.md】

7. **RBC-LIVENESS-RECOVERY — Diagnose and unblock reliable-broadcast stalls** (Consensus/Sumeragi/Core, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
   - [x] Instrumentation: RBC backlog and status snapshots now export READY/chunk deferral counters, pending-stash age/size/drops, retry/abort totals, and store-pressure gauges across `/v1/status`, `/v1/sumeragi/status`, and Prometheus so stalled deliveries can be triaged quickly.【crates/iroha_core/src/sumeragi/status.rs】【crates/iroha_telemetry/src/metrics.rs】
   - [x] Reschedule paths: pending blocks stuck on `MissingLocalData` are rescheduled off the availability timeout (requeue payloads, purge RBC state, bump view), and RBC handlers keep sessions bounded with unit coverage for deferral/eviction counters and stash limits.【crates/iroha_core/src/sumeragi/main_loop.rs】
   - [x] Adversarial coverage: chunk-drop/shuffle scenarios, cold-start recovery, and restart liveness suites hold blocks pending without committing until RBC delivery resumes and assert telemetry reflects the stalled sessions under packet loss/reorder/view changes.【integration_tests/tests/sumeragi_adversarial.rs】【integration_tests/tests/sumeragi_da.rs】【integration_tests/tests/sumeragi_npos_liveness.rs】
   - [x] Operational guidance: the Sumeragi runbook documents the RBC backlog/deferral/store-pressure signals, availability vote health, and DA reschedule counters used to remediate stalls.【docs/source/sumeragi.md】

8. **SUM-MODE-CUTOVER-LIVE — Runtime flip between permissioned and NPoS without restart** (Consensus/Sumeragi/Core, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - Flip trigger + safety: tick polls `effective_consensus_mode` vs staged `next_mode`/activation height, debounces repeat flips, and enforces the `mode_flip_enabled` kill switch with activation-lag surfacing in status/telemetry.
 - Runtime swap: live flips now reset pacemaker/view-change trackers (including the phase tracker and VRF local state) to the base pacemaker interval, clear mode-sensitive caches (pending blocks/RBC sessions/proposals/QCs), rebuild PRF/epoch managers + collectors, and refresh mode tags/PRF context in status + telemetry.
 - Pending state handling: in-flight proposals, votes, RBC sessions, and cached blocks are purged during the flip so DA/Exec gating can resume cleanly on the new mode without reusing pre-flip artefacts.
 - alignment surface: consensus fingerprints/handshake caps are recomputed and pushed to the network on flip; success/failure/blocked counters and last-error surfaces are wired for operators.
 - Tests: runtime-reset regression locks pacemaker/view/RBC-cache clearing on flip and the Sumeragi doc calls out the deterministic reset policy.【crates/iroha_core/src/sumeragi/main_loop.rs】【docs/source/sumeragi.md】

9. **P2P-GOSSIP-SCALING — Fanout strategy for large NPoS overlays** (Consensus/Networking, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Route availability/precommit votes through deterministic collector targets with commit-topology fallback to reduce broadcast fanout in the steady state.
 - [x] Send precommit-triggered block-sync updates to the commit topology instead of broadcast.
 - [x] Scope proposals, QCs, RBC, VRF, and NEW_VIEW/view-change control frames to the commit topology (validators) instead of network-wide broadcast.
 - [x] Gossip `BlockSyncUpdate` payloads (commit + vote/backfill paths) to a capped fanout (`block_gossip_size`) with per-update random sampling, removing full broadcast and documenting the fanout behavior.
 - [x] Add config knobs + telemetry for per-plane gossip fanout and target reshuffle cadence.【crates/iroha_config/src/parameters/user.rs】【crates/iroha_core/src/gossiper.rs】【crates/iroha_core/src/telemetry.rs】【crates/iroha_telemetry/src/metrics.rs】【docs/source/references/configuration.md】
 - [x] Run synthetic or localnet-scale load tests (>=22 peers) to validate bandwidth/CPU headroom. 22-peer localnet run captured with `scripts/deploy_localnet.sh --peers 22 --out-dir./localnet-22 --release --skip-asset-register --telemetry-profile extended`, storing `/status`, `/metrics`, and CPU snapshots for review.【scripts/deploy_localnet.sh】

9. **SUM-MODE-CUTOVER-OPS — Observability and docs for mixed-mode cutover** (Consensus/Sumeragi/Torii/Docs, Line: Shared, Owner: Consensus WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - Status/telemetry: `/v1/sumeragi/status` exports staged vs active tags, activation height, flip lag, kill-switch state, `{success,failure,blocked}_total`, and last flip timestamp/error; Prometheus mirrors the gauges/counters with mode labels for alerting.
 - Governance/API/SDK: `/v1/configuration` now includes consensus mode + `mode_flip_enabled`, status mirrors staged/active modes, and runtime flips recompute + re-announce consensus handshake caps to peers (dropping stale connections) so SDKs can refresh capabilities on tag changes.
 - Runbook/docs: live cutover notes document the kill switch, runtime cache/pacemaker reset, and handshake-cap refresh, plus the operator surfaces to watch for blocked flips or lag.【docs/source/sumeragi.md】【docs/source/references/configuration.md】
 - Tests: configuration endpoint regression covers consensus mode/kill-switch fields and status snapshot coverage pins flip counter/error updates.【crates/iroha_torii/tests/configuration_endpoint.rs】【crates/iroha_core/src/sumeragi/status.rs】

10. **IVM-STACK-POLICY-ALIGNMENT — Enforce configured stack limits for guest code and Rayon workers** (IVM/Compute, Line: Shared, Owner: Runtime WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - Config surface: introduce distinct knobs for guest stack bytes, scheduler worker stack bytes, and prover worker stack bytes derived from `ComputeResourceBudget.max_stack_bytes`; validate bounds at parse-time and emit config warnings when defaults are overridden or budgets are mismatched.
 - Guest stack/ABI: remove or justify the current slop window, enforce stack size at program load/admission (reject over-limit bytecode), and add tests that SP init/top-of-stack and Kotodama lowering frames stay within the configured region.
 - Pool enforcement: ensure scheduler/prover Rayon pools always use the configured stack size even when a global pool already exists; add a deterministic fallback (per-VM pool) when the global pool conflicts and cover deep-recursion paths.
 - Telemetry and failure surfacing: log/metric when stack overrides are ignored, when pool initialization falls back, and when guest stack limits are hit at runtime; expose current stack settings via status endpoints.
 - Docs/runbook/tests: update IVM docs/status.md/operator notes to spell out the stack limits and knobs; add integration/unit tests for admission rejections, SP boundaries, and pool fallback behaviour.
 - Per-route budget enforcement: (a) plumb `ComputeResourceBudget.max_stack_bytes` into admission, route execution, and the IVM builder; (b) reject/clamp bytecode exceeding `min(route budget, config cap, gas cap)` with explicit error codes; (c) add unit/integration tests for over-budget programs, SP init under per-route caps, and admission-time rejections.
 - Gas->stack policy surface: (a) expose a configurable gas->stack multiplier/strategy (with validation) instead of the fixed heuristic; (b) surface the derived stack cap and which constraint applied (route/config/gas) via status/telemetry; (c) document the policy, defaults, and operator tuning guidance.
 - Global Rayon fallback: (a) detect when a pre-existing global Rayon pool conflicts with requested stack size; (b) choose a deterministic fallback (per-VM pool or fail-fast) and emit logs/metrics for degraded mode; (c) add deep-recursion coverage under fallback and operator signals for the degraded path.

11. **NEXUS-LANE-BOUNDARY-ENFORCEMENT — Keep Iroha 2 lane-free, gate lanes to Nexus only** (Config/Core/Torii, Line: Shared, Owner: Nexus Core WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Config guards: single-lane configs now reject lane/dataspace/routing overrides when `nexus.enabled=false`, with fixtures/tests pinning the failure and Nexus profile templates unchanged.【crates/iroha_config/src/parameters/user.rs】【crates/iroha_config/tests/fixtures.rs】
 - [x] Admission/runtime: startup refuses Nexus lane overrides without `nexus.enabled` and treats any Nexus lane config as a Sora-profile feature requiring `--sora`.【crates/irohad/src/main.rs】
 - [x] API/telemetry split: gate `/status`, `/v1/sumeragi/status`, and Prometheus lane labels behind `nexus.enabled`; lane/dataspace caches reset when Nexus is disabled (dual-profile fixtures still to come).【crates/iroha_torii/src/routing.rs】
 - [x] Torii/SDK surfacing: lane-scoped endpoints (public lanes, DA commitments) return `nexus_disabled` in Iroha 2 mode with regression coverage.【crates/iroha_torii/tests/nexus_public_lanes.rs】【crates/iroha_torii/src/da/commitments.rs】
 - [x] Docs/runbook: config/transition notes call out the lane-free Iroha 2 boundary and the need for `nexus.enabled=true`/`--sora` to expose lanes.【docs/source/references/configuration.md】【docs/source/nexus_transition_notes.md】
 - [x] CI guard: lane-disabled regressions in routing/telemetry and DA/public-lane tests keep `nexus.enabled=false` builds from exposing lane metrics or routes.【crates/iroha_torii/src/routing.rs】【crates/iroha_torii/tests/nexus_public_lanes.rs】

12. **NEXUS-LANE-LIFECYCLE — Dynamic lane creation/destruction per dataspace** (Nexus/Core/Storage, Line: Iroha 3, Owner: Nexus Core WG, Priority: High, Status: 🈴 Completed, target TBD)
 - Design/API: derive lane instantiation from dataspace manifests (validator set, governance, DA/storage profile) with deterministic slug/segment geometry and shard cursors; define retirement semantics, catalog drift, and rollback on failure.
 - Storage/runtime guards: DONE — lifecycle plans now validate dataspace bindings, abort on lane storage/tiered reconciliation failures instead of mutating the catalog, and prune cached lane relay envelopes when lanes retire; ops note lives in `docs/source/nexus_transition_notes.md` and tests cover the rollback path.【crates/iroha_core/src/state.rs】
 - Provision/teardown: implement per-lane Kura/WSV segment creation/removal, routing-table rebuilds, shard cursor init, and telemetry/status diffs on lane add/remove without restart.
 - Routing/telemetry: queue/runtime helpers now reload Nexus lane/dataspace catalogs and rebuild routing decisions + TEU/backpressure snapshots at runtime; `Queue::apply_lane_lifecycle` reconfigures routing/manifests after catalog changes, guards reject lifecycle when `nexus.enabled=false`, and regression coverage pins rerouting + per-lane TEU caps.
 - Scheduling/fairness: move to per-lane queues, TEU/DA quotas, and RBC/payload budgets that rebalance when lanes are added/removed; guard head-of-line blocking during churn.
 - Governance/handoff: wire manifest activation/expiry to lane lifecycle; ensure validator-set changes propagate to per-lane consensus membership and DA policy.
 - Validator reuse: allow the same validator identities to serve multiple lanes concurrently; document/support multi-lane membership in manifest validation, scheduler membership maps, and telemetry.
 - Tooling/tests: helpers to create/retire lanes from manifests; soak/integration tests that add/remove lanes under load and assert deterministic storage paths, routing, and telemetry snapshots.
 - Ops/runbook: document lifecycle hooks, cleanup of retired segments, and alerting for stuck create/destroy flows.

13. **NEXUS-LANE-PROOFS-MERGE — Per-lane proof ingestion into the global merge ledger** (Nexus/Core/Consensus, Line: Iroha 3, Owner: Nexus Core WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Relay receiver: validate lane relays against the lane catalog/dataspace mapping, enforce commit-topology quorum for execution QCs, reject stale/conflicting or Nexus-disabled submissions, and dedupe per lane/dataspace before surfacing to status.
 - [x] Merge integration: synthesise merge-ledger entries from the latest relays across all lanes, reduce settlement hashes into `global_state_root`, persist via Kura, and suppress duplicate commits when inputs are unchanged.
 - [x] Lane finality/telemetry: accepted relays update per-lane block height/finality lag and RBC byte totals so `/status` and Prometheus expose merge progress by lane.
 - [x] Evidence/persistence/tests: relays are retained in-memory keyed per lane/dataspace with stale/conflict guards, merge-ledger commits are durable, and unit tests cover happy paths plus dataspace/quorum/stale rejection and merge synthesis.

1. **DA-QUEUED-FEED-BRIDGE — Deterministic DA receipt fanout (Iroha 3 only)** (Nexus/Core/Torii, Line: Iroha 3, Owner: DA WG + Core Protocol WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Durable receipt spool loader + cursor index track highest `(lane, epoch, sequence)` values from committed bundles; Kura hydration/commit advance cursors with telemetry updates and prune stale receipt files to bound disk usage.【crates/iroha_core/src/da/receipts.rs】【crates/iroha_core/src/state.rs】
 - [x] Block assembly enforces contiguous DA receipts per lane/epoch using cursor snapshots and sealed sets, errors when reachable receipts lack commitments, and filters bundles to receipt-aligned slices to avoid omissions or replays.【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Receipt cursor gauges surface alongside shard cursors and the DA commitments plan documents the spool-based queue, retention, and inclusion guard for operators.【crates/iroha_core/src/telemetry.rs】【docs/source/da/commitments_plan.md】【status.md】

1. **DA-CONFIDENTIAL-COMPUTE-LANES — Privacy path for SMPC/computation DA** (Nexus/Core/Storage, Line: Iroha 3, Owner: DA WG + Compute Lane WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Lane metadata flags (`confidential_compute`, `confidential_key_version`, optional mechanism/audience labels) derive deterministic confidential-compute policies for each lane.【crates/iroha_data_model/src/da/confidential_compute.rs】【crates/iroha_config/src/parameters/actual.rs】
 - [x] Validation enforces non-zero payload/manifest digests and storage tickets, rejects full-replica storage for confidential lanes, and indexes confidential-compute receipts with policy versions while hydrating them from Kura replay.【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】
 - [x] Regression coverage exercises policy parsing, validation failures, and restart hydration; the ingest plan documents the operator knobs for confidential compute lanes.【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/state.rs】【docs/source/da/ingest_plan.md】

1. **DA-PIN-REGISTRY-REPLAY — On-chain PinIntent for SoraFS/web assets** (Nexus/SoraFS/Core, Line: Iroha 3, Owner: Storage Team + Core Protocol WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Schema and block wiring: versioned `DaPinIntent`/bundle with deterministic ordering + Merkle root now hashes into `da_pin_intents_hash`, and block assembly loads spool intents, dedupes sealed entries, and keeps pin processing non-blocking for proposal build.【crates/iroha_data_model/src/da/pin_intent.rs】【crates/iroha_data_model/src/block/header.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Registry/replay/query surface: WSV stores intents keyed by ticket/alias/manifest/lane-epoch, sanitizes unknown lanes/owners/zero manifests while tagging telemetry reasons, rebuilds indexes from Kura replay, and exposes find-by-{ticket,manifest,alias,lane/epoch/seq} via Torii.【crates/iroha_core/src/da/mod.rs】【crates/iroha_core/src/state.rs】【crates/iroha_telemetry/src/metrics.rs】【crates/iroha_torii/src/da/pin_intents.rs】
 - [x] Tests/docs: spool canonicalization covers alias supersession/duplicate drops, replay/owner-drop regressions lock registry behaviour, and the DA ingest plan documents PinIntent artefacts alongside DA commitments.【crates/iroha_core/src/da/pin_intents.rs】【crates/iroha_core/src/state.rs】【docs/source/da/ingest_plan.md】

1. **DA-MANIFEST-ADMISSION-GUARD — Enforce manifest availability before commitment** (Nexus/Core/Torii, Line: Iroha 3, Owner: DA WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Availability policy + telemetry: manifest guard reasons (missing/hash mismatch/read/spool scan) feed `ManifestGuard` counters in status/telemetry alongside DA availability snapshots, honoring per-lane audit vs strict policies.【crates/iroha_data_model/src/block/consensus.rs】【crates/iroha_core/src/sumeragi/status.rs】【crates/iroha_telemetry/src/metrics.rs】
 - [x] Core wiring: block assembly scans DA spool for manifests, drops mismatched commitments, gates precommit on manifest availability, and re-evaluates guards when manifests arrive or spool errors clear while leaving audit-only lanes as warnings.【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Tests/docs: regressions cover missing/mismatched/late manifests and spool read/scan failures plus gate release after manifest arrival; the Sumeragi runbook documents manifest guard fields for operators.【crates/iroha_core/src/sumeragi/main_loop.rs】【docs/source/sumeragi.md】

1. **DA-PROOF-POLICY-LANES — Fix commitment/proof scheme per lane** (Nexus/Core/SDK, Line: Iroha 3, Owner: DA WG + SDK WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Added a versioned `DaProofPolicyBundle` with a Norito-derived `policy_hash`, threaded it into block headers via `da_proof_policies_hash`, and sealed it during block assembly so blocks pin the active per-lane proof policy set.【crates/iroha_data_model/src/da/commitment.rs】【crates/iroha_data_model/src/block/header.rs】【crates/iroha_core/src/block.rs】【crates/iroha_core/src/sumeragi/main_loop.rs】
 - [x] Torii now serves the bundle from `/v1/da/proof_policies` (version + hash + policies) and docs call out the new response shape for clients to validate lane schemes before building proofs.【crates/iroha_torii/src/da/commitments.rs】【docs/source/da/ingest_plan.md】

1. **SUM-BLOCK-FINALITY-REMEDIATION — Track DA availability and safe commit rollback paths** (Consensus/Sumeragi/Core, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
   - [x] DA availability correctness (logic):
     - [x] Remove implicit `availability_qc_view` mutation on RBC deliver; only `availability evidence` marks availability.【crates/iroha_core/src/sumeragi/main_loop.rs:20324】
     - [x] Record `MissingLocalData` until an `availability evidence` arrives when `da_enabled=true`; `finalize_pending_block` proceeds regardless and RBC deliver remains a transport/hydration path only.【crates/iroha_core/src/sumeragi/main_loop.rs:17954】【crates/iroha_core/src/sumeragi/main_loop.rs:18049】【crates/iroha_core/src/sumeragi/main_loop.rs:18160】
     - [x] Add structured telemetry/log reasons for `MissingLocalData` and record which availability condition was satisfied last.【crates/iroha_core/src/sumeragi/status.rs:2019】【crates/iroha_core/src/sumeragi/status.rs:3075】
   - [x] DA availability regressions (tests):
     - [x] Unit: DA availability reports `MissingLocalData` until a real QC arrives even if RBC delivered; DA-disabled variant still commits with QC+payload.【crates/iroha_core/src/sumeragi/main_loop.rs:17942】【crates/iroha_core/src/sumeragi/main_loop.rs:18160】
     - [x] Integration: RBC-only (no availability evidence) still records missing availability; availability evidence-only (no payload/manifests) records availability while fetch runs; QC-first with late payload + DA artifacts commits once payload is available, independent of availability status.【crates/iroha_core/src/sumeragi/main_loop.rs:18029】【crates/iroha_core/src/sumeragi/main_loop.rs:18077】【crates/iroha_core/src/sumeragi/main_loop.rs:18111】
     - [x] Regression: availability tracking keeps Highest/Locked QC stable while waiting on payload/manifest arrival; availability evidence is recorded independently.【crates/iroha_core/src/sumeragi/main_loop.rs:18862】
   - [x] Storage/WSV atomicity:
     - [x] Reorder commit path to persist to Kura before/state in lockstep, or keep block pending with retry/backoff; never advance WSV on failed store.【crates/iroha_core/src/sumeragi/main_loop.rs:6823】
     - [x] Add telemetry/alerts on Kura failure (per-block) and a retry budget.【crates/iroha_core/src/telemetry.rs:5742】【crates/iroha_telemetry/src/metrics.rs:5556】【docs/source/sumeragi.md:255】
     - [x] Regression: simulate `kura.store_block` error; assert WSV/locks/highest_qc unchanged and pending entry retained for retry.【crates/iroha_core/src/sumeragi/main_loop.rs:20186】【crates/iroha_core/src/sumeragi/main_loop.rs:20244】【crates/iroha_core/src/sumeragi/main_loop.rs:21330】
   - [x] QC-backed commit failure recovery:
     - [x] When commit fails with precommit QC quorum, keep pending block and requeue txs; trigger view-change/evidence if payload invalid.【crates/iroha_core/src/sumeragi/main_loop.rs:7062】【crates/iroha_core/src/sumeragi/main_loop.rs:7080】
     - [x] Ensure `locked_qc`/`highest_qc` realign to a committable chain (or stay unchanged) and avoid deadlock on uncommittable block.【crates/iroha_core/src/sumeragi/main_loop.rs:206】【crates/iroha_core/src/sumeragi/main_loop.rs:15870】
     - [x] Regression: invalid-payload-with-QC does not drop txs or stall liveness; view change or retry proceeds.【crates/iroha_core/src/sumeragi/main_loop.rs:7062】【crates/iroha_core/src/sumeragi/main_loop.rs:21330】
   - [x] Docs/runbook:
     - [x] Update DA/commit/finality docs with the availability matrix (DA on/off), required artifacts, and retry/backoff semantics.【docs/source/sumeragi.md:673】
     - [x] Document telemetry/log labels for DA availability warnings, Kura errors, QC-backed commit failures, and operator actions/alerts.【docs/source/sumeragi.md:673】
     - [x] Export DA availability, missing-block fetch, and Kura persistence snapshots via `/v1/sumeragi/status` (JSON + Norito) with Torii/data-model roundtrip tests and operator notes.【crates/iroha_torii/src/routing.rs:19642】【crates/iroha_torii/src/routing/consensus.rs:2169】【crates/iroha_data_model/src/block/consensus.rs:618】【docs/source/sumeragi.md:255】
1. **RBC-HYDRATE-AVAILABILITY — Require READY/DELIVER evidence before availability is marked** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Hydration now validates payload hash, chunk root, and chunk count against INIT, marking sessions invalid, updating status/backlog snapshots, and clearing pending stashes on mismatches.【crates/iroha_core/src/sumeragi/main_loop.rs:2692】【crates/iroha_core/src/sumeragi/main_loop.rs:5695】
 - [x] Hydrated sessions emit READY and only attempt DELIVER once RBC quorum and a local validator index exist; hydration never sets delivery on its own so availability remains missing until RBC evidence lands.【crates/iroha_core/src/sumeragi/main_loop.rs:5651】【crates/iroha_core/src/sumeragi/main_loop.rs:5726】
 - [x] Regression coverage holds availability missing on hydration-only paths, derives missing chunk roots, and rejects hash/root/layout mismatches.【crates/iroha_core/src/sumeragi/main_loop.rs:22172】【crates/iroha_core/src/sumeragi/main_loop.rs:22224】【crates/iroha_core/src/sumeragi/main_loop.rs:22252】【crates/iroha_core/src/sumeragi/main_loop.rs:22269】

1. **RBC-PENDING-STASH-BOUNDS — Bound pre-INIT chunk buffering** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Enforced per-session chunk/byte caps, TTL eviction, and stash-size housekeeping before INIT, dropping/evicting pending frames deterministically and tracking per-session drop counters in backlog snapshots.【crates/iroha_core/src/sumeragi/main_loop.rs:2606】【crates/iroha_core/src/sumeragi/main_loop.rs:3517】【crates/iroha_core/src/sumeragi/main_loop.rs:8467】
 - [x] Surfaced pending stash gauges and drop/eviction counters (with per-entry breakdown) via `/v1/sumeragi/status` and telemetry payloads and added regression coverage for cap eviction/TTL and drop accounting.【crates/iroha_core/src/sumeragi/status.rs:741】【crates/iroha_torii/src/routing.rs:19555】【crates/iroha_torii/src/routing/consensus.rs:2348】【crates/iroha_core/src/telemetry.rs:5796】【crates/iroha_core/src/sumeragi/main_loop.rs:22733】

1. **BLOCK-SYNC-QC-HARDENING — Reject forged block-sync aggregates** (Core/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Block-sync batches now drop when commit-signature validation fails even if a QC is present; incoming QCs are validated against BLS aggregate signatures and verified block signers before processing.【crates/iroha_core/src/block_sync.rs:904】【crates/iroha_core/src/sumeragi/main_loop.rs:595】
 - [x] QC handling reuses verified signer tallies to reject forged aggregates, mismatched bitmaps, and BLS aggregate mismatches before caching or commit, aligning block-sync updates with the commit-signature set.【crates/iroha_core/src/block_sync.rs:904】【crates/iroha_core/src/sumeragi/main_loop.rs:595】
 - [x] Regression coverage exercises forged QCs paired with single-signature or missing-proxy-tail blocks and stale signer records to prove proxy-tail/quorum enforcement in block sync.【crates/iroha_core/src/block_sync.rs:1569】【crates/iroha_core/src/block_sync.rs:1588】【crates/iroha_core/src/sumeragi/main_loop.rs:17460】

1. **SUM-VIEW-CHANGE-ROSTER-HARDENING — Avoid stalls on roster/source drift** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Roster lookup order: block sync prefers persisted commit-roster snapshots (journal -> sidecar) before caches, logs the chosen source, and drops uncertified rosters; pending-block repair uses `BlockCreated` replies instead of block-sync updates.
 - [x] Hint retirement: raw roster hints are decode-only/ignored; roster selection relies on certified commit certificates/checkpoints and durable snapshots.
 - [x] Persistence symmetry: certified rosters arriving via block sync are persisted to the journal and sidecar, with a restart regression that clears status caches and still selects the journal entry.
 - [x] PoP/topology safety: PoP filtering warns when maps are incomplete, preserves commit quorum, and reintroduces the local peer when allowed.
 - [x] Telemetry/docs: added source/drop counters (status + Prometheus) and refreshed the Sumeragi runbook with the roster source order and recovery notes.

1. **P2P-TRUST-GOSSIP-GATING — Honor trust-gossip capability without starving peer gossip** (P2P/Core, Line: Shared, Owner: Network WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Capability note details the dual handshake flags (`network.trust_gossip` and `soranet_handshake.trust_gossip`), send/recv gating, relay semantics, and operator guidance so permissioned overlays can opt out while peer-address gossip stays untouched.【docs/source/p2p.md:206】【docs/source/p2p_trust_gossip.md:1】【docs/source/references/configuration.md:15】【docs/source/references/peer.template.toml:53】
 - [x] Wiring keeps `PeerTrustGossip` on the `TrustGossip` topic, enforces capability checks on post/broadcast/recv, and counts/logs skipped trust frames without changing peer gossip caps/backoffs; skip telemetry lives at `p2p_trust_gossip_skipped_total{direction,reason}`.【crates/iroha_core/src/lib.rs:246】【crates/iroha_p2p/src/network.rs:676】【crates/iroha_p2p/src/network.rs:4011】【crates/iroha_p2p/src/network.rs:4528】【crates/iroha_telemetry/src/metrics.rs:7915】
 - [x] Unit coverage asserts the gating helper blocks trust frames when disabled while leaving other topics unaffected, exercising both send and recv paths.【crates/iroha_p2p/src/network.rs:4943】
 - [x] Integration: a `trust_gossip=false` peer neither sends nor receives trust frames but still exchanges peer-address gossip, and trust-enabled peers deliver trust frames end-to-end under the existing Low queue/caps.【crates/iroha_p2p/tests/integration/p2p_trust_gossip.rs:1】

1. **P2P-TRUST-PENALTY-SURFACE — Align unknown-peer penalty with behaviour** (P2P/Docs, Line: Shared, Owner: Network WG, Priority: Low, Status: 🈴 Completed, target TBD)
 - [x] Decision: keep unknown-peer penalties enforced for permissioned overlays (and penalty-free for public/NPoS) with operator impact/alerting captured in the trust-gossip note.【docs/source/p2p_trust_gossip.md:8】
 - [x] Enforcement/telemetry: off-topology trust gossip logs and labels `p2p_trust_penalties_total{reason="unknown_peer"}` without touching peer-address gossip, keeping penalty reasons centralised for telemetry.【crates/iroha_core/src/peers_gossiper.rs:613】
 - [x] Tests: regressions cover penalty decay -> eviction -> recovery plus the public-mode allowance so dial sets stay open when penalties are suppressed.【crates/iroha_core/src/peers_gossiper.rs:824】【crates/iroha_core/src/peers_gossiper.rs:894】【crates/iroha_core/src/peers_gossiper.rs:966】
 - [x] Templates/docs refreshed with the reason label and reinstatement flow so operators can alarm on `unknown_peer` penalties without misreading skips.【docs/source/references/peer.template.toml:51】【docs/source/references/configuration.md:16】【docs/source/p2p_trust_gossip.md:8】

1. **AXT-COMPOSABILITY — Cross-dataspace atomic transactions** (Nexus/IVM/Core, Line: Shared, Owner: mtakemiya, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Define AXT policy snapshot builder (Space Directory -> lane map) with Norito goldens and error vectors; keep `current_slot`/lane gating deterministic and sorted.【crates/iroha_data_model/tests/axt_policy_vectors.rs:1】
 - [x] Cache AXT policies in WSV on startup and during Space Directory events; host-side enforcement now falls back to cached snapshots and records telemetry tags on reject paths.【crates/iroha_core/src/state.rs:933】【crates/iroha_core/src/smartcontracts/ivm/host.rs:1342】
 - [x] Persist AXT envelopes/handles into block artifacts and admission: export handle/proof fragments from CoreHost, thread through block assembly, and add WSV/admission validation for duplicates/expiry/lane mismatches.
 - [x] Dataspace manifest lifecycle: refresh bindings on manifest expire/revoke/rotate, gossip manifest roots per lane, and add negative tests for stale/missing manifests and zeroed roots.
 - [x] Remote spend flow: validate proofs per dataspace, enforce handle budgets/sub-nonces/expiry across lanes, and add replay protection + slot-length config knob in `iroha_config`.
 - [x] Telemetry/metrics: expose policy snapshot version, reject labels (lane/manifest/era/sub_nonce/expiry), and cache-hit/miss counters for host state hydration.
 - [x] Integration tests: multi-dataspace happy path (touch+proof+handle), lane-mismatch/expired-handle/budget-exceeded rejects, manifest rotation mid-envelope, and restart resilience (cached policies restored).
 - [x] Docs/runbooks: describe AXT lane catalog mapping, manifest requirements, operator checklist for cross-dataspace composability, and SDK samples for remote spend without token egress.


1. **AXT-USABILITY — Developer and operator ergonomics for AXT** (IVM/Core/SDK, Line: Shared, Owner: mtakemiya, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Error taxonomy: structured `AxtRejectReason` codes now flow from CoreHost into overlay/block `ValidationFail::AxtReject` and Torii headers; SDKs expose enums/helpers (TS/Swift/Android) with tests for manifest/expiry/replay contexts and telemetry labels match reason codes.
 - [x] Proof cache visibility: expose per-dataspace cache status (hit/miss/invalid/slot/manifest_root) via telemetry metrics and a `/v1/debug/axt/cache` Torii endpoint; add host/unit tests that invalidate cache on manifest rotation/slot change and verify metrics labels.
 - [x] Expiry/slot ergonomics: validate `nexus.axt.slot_length_ms` against sane ranges, add `nexus.axt.max_clock_skew_ms` to host expiry checks, and document recommended values per block cadence with unit tests for zero/too-large/negative cases.
 - [x] Handle refresh hints: include `next_min_handle_era`/`next_min_sub_nonce` in host/Torii rejects; add SDK helpers to parse and request refreshed handles; add tests that the hinted minima advance after replay/era failures.
 - [x] Descriptor/build helpers: add SDK builder utilities + Norito fixtures for deterministic `AxtDescriptor`/`TouchManifest`/binding computation, ship multi-DS JSON samples, and wire a lint/test guarding descriptor/touch schema drift.【crates/iroha_data_model/src/nexus/axt.rs:1】【crates/iroha_data_model/tests/axt_descriptor_fixture.rs:1】【crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json:1】
 - [x] Replay ledger: persist handle usage/nonces in WSV (with bounded retention) and add cross-node/restart integration tests proving replay rejection after peer switch; document retention knobs.【crates/iroha_core/src/state.rs:11498】【crates/iroha_core/tests/ivm_corehost_axt.rs:1497】【docs/amx.md:21】
 - [x] Unified documentation: consolidate host + policy enforcement rules, cache semantics, and troubleshooting into one doc/runbook with per-reject remediation guidance and links to telemetry metrics.
 - [x] Golden fixtures: publish happy/reject multi-DS fixtures (handles/proofs/touches) plus reusable test helpers; add a “golden refresh” script with schema pinning and CI guard.【crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json:1】【crates/iroha_data_model/tests/fixtures/axt_envelope_multi_ds.json:1】【crates/iroha_data_model/tests/fixtures/axt_poseidon_constants.json:1】【crates/iroha_data_model/src/bin/axt_fixtures.rs:1】【ci/check_axt_fixtures.sh:1】【crates/iroha_data_model/tests/axt_envelope_fixture.rs:1】
 - [x] Observability: attach reject reason + snapshot version to Torii/block AXT responses; ensure telemetry/logs include per-envelope failure causes and propose alert templates for common rejects. Torii now stamps `X-Iroha-Axt-*` headers and ISO bridge rejection codes use `PRTRY:AXT_*` so operators can scrape reasons without decoding payloads.
 - [x] Config discoverability: surface AXT knobs (slot length, skew, replay retention, cache TTLs) in `iroha_config` defaults; add docs/examples and SDK config readers that consume the same fields. `/v1/configuration` now exports the `nexus.axt` block and JS/Swift readers normalise the values for clients.【crates/iroha_config/src/client_api.rs:1】【javascript/iroha_js/src/toriiClient.js:6436】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】【docs/source/references/configuration.md:53】


1. **AGENTS-CONFIG-SURFACE — Prefer iroha_config over env toggles** (Config/Runtime, Line: Shared, Owner: Config, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Refresh the env-toggle inventory/ownership map and stage migrations per subsystem (Torii/P2P/pipeline/IVM/FastPQ/SDKs) with target config sections and deprecation warnings where shims remain test-only. Inventory now tags `build.rs` + `#[cfg(test)]` + debug-guarded scopes (including `dev_env_flag` checks), reclassifying CUDA build flags and harness envs, and was regenerated Dec 07, 2025.
 - [x] Migrate remaining production env knobs into `iroha_config` defaults (user->actual) and host constructors; env shims (`IROHA_P2P_TOPOLOGY_UPDATE_MS`, `IVM_COMPILER_DEBUG`, `NORITO_CRC64_GPU_LIB`/`NORITO_GPU_CRC64_MIN_BYTES`, `NORITO_DISABLE_PACKED_STRUCT`, `TORII_DEBUG_SORT`) are removed entirely so both release and debug/test builds rely solely on configuration/defaults.
 - [x] Extend CI/pre-commit guardrails to fail on new env toggles in production paths and add regression tests proving each migrated knob is sourced from config (host/unit/integration as appropriate). Guard now diffs the env inventory against `AGENTS_BASE_REF` and fails on new production shims unless `ENV_CONFIG_GUARD_ALLOW=1` is explicitly set.
 - [x] Update config/operator docs plus the env inventory/migration tracker with timelines, and leave TODO breadcrumbs for any still-pending shims with assigned owners/ETA. Tracker refreshed with the new scope classification and the release guards above; next sweeps run alongside config-surface updates.


1. **P2P-DATASPACE-GOSSIP — Isolate public vs permissioned data spaces** (P2P/Core, Line: Shared, Owner: Network WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Target selection: derive per-dataspace peer sets from lane manifests/Space Directory (validators, relay policy, visibility) with cache/expiry semantics; define deterministic fallback (e.g., commit topology) and a drop policy when metadata is stale.【crates/iroha_core/src/gossiper.rs:120】
 - [x] Overlay separation: add distinct overlays/topics for public vs restricted dataspaces (tx gossip, trust gossip, lane relay, DA/RBC); enforce accept-side gating that rejects mixed-plane deliveries with stable error codes.【crates/iroha_core/src/lib.rs:226】【crates/iroha_p2p/src/network.rs:5298】
 - [x] Payload handling: refuse restricted-dataspace payloads when only public overlays are available (until encrypted fallback ships) and surface per-attempt telemetry/error labels for blocked fallbacks.【crates/iroha_core/src/gossiper.rs:386】【crates/iroha_telemetry/src/metrics.rs:7898】
 - [x] Config surface: expose dataspace-aware gossip knobs (`p2p.overlay.{public,restricted}`, per-topic caps, routing strategy, fallback policy) in `iroha_config`; wire through P2P network, gossipers, and lane relay.【crates/iroha_config/src/parameters/actual.rs:3121】【crates/iroha_config/src/parameters/user.rs:6328】
 - [x] Telemetry/ops: per-topic caps plus per-dataspace gossip/skip counters (public vs restricted), `/status` views showing chosen targets per lane/dataspace, and alerts for fallback-to-public/drop events.【crates/iroha_telemetry/src/metrics.rs:7898】【crates/iroha_core/src/telemetry.rs:811】
 - [x] Tests: integration suites mixing public/restricted lanes covering routing/targets/drop paths, lane-relay scoping, trust/peer gossip gating, DA/RBC overlays, and stale/missing metadata fallback; add unit tests for target selection and gating.【crates/iroha_core/src/gossiper.rs:860】
 - [x] Docs/runbooks: update P2P/Nexus docs with the dataspace-aware model, config examples, failure/alert taxonomy, and operator checklist for co-hosting public+restricted lanes; include troubleshooting for fallback/denial cases.【docs/source/references/configuration.md:10】


1. **NPOS-VALIDATOR-LIFECYCLE — Admission/retirement for public vs permissioned networks** (Consensus/Staking/Core, Line: Shared, Owner: Sumeragi/Stake WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Activation path (public NPoS):
 - [x] Add a governance/election instruction or scheduled hook to flip `PendingActivation` -> `Active` at epoch boundaries.【crates/iroha_core/src/state.rs:3914】【crates/iroha_core/src/smartcontracts/isi/staking.rs:87】
 - [x] Track activation epoch/height monotonically; reject out-of-order activations; tests for epoch-boundary activation and roster refresh.【crates/iroha_core/src/smartcontracts/isi/staking.rs:150】【crates/iroha_core/src/smartcontracts/isi/staking.rs:1772】
 - [x] Telemetry/metrics for activation events; Norito payloads documented.【crates/iroha_core/src/telemetry.rs:1281】【docs/source/nexus_public_lanes.md:44】
 - [x] Genesis/permissioned handling:
 - [x] Keep genesis peers `Active` without staking admission and document the invariant.
 - [x] In permissioned mode, ensure admin multisig `RegisterPeer`/`Unregister` bypasses staking guards; add regression covering admin path.【crates/iroha_core/src/smartcontracts/isi/staking.rs:62】【crates/iroha_core/src/smartcontracts/isi/staking.rs:1957】
 - [x] Exit/unregister:
 - [x] Add an exit instruction (`ExitPublicLaneValidator`) that sets `Exiting(releases_at_ms)` -> `Exited`, unblocks capacity, and gates rewards.
 - [x] Allow re-registration after `Exited`; add invariants/tests for capacity release, reward stop, slash/unbond interplay, and duplicate prevention while Exiting.
 - [x] Roster gating on peers:
 - [x] Require a live `Peer` entry (with address) before accepting `RegisterPublicLaneValidator`; fail fast with clear error/telemetry.【crates/iroha_core/src/smartcontracts/isi/staking.rs:956】【crates/iroha_core/src/telemetry.rs:1169】【crates/iroha_core/src/smartcontracts/isi/staking.rs:1567】
 - [x] Make `StakeSnapshot` drop validators whose peer is missing/disabled; tests for missing-peer gating and address changes.【crates/iroha_core/src/state.rs:4337】【crates/iroha_core/src/state.rs:4718】
 - [x] Peer unregister coupling:
 - [x] On `Unregister<Peer>`, jail/eject linked validators and prune them from rosters; ensure bonded stake/slash paths stay consistent.【crates/iroha_core/src/smartcontracts/isi/world.rs:7321】【crates/iroha_core/src/smartcontracts/isi/staking.rs:1701】
 - [x] Regression: peer removal while staked frees capacity and prevents dangling signers in consensus.【crates/iroha_core/src/smartcontracts/isi/staking.rs:1701】
 - [x] Mixed-mode branching:
 - [x] Add lane/network config selecting stake-elected activation for public lanes vs peer-admin for permissioned lanes; default sane values.【crates/iroha_config/src/parameters/actual.rs:1472】【crates/iroha_core/src/state.rs:4945】
 - [x] Tests for mixed deployments (public + restricted lanes) proving the correct path per lane and preventing cross-mode leakage.【crates/iroha_core/src/smartcontracts/isi/staking.rs:1957】【crates/iroha_core/src/state.rs:4938】【crates/iroha_core/src/state.rs:5018】
 - [x] CLI/Torii/docs/tests:
 - [x] Update staking/NPoS docs and CLI/Torii help with activation/exit lifecycle, peer prerequisites, mode selection, and genesis behavior.
 - [x] Add API/SDK/Torii docs for new instructions/endpoints; add regression tests for pending->active, capacity leak prevention, peer removal effects, re-registration after exit/slash, and mixed-mode behavior.


1. **AGENTS-DETERMINISM-ACCEL — Hardware acceleration with deterministic fallback** (IVM/Crypto/Performance, Line: Shared, Owner: IVM/Crypto, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Choose the next accel targets (ZK/FastPQ hotspots) with profiling evidence and baseline budgets so CPU-only/simd-off runs stay canonical. Metal trace `bench_trace.json` (16×2 048 columns) shows `poseidon_hash_columns` CPU 13.94 ms vs GPU 13.97 ms and `lde` CPU 5.53 ms vs GPU 3.91 ms; budgets and owners are captured in the accel runbook.【docs/source/config/acceleration.md:117】
 - [x] Implement shared accel toggle/config plumbing (METAL/NEON/SIMD/CUDA) with deterministic CPU fallbacks, keeping syscalls/opcodes shipped regardless of hardware. Host accel shims now thread the SIMD switch alongside GPU knobs in `apply_ivm_acceleration_config`, and `acceleration-state` exposes SIMD status for operators.【crates/irohad/src/main.rs:2830】【crates/ivm/src/lib.rs:178】【xtask/src/main.rs:567】
 - [x] Expand tests/benches to run accel-on/off across architectures (Metal vs CPU, optional CUDA) and assert identical outputs plus telemetry budget labels. SIMD parity now covers poseidon instructions, vector ops, and Merkle roots with accel toggled off/on, alongside existing Poseidon parity and CUDA/Metal guards.【crates/ivm/tests/crypto.rs:57】【crates/ivm/tests/crypto.rs:139】【crates/ivm/tests/acceleration_simd.rs:17】
 - [x] Update docs/comments with accel/fallback behaviour, operator knobs, and parity test recipes; leave TODO breadcrumbs for hardware-specific paths still pending.【docs/source/config/acceleration.md:1】【docs/source/metal_neon_acceleration_plan.md:24】
 - [x] SIMD observability: `acceleration_runtime_errors` now distinguishes config-disabled, forced-scalar, and hardware-missing cases; regression covers clearing overrides, and the acceleration guide documents the error strings for operators.【crates/ivm/src/lib.rs:401】【crates/ivm/tests/acceleration_simd.rs:74】【docs/source/config/acceleration.md:79】


1. **AGENTS-DOC-TEST-POLICY — Docs, tests, and PR hygiene** (Docs/QA, Line: Shared, Owner: QA/Docs, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Regenerate the missing-docs inventory for crates touched by recent changes, then queue follow-ups to add inner `//!` and item docs instead of `allow(missing_docs)`. Inventory now lives at `docs/source/agents/missing_docs_inventory.{json,md}` via `scripts/inventory_missing_docs.py` with guardrail freshness checks.【scripts/inventory_missing_docs.py:1】【docs/source/agents/missing_docs_inventory.md:1】【ci/check_missing_docs_guard.sh:1】
 - [x] Wire a guard that fails when changed functions lack at least one unit test (inline or crate `tests/`) and keep `cargo test --workspace` the default merge gate; encourage optional `cargo clippy -- -D warnings`. Guard maps changed lines to owning functions and requires a matching test reference (existing or new) instead of auto-passing on touched tests, so unrelated test edits no longer bypass coverage checks.【ci/check_tests_guard.sh:1】【ci/check_tests_guard.py:1】【ci/tests/test_check_tests_guard.py:1】
 - [x] Fill proc-macro trybuild coverage gaps for macros still missing UI diagnostics and stabilise `.stderr` fixtures; pair UI suites with unit tests for helpers.
 - [x] Add a lightweight lint/pre-commit/CI hook that blocks dropping TODO markers or skipping docs/tests on touched functions, and document the policy in CONTRIBUTING/dev workflow. TODO guard wired into Makefile/pre-commit/agents workflow (`ci/check_todo_guard.sh`) and documented alongside the new skip knob.【ci/check_todo_guard.sh:1】【Makefile:1】【hooks/pre-commit.sample:1】【docs/source/dev_workflow.md:27】【CONTRIBUTING.md:34】


1. **JS-SDK-DX — Developer experience for the JS SDK** (SDK, Line: Shared, Owner: mtakemiya (SDK WG), Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Norito fallback: wire the pure-JS Norito path when native bindings are missing/disabled; add JS-only unit tests + CI job; surface a clear error message with a “JS mode” hint; document the native-free flow in README/examples.
 - [x] Native build ergonomics: stop `getNativeBinding` from auto-building; gate builds behind an explicit `npm run build:native` (or postinstall) with a fast failure message when natives are absent; keep the loader async/non-blocking and log the exact env/steps to enable native mode.
 - [x] Packaging/exports: publish built ESM artifacts with subpath exports (`@iroha/iroha-js/torii`, `/norito`, `/crypto`, `/offline`); trim the export map to hide internals; add a bundle-size/tree-shake check (skips when esbuild is unavailable) to keep subpaths minimal.
 - [x] Surface shaping: regroup public API into namespaces (Torii client, crypto, Norito helpers, offline recipes), deprecate the monolithic barrel by re-exporting shims with warnings, and update `index.d.ts`/README to show the new import paths plus a migration guide.
 - [x] JS-only CI coverage: run the `npm run test:js` suite with the native binding disabled in CI and gate integration runs on both native and JS-only jobs to keep the fallback path green.【.github/workflows/javascript-sdk.yml:1】


1. **SDK-SWIFT-DEVEX — Clarify bridge dependency and make signing deterministic** (SDK/Swift, Line: Shared, Owner: Swift SDK WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] NoritoBridge policy: bridge loading is automatic; when `dist/NoritoBridge.xcframework` is present native helpers are enabled, and when absent Swift-only fallback is used with clear bridge-path hints in Connect/crypto/TxBuilder errors.
 - [x] Bridge coverage: Connect codec + transfer encoder now have regressions for the bridge-disabled path (using the runtime overrides) that assert `bridgeUnavailable` errors carry the bridge-path hint.
 - [x] SwiftPM guardrails: Package.swift auto-enables the bridge when the xcframework exists, emits a fallback warning when it is missing, and threads an explicit `useBridge` boolean into target defines/docs.
 - [x] SPM validation: add an SPM resolution/build job (with and without the bridge) to CI to lock the manifest behaviour; fail the job when the bridge is missing and `useBridge` is on.【scripts/check_swift_spm_validation.sh:1】【ci/check_swift_spm_validation.sh:1】【.github/workflows/swift-packaging.yml:1】
 - [x] Deterministic signing: add an injectable clock (creationTimeMs provider) to `TxBuilder`/`SwiftTransactionEncoder` and queue replay; wire default to `Date()` and allow overrides; add unit tests proving hash stability under a fixed clock and advancement under real time.
 - [x] Input validation: introduce ID validators or small wrapper types for chain/account/asset IDs in Transfer/Mint/Burn/etc.; fail fast with clear errors before bridge/Torii; add negative fixtures and assertion tests in `IrohaSwiftTests`.
 - [x] Swift parity fixtures: add a regen helper to rebuild `swift_parity_*` fixtures from payload JSON and refresh the Swift parity manifest/fixtures to match current encoder output.【crates/connect_norito_bridge/src/bin/swift_parity_regen.rs】【IrohaSwift/Fixtures/swift_parity_manifest.json】
 - [x] CocoaPods packaging: refresh `IrohaSwift.podspec` metadata (author/version/homepage/license matches repo); define how NoritoBridge is sourced (hosted URL vs vendored path) and add a `pod lib lint`/CI step that fails when the framework is missing.【IrohaSwift/IrohaSwift.podspec:1】【scripts/check_swift_pod_bridge.sh:1】【.github/workflows/swift-packaging.yml:1】【ci/check_swift_pod_bridge.sh:1】
 - [x] Install docs: README + Swift quickstart now document bridge-present/absent behaviour, automatic fallback, and troubleshooting using bridge-path hints.


1. **NEXUS-AMX-AUDIT — Close AXT/AMX cross-dataspace gaps** (Nexus/IVM/Core/Telemetry, Line: Nexus, Owner: Nexus Core WG, Priority: High, Status: 🈴 Completed, target TBD) - [x] Data model + WSV surfaces: define Norito structs/schema for per-DS AXT fragments (touch/proof/handle) with lane binding and commit markers; thread executor outputs into WSV/block artifacts with codec roundtrips and gossip/replication hooks. - [x] Admission/scheduler validation: add pre-exec checks rejecting conflicting/partial fragments and keep per-tx state hashes stable, including deterministic reschedule/rollback handling.
 - [x] Proof/PVO verification + cache: implement proof-service client integration binding proofs to DS roots/DA commitments with expiry checks; add a verify-once-per-slot cache with telemetry reasons and retry/backoff policy using `expiry_slot`.
 - [x] UAID manifest/role enforcement: evaluate Space Directory manifests for `AXT_TOUCH`/`USE_ASSET_HANDLE`; enforce lane binding, era/sub-nonce monotonicity, manifest_root match, expiry/clock skew, target_lane, and deterministic budget consumption with replay protection.
 - [x] WSV policy surface (schema + data): add a `DataspaceAxtPolicy` Norito struct and extend WSV/Space Directory snapshots with `HashMap<DataSpaceId, policy>` plus helper `current_slot = current_time_ms / slot_length_ms` (slot length configurable or defaulted).
 - [x] WSV policy persistence: add an `axt_policies` map to State/WSV populated from Space Directory manifest activation/expiry/revocation (manifest hash -> manifest_root, lane/era/sub_nonce/slot), and emit `AxtPolicySnapshot` for hosts/gossip.
 - [x] Host/admission wiring: default CoreHost/DefaultHost construction pulls `AxtPolicySnapshot` from State; admission rejects handles on lane/manifest_root/era/sub_nonce/expiry mismatch with telemetry counters for each reason.
 - [x] Block/gossip persistence: include `AxtPolicySnapshot` (or hash) in block metadata/relay payloads and hydrate it deterministically during sync.
 - [x] Policy implementation: replace `WsvAxtPolicy` with `SpaceDirectoryAxtPolicy::from_snapshot(map, current_slot)` enforcing expiry_slot > current_slot, target_lane, manifest_root, handle_era >= min_handle_era, sub_nonce >= min_sub_nonce (unknown DS -> PermissionDenied); leave binding/scope/budget/proof checks untouched.
 - [x] Host wiring: CoreHost now derives AXT policy from WSV Space Directory snapshots (lane catalog + manifest roots/activation era/current slot) instead of allow-all defaults; WsvHost refreshes policy at `AXT_BEGIN`/state restore/current_time updates.
 - [x] Tests/fixtures/docs: add accept/deny unit tests for lane/root/expiry/era/sub_nonce + missing DS for CoreHost via injected snapshots; add golden policy/handle/descriptor vectors; document policy fields/failure modes in AMX/AXT docs (WsvHost coverage landed; CoreHost now has policy snapshot/WSV tests).【crates/iroha_data_model/tests/fixtures/axt_golden.rs:1】【crates/iroha_data_model/tests/axt_policy_vectors.rs:1】【crates/ivm/tests/core_host_policy.rs:800】【docs/amx.md:71】
 - [x] Budget/telemetry: run `ivm::analysis::enforce_amx_budget` per descriptor, map overruns to `AMX_TIMEOUT`/`AMX_LOCK_CONFLICT` with DS labels, align heavy-instruction allowlists/PVO requirements across hosts, and surface metrics (`iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, abort counters) plus receipt labels.
 - [x] Tests/docs: add cross-DS commit/failure coverage (missing/expired proof, manifest deny, handle replay/budget/expiry/lane mismatch, cache cold/hit, heavy-instruction gating) with golden bindings/proof vectors; update `docs/amx*.md`, `docs/source/nexus.md`, status, SDK runbooks, and operator checklist for cache validation/telemetry alarms.


2. **SUM-QC-COVERAGE — Harden Sumeragi QC/vote validation with tests** (Consensus/Sumeragi, Status: 🈴 Completed, target TBD)
   - Subject binding: QC validation now rejects votes whose block hash differs from the QC subject with a regression guarding the path.
   - NEW_VIEW gating: HighestQC on NEW_VIEW must be a commit QC or a prepare QC at the target height; validation rejects other phases.
   - Lock extension: non-extending precommit QCs are rejected by the lock-extension helper.
   - Lock/phase safety: unit tests for rejecting precommit QCs that do not extend `locked_qc`; prevent lock regression on same-height hash mismatch; drop NEW_VIEW with wrong-phase highest QC; assert lock remains after back-to-back view changes.
   - Roster/view/epoch binding: enforce bitmap length == roster and signer IDs in range; reject QCs replayed across roster/epoch change; NEW_VIEW highest QC must match current view/epoch; add roster-change regression covering bitmap length increase/decrease.
   - Bitmap edge cases: reject empty bitmap; count duplicate bits once; reject high-bit-only sparse maps beyond roster; reject oversized bitmaps early; ensure signer-count math matches min_votes_for_commit across small/large rosters.
   - Payload/DA handling: QC-first arrival without payload must not lock/commit; with `da_enabled=true`, availability evidence is tracked (not local RBC delivery) while commit proceeds once payload is available; DA-disabled path still handles QC-first safely; add DA timeout + DA-disabled regression.
   - Byzantine leader: equivocation with two proposals same height/view emits evidence, accepts only extending QC, ignores conflicting QC; QC for already committed height with divergent hash rejected; rebuild ignores invalid signatures and refuses forged QC.
   - Recovery/sync: node starting from snapshot consumes highest QC but still enforces lock and DA before progressing; late payload arrival triggers QC rebuild and commit if quorum valid; integration test: QC-first then payload-late happy path; integration test: sync via checkpoint + highest QC with missing payload triggers block fetch before lock/commit.
   - Evidence validation: enforce cryptographic vote signature checks (Torii evidence ingest validates against commit topology), rejecting forged payloads before persistence.【crates/iroha_core/src/sumeragi/evidence.rs:383】【crates/iroha_torii/src/routing.rs:4432】
   - Commit QC integrity: validate aggregate signature/bitmap/quorum on receipt before caching/persisting; regressions cover valid/invalid aggregates.【crates/iroha_core/src/sumeragi/main_loop/qc.rs:1118】【crates/iroha_core/src/sumeragi/main_loop/tests.rs:4443】
   - Witness routing: exec witnesses target deterministic collectors per `(height, view)` with commit-topology fallback when collectors are empty/local-only/below quorum; tests/docs updated.【crates/iroha_core/src/sumeragi/main_loop/commit.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】
   - NEXT: refine into executable work items:
     1) Double-vote evidence:
        - [x] Added a shared `record_double_vote` helper and prevote/precommit regressions to persist/deduplicate equivocation evidence across the in-memory store and WSV, covering duplicate suppression on repeated votes.
     2) DA sequencing (QC-first/DA-required):
        - [x] Simulate QC arrival before payload with `da_enabled=true`, assert no lock/commit until payload fetch succeeds while availability evidence is tracked (pending block test now exercises QC-first + DA-enabled with late payload and availability QC).【crates/iroha_core/src/sumeragi/main_loop.rs:13940】
        - [x] Simulate DA-disabled path to prove QC-first still progresses (liveness) without gating (pending block regression keeps gates open when DA is off even if RBC is enabled).【crates/iroha_core/src/sumeragi/main_loop.rs:13960】
        - [x] Add late payload fetch path that triggers QC rebuild -> lock/commit; payload fetch proceeds independently of availability status and records `availability evidence` when observed.【crates/iroha_core/src/sumeragi/main_loop.rs:4181】【crates/iroha_core/src/sumeragi/main_loop.rs:15672】
     3) NEW_VIEW validation tightening:
        - [x] Reject NEW_VIEW frames whose highest QC is not commit or same-height prepare, or whose height/view regress relative to the local pacemaker/locked QC via a freshness guard.
        - [x] Regression tests cover stale-view drops and locked-QC height regression alongside the existing wrong-phase coverage.
     4) RBC resync:
        - [x] Clear missing-block fetch state once payloads arrive so retries are allowed immediately; regression confirms `touch_missing_block_request` reissues after `clear_missing_block_request`.【crates/iroha_core/src/sumeragi/main_loop.rs:12136】
        - [x] Pending-block replacement for new payload hashes resets timestamps and vote state so late payload fetches can trigger QC rebuild and precommit emission cleanly.【crates/iroha_core/src/sumeragi/main_loop.rs:15973】
        - [x] Ensure QC-first with missing block triggers block fetch before lock/commit when the payload is absent; fetch planning now falls back to the commit topology when signer hints are empty/out of range with regression coverage.【crates/iroha_core/src/sumeragi/main_loop.rs:1228】【crates/iroha_core/src/sumeragi/main_loop.rs:12208】【crates/iroha_core/src/sumeragi/main_loop.rs:12272】
        - [x] Cover missing-block resync path: after fetch, lock/commit proceeds while availability status records `availability evidence` when it arrives (or immediately when DA is off).【crates/iroha_core/src/sumeragi/main_loop.rs:14230】【crates/iroha_core/src/sumeragi/main_loop.rs:14258】
     5) Bitmap/roster replay:
        - [x] Integration-style test where a QC from an old roster (bitmap length/signers mismatch) is replayed after roster change and is rejected; include signer-index mismatch coverage.【crates/iroha_core/src/sumeragi/main_loop.rs:14634】
     6) Evidence persistence assertions:
        - [x] Integration test that feeds two conflicting proposals at same height/view, confirms evidence is recorded in WSV, and subsequent duplicates are ignored.【crates/iroha_core/src/sumeragi/evidence.rs:1558】
7) QC+block pairing + fetch enforcement:
 - [x] Add invariants/tests that paired block+QC gossip is the common case; drop/park orphaned QCs until payload fetch succeeds and assert the fetch retry/backoff path.【crates/iroha_core/src/sumeragi/main_loop.rs:1248】
 - [x] Add integration coverage for block-only and QC-only delivery (with/without DA), proving we resync deterministically and never advance locks/commit on orphaned artifacts.【crates/iroha_core/src/sumeragi/main_loop.rs:12664】【crates/iroha_core/src/sumeragi/main_loop.rs:12719】【crates/iroha_core/src/sumeragi/main_loop.rs:14952】
 - [x] Add a property/telemetry check that missing-block fetch is always issued to QC signers first (fallback to commit topology) and that lock/highest_qc are untouched until payload arrival.【crates/iroha_core/src/sumeragi/main_loop.rs:12957】
 - [x] Add a deterministic harness that replays QC->payload and payload->QC arrival orders across views/epochs to pin expected logs/metrics.【crates/iroha_core/src/sumeragi/main_loop.rs:13121】
 - [x] Add a smoke test that gossips paired block+QC together and asserts no redundant fetch requests/logs are emitted in the happy path.【crates/iroha_core/src/sumeragi/main_loop.rs:13353】
 - [x] Add a regression for empty/out-of-range signer targets to ensure missing-block fetch falls back to the full commit topology (with a warning when no peers are available).【crates/iroha_core/src/sumeragi/main_loop.rs:9033】【crates/iroha_core/src/sumeragi/main_loop.rs:14131】
 - [x] Add metrics assertions for “requested missing block payload” log/telemetry fields (targets, dwell_ms) to aid ops alerts.【crates/iroha_core/src/sumeragi/main_loop.rs:9345】【crates/iroha_core/src/sumeragi/main_loop.rs:12664】【crates/iroha_core/src/sumeragi/main_loop.rs:12719】【crates/iroha_core/src/sumeragi/status.rs:159】【crates/iroha_core/src/sumeragi/status.rs:2742】【crates/iroha_core/src/telemetry.rs:6867】【crates/iroha_core/src/telemetry.rs:6875】【crates/iroha_core/src/telemetry.rs:6886】【crates/iroha_torii/src/routing/consensus.rs:147】【crates/iroha_torii/src/routing/consensus.rs:1715】
 8) Finality proof surface hygiene:
 - [x] Clarify the canonical finality proof tuple (header + block hash + commit certificate), document the choice, and add a regression that rejects forged certificates whose hash disagrees with the stored block.【crates/iroha_core/tests/bridge_finality_proof.rs:90】【docs/source/bridge_finality.md:18】
 - [x] Add regressions for replayed finality proofs across epochs/rosters; the light-client verifier now anchors validator-set hashes and epochs, rejecting mismatched rosters/epochs with deterministic errors.【crates/iroha_data_model/src/bridge.rs:560】【crates/iroha_data_model/src/bridge.rs:588】【crates/iroha_data_model/src/bridge.rs:910】
 - [x] Add light-client proof verifier tests for wrong-chain_id and stale/advanced height to ensure rejects with stable errors.【crates/iroha_data_model/src/bridge.rs:519】【crates/iroha_data_model/src/bridge.rs:827】
 - [x] Add a bounded retention test ensuring finality proofs beyond the retention window are pruned and that fetch errors are surfaced cleanly.【crates/iroha_core/tests/bridge_finality_proof.rs:149】
 - [x] Add a replay test where a valid proof from a prior epoch/roster is rejected after topology change with a precise error label.【crates/iroha_data_model/src/bridge.rs:920】【docs/source/bridge_finality.md:72】
 9) Byzantine/DoS edge cases:
 - [x] Fuzz signer bitmaps vs. vote content (e.g., signer index points to a different key than the signature) and ensure verification catches the mismatch before aggregation.【crates/iroha_core/src/sumeragi/main_loop.rs:16693】
 - [x] Exercise adversarial leaders withholding blocks while spamming highest-QC NEW_VIEW frames; assert pacemaker/view-change throttles progress without violating locks and that evidence is emitted once a valid block is seen.【crates/iroha_core/src/sumeragi/main_loop.rs:247】【crates/iroha_core/src/sumeragi/main_loop.rs:14069】
 - [x] Add a test that drops duplicate/high-bit-only bitmaps with mismatched signatures and records evidence without progressing locks.【crates/iroha_core/src/sumeragi/main_loop.rs:16790】
 - [x] Add a pacing guard regression for repeated NEW_VIEW without payload that ensures pacemaker backoff and evidence emission once a valid block/QC arrives.【crates/iroha_core/src/sumeragi/main_loop.rs:14069】
 - [x] Add a stress test where equivocation is detected across prevote and precommit phases in the same view, ensuring deduped evidence and no lock regression.【crates/iroha_core/src/sumeragi/evidence.rs:1721】
 - [x] Add telemetry assertions for bitmap/signature mismatch paths so alerts can key off consensus errors rather than generic invalid-QC; QC validation now tags bitmap length/out-of-bounds/insufficient-or-missing-vote/subject-mismatch/invalid-signature failures into labeled counters across handle_qc/NEW_VIEW/block-sync, with mapping + disabled-telemetry regressions for alerting.【crates/iroha_core/src/sumeragi/main_loop.rs:9296】【crates/iroha_core/src/sumeragi/main_loop.rs:9676】【crates/iroha_core/src/telemetry.rs:4086】【crates/iroha_core/src/sumeragi/main_loop.rs:15668】
 10) RBC/DA fetch gating:
 - [x] Integration test where QC arrives first, payload is missing, and block fetch must complete before lock/commit (DA on/off).【crates/iroha_core/src/sumeragi/main_loop.rs:12208】【crates/iroha_core/src/sumeragi/main_loop.rs:14230】【crates/iroha_core/src/sumeragi/main_loop.rs:14292】
 - [x] Add retry/backoff assertion for `touch_missing_block_request` so liveness is maintained without premature lock updates.
 - [x] Add a DA-disabled variant proving QC-first still progresses after payload fetch without waiting on availability QC.【crates/iroha_core/src/sumeragi/main_loop.rs:14292】
 - [x] Add metrics/log invariants for retry windows and first-seen dwell times so operational alerts can key off stalled fetches.【crates/iroha_core/src/sumeragi/main_loop.rs:9085】【crates/iroha_core/src/telemetry.rs:4035】【crates/iroha_telemetry/src/metrics.rs:5051】
 - [x] Add a timed-out transport scenario that triggers a fetch and later payload arrival, asserting we commit only after both payload and (when DA-on) availability QC arrive.【crates/iroha_core/src/sumeragi/main_loop.rs:14699】
 - [x] Add a regression ensuring availability-QC arrival without payload still keeps the gate closed until payload fetch completes.【crates/iroha_core/src/sumeragi/main_loop.rs:14187】
 - Remaining gaps to close SUM-QC-COVERAGE:
 - [x] Fuzz signer bitmap/signature mismatches (index swap/different key) so verification rejects before aggregation and telemetry captures the reason.
 - [x] Simulate adversarial leaders spamming NEW_VIEW with highest-QC while withholding payloads; assert pacemaker backoff, evidence emission, and lock safety until a valid block arrives.【crates/iroha_core/src/sumeragi/main_loop.rs:247】【crates/iroha_core/src/sumeragi/main_loop.rs:18428】
 - [x] Add a regression for duplicate/high-bit-only bitmaps with mismatched signatures that records evidence without advancing locks and tags telemetry for alerting.
 - [x] Stress-test equivocation across prevote and precommit in the same view with deduped evidence and stable locks/highest_qc.【crates/iroha_core/src/sumeragi/main_loop.rs:8650】【crates/iroha_core/src/sumeragi/evidence.rs:1744】
 - [x] Add a pacing guard test for repeated NEW_VIEW without payload that proves liveness once payload+QC eventually arrive and enforces backoff in the meantime.【crates/iroha_core/src/sumeragi/main_loop.rs:18428】


1. **IROHA-CORE-TEST-STABILIZATION — Repair failing iroha_core test suite** (Core/Testing, Status: 🈴 Completed, target TBD)
 - Kura persistence/cache:
 - [x] `fast_init` tamper/truncation: reproduce hash-file truncation and tamper paths, ensure hash manifests are regenerated and truncated block data pruned (`fast_init_prunes_truncated_block_data`, `fast_init_rewrites_tampered_hash_file`).【crates/iroha_core/src/kura.rs:4652】【crates/iroha_core/src/kura.rs:4690】
 - [x] Block cache rebuild: make cached-bytes path return `None` on missing data and repopulate cache on read (`get_block_returns_none_when_data_missing`, `get_block_caches_loaded_block`, `deep_history_get_block_uses_cached_bytes`).【crates/iroha_core/src/kura.rs:4019】【crates/iroha_core/src/kura.rs:4053】【crates/iroha_core/src/kura.rs:4084】
 - [x] Merge ledger/log truncation: enforce log truncation on prune/rollback and preserve ledger entries across restart (`merge_ledger_entries_persist_across_restart`, `merge_log_truncated_when_block_store_pruned`, `kura_not_miss_replace_block`).【crates/iroha_core/src/kura.rs:3532】【crates/iroha_core/src/kura.rs:3615】
 - [x] Strict init corruption: drop corrupted index segments on startup and verify reindex (`strict_init_prunes_corrupted_index_end_to_end`).【crates/iroha_core/src/kura.rs:4569】
 - Consensus key policy/HSM:
 - [x] Allow-listed algorithms: align `register_consensus_key_*` expectations with policy config and error surfaces; add regression covering allowed/denied algorithms with precise errors.【crates/iroha_core/src/state.rs:9797】【crates/iroha_core/src/smartcontracts/isi/world.rs:9171】
 - [x] HSM gating: thread HSM-required flag through admission and lifecycle history; add coverage for HSM-required vs optional paths and rotation status (`register_consensus_key_requires_hsm_when_configured`, `rotate_consensus_key_requires_hsm_when_configured`, `rotate_consensus_key_allows_missing_hsm_when_optional`).【crates/iroha_core/src/smartcontracts/isi/world.rs:9311】【crates/iroha_core/src/smartcontracts/isi/world.rs:9395】
 - IVM host/syscalls:
 - [x] Wire syscalls 36/37/224 into the CoreHost dispatcher: test programs now encode ABI v1 so pointer-ABI transfers/NFT sentinels/INPUT_PUBLISH_TLV reach the host instead of returning `UnknownSyscall`, with regressions covering the three surfaces.【crates/iroha_core/src/smartcontracts/ivm/host.rs:1468】【crates/iroha_core/src/smartcontracts/ivm/host.rs:2218】
 - [x] Pointer-ABI TLV: wrong type IDs now surface `NoritoInvalid` with a regression covering POINTER_FROM_NORITO mismatches.【crates/ivm/tests/pointer_abi_tests.rs:19】
 - Streaming config:
 - [x] Bundle width guard now caps `bundle_width` at the SignedRansTablesV1 width during parsing, with a failing fixture/regression for oversized widths.【crates/iroha_config/tests/fixtures/bad.streaming_bundle_width.toml:1】【crates/iroha_config/tests/fixtures.rs:1720】
 - Soranet incentives numerics:
 - [x] `numeric_to_nanos` now guards nanos overflow and returns `None` when conversion would exceed `u128`, with boundary/overflow coverage so reward fixtures keep realistic conversions.【crates/iroha_core/src/soranet_incentives.rs:515】【crates/iroha_core/src/soranet_incentives.rs:808】
 - Repo/settlement/sorafs/staking retention/telemetry:
 - [x] Repo proofs: refresh lifecycle proof fixtures and deterministic ordering (`repo_deterministic_lifecycle_proof_matches_fixture`) with a helper to regenerate goldens under pinned toolchains.
 - [x] Settlement DvP: ensure commit-first/second roll back on payment spec errors with stable telemetry (`dvp_commit_first_keeps_delivery_on_payment_spec_error`, `dvp_commit_second_rolls_back_on_payment_spec_error`) and add asserts on ledger state and alerts.
 - [x] Sorafs capacity/telemetry: de-flake retention/penalty/cooldown expectations and capacity fee ledger soak tests; document expected ranges/tolerances for ops.
 - [x] Staking retention: verify prune window on retention entries (`retention_prunes_entries_older_than_grace_window`) and add fixture to lock the grace-window math.【crates/iroha_core/src/smartcontracts/isi/world.rs:5370】
 - Data triggers/fixtures:
 - [x] Refresh `aborts_on_execution_error` expect JSONs and guard transfers against negative balances so chained triggers roll back cleanly (`atomically_chains_from_{time,transaction}`).【crates/iroha_core/src/smartcontracts/isi/asset.rs:139】【crates/iroha_core/tests/fixtures/data_trigger/aborts_on_execution_error-txn.json:1】【crates/iroha_core/tests/fixtures/data_trigger/aborts_on_execution_error-time.json:1】
 - Block sync candidate rejection:
 - [x] `candidate_prev_block_hash_mismatch` now feeds a mismatched hash so ShareBlocks validation rejects bad chains deterministically with the correct error surface.【crates/iroha_core/src/block_sync.rs:739】
 - Election determinism:
 - [x] Make seat bands/correlation caps deterministic across runs and align expected seat counts (`election_is_deterministic_and_capped`, `seat_band_allows_extra_validators_and_correlation_limits_entities`) with fixtures that pin random seeds and roster variations.【crates/iroha_core/src/sumeragi/election.rs:164】【crates/iroha_core/src/sumeragi/election.rs:889】



1. **NEXUS-MULTILANE-GAPS — Enablement + CI/docs cleanup** (Nexus/Core/QA, Line: Nexus, Owner: @nexus-core, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Turn on Nexus when multi-lane catalogs are present or `--sora` is used: set the nexus profile/defaults to `enabled = true`, fail fast when `lane_count > 1` but `nexus.enabled` is false, and add unit tests for `apply_sora_profile`/`should_use_config_router` plus the sample config in `defaults/nexus/config.toml` to lock the behaviour.【crates/iroha_config/src/parameters/{defaults,actual}.rs】【crates/irohad/src/main.rs】【defaults/nexus/config.toml】
 - [x] Wire `uses_sora_features`/`enforce_build_line` to emit a clear error when multi-lane catalogs appear without `nexus.enabled`.
 - [x] Add regression that `--sora` flips `nexus.enabled` and produces the Sora catalog hashes, and that `lane_count == 1` preserves single-lane defaults (two tests: one for CLI flag, one for explicit config parse).
 - [x] Snapshot updated `defaults/nexus/config.toml` hashes in the tracker/status files used by the rehearsal/runbook so enablement is auditable.
 - [x] Restore multi-lane regression coverage + CI: add an integration/py test that drives ConfigLaneRouter with a 3-lane catalog and asserts lane-local Kura/merge-log provisioning and routing, and wire a CI target for it to replace the missing `pytests/nexus/test_multilane_pipeline.py` / `ci/integration_tests_multilane.yml` references.【crates/iroha_core/src/queue.rs】【crates/iroha_core/src/kura.rs】【integration_tests/tests/nexus/multilane_router.rs:1】【integration_tests/tests/nexus/multilane_pipeline.rs:1】【ci/check_nexus_multilane.sh:1】【ci/check_nexus_multilane_pipeline.sh:1】
 - [x] Exercise lane-specific storage layout (`lane_{id:03}_{slug}` / merge logs) and routing paths (governance/zk rules) with assertions on produced block dirs and telemetry labels.【integration_tests/tests/nexus/multilane_router.rs:1】【integration_tests/tests/nexus/multilane_pipeline.rs:1】
 - [x] Add a CI entry (agents guardrail or workflow) that runs the multilane test suite and publishes artefact hashes to avoid future drift (pin job name + path, e.g., `.github/workflows/integration_tests_multilane.yml`).【ci/check_nexus_multilane.sh:1】【ci/check_nexus_multilane_pipeline.sh:1】【.github/workflows/integration_tests_multilane.yml:1】
 - [x] Include a negative test ensuring Iroha2 build-line still rejects multi-lane configs to keep split release lines safe.【crates/irohad/src/main.rs:2802】
 - [x] Fix docs/runbooks pointing at absent artefacts once coverage exists: update Nexus transition notes/runbook and status references to cite the new test/CI job and document the required `nexus.enabled = true`/config hash for the Nexus profile.【docs/source/nexus_transition_notes.md:1】【docs/source/runbooks/nexus_multilane_rehearsal.md】【status.md】
 - [x] Replace the dead `pytests/nexus/test_multilane_pipeline.py` reference with the new test path and CI job name.【docs/source/nexus_transition_notes.md:1】
 - [x] Refresh `defaults/nexus/config.toml` hash pointers in the tracker/runbook once the enablement flag ships, and ensure `status.md` links to the validated artefacts plus any new telemetry pack manifests from the multilane CI run.
 - [x] Add a short operator checklist in the runbook noting the `nexus.enabled` requirement, the expected lane catalog digest, and where to find the multilane CI artefacts.【docs/source/runbooks/nexus_multilane_rehearsal.md】【status.md】


1. **SORAFS-SORANET-AUTHZ — Gate SoraFS/SoraNet surfaces with RBAC and ownership** (Core/Torii/Config/Docs, Line: Shared, Owner: Storage/Gov WG, Priority: High, Status: 🈴 Completed, target TBD)
 - **RBAC tokens + enforcement**
 - [x] Define data-model + executor tokens (names/payloads) for: pin register/approve/retire/alias; capacity declare/telemetry/dispute; replication order issue/complete; pricing set; provider credit upsert; SoraNet privacy ingest.
 - [x] Wire tokens into `declare_permissions!` and instruction registry; add grant/revoke validation rules and negative tests for unknown/unauthorised callers.
 - [x] Add allow/deny unit tests for replication-order issue/complete paths in `isi/sorafs.rs` to prove permissions are enforced and deterministic; expand to the remaining instructions.
 - **Ownership binding**
 - [x] Introduce provider->account binding storage (WSV table + query) populated from config/genesis/CLI; add invariants/tests for uniqueness and deletion.
 - [x] Enforce ownership or delegated role on provider-scoped actions (declare/telemetry/dispute/orders/pricing/credits) with specific error codes; add regression tests for spoofed provider IDs and missing bindings.
 - [x] Scope telemetry submitter allow-lists per provider, default `require_submitter/require_nonce=true`, expose config knobs, and test bypass attempts/fallbacks.
 - **Torii API authn/z**
 - [x] Add authn (signed token or mTLS) + rate limits to `/v1/sorafs/storage/pin`; integrate with config surface; add tests for auth success/failure, quota exhaustion, and abuse throttling.【crates/iroha_torii/src/sorafs/api.rs:2695】【crates/iroha_torii/src/sorafs/api.rs:8742】
 - [x] Add authn/z + rate limits to `/v1/soranet/privacy/{event,share}`; gate ingestion on config flag + token; add poisoning/DoS regressions and telemetry counters.
 - [x] Add optional namespace restriction (trusted subnets) for both endpoints with deny-by-default tests when not configured.
 - **Docs + tooling**
 - [x] Update docs/runbooks/CLI help to show the governance workflow (who can register/approve/retire pins, issue orders, submit telemetry), new tokens, provider binding, and endpoint auth requirements.
 - [x] Add operator checklist for provisioning tokens, binding providers, rotating submitter allow-lists, and rolling endpoint credentials; include sample configs and CLI snippets.
 - [x] Add CI guard or lint to reject new unauthenticated SoraFS/SoraNet endpoints and ensure docs stay aligned with the enforced model.【ci/check_soranet_privacy_guard.sh:1】【ci/check_agents_guardrails.sh:1】

### AGENTS Task Breakdown
 - [x] Owners/priorities recorded for all AGENTS items (Line: Shared across Iroha 2/Nexus unless noted). - [x] DevEx: contributor runbook + helper target wrapping `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build/test --workspace`, and `swift test` with runtime caveats; pre-commit/CI guards that block `Cargo.lock` edits and unapproved new workspace members, enforce fmt/clippy/test by default, and nudge large changes into TODO-backed follow-ups. Delivered via `docs/source/dev_workflow.md`, `scripts/dev_workflow.sh` (`make dev-workflow`), `.github/workflows/agents-guardrails.yml`, `ci/check_agents_guardrails.sh`, and the guard hook in `hooks/pre-commit.sample`. - [x] DevEx: dependency discipline—guardrails fail on new dependencies/workspace members and lockfile edits via `ci/check_agents_guardrails.sh` (Makefile, workflow, pre-commit) with `ci/check_dependency_discipline.sh` available as an explicit lint; docs/dev workflow/contributing guides cover the policy, and `make agents-preflight` runs all guards (including missing-docs/std-only/status-sync). - [x] Config: inventory env toggles and replace production knobs with `iroha_config` user->actual->defaults parameters threaded through constructors/hosts; keep env overrides only in dev/test harnesses, document defaults, and add regressions proving production paths source config only. Progress: block scheduler/overlay traces now live under `pipeline.debug_trace_scheduler_inputs`/`pipeline.debug_trace_tx_eval`, Torii filter debug tracing is `torii.debug_match_filters`, P2P topology updates now warn on `IROHA_P2P_TOPOLOGY_UPDATE_MS` and use config cadence, Torii attachments/webhooks/DA spools use `torii.data_dir` (env ignored outside tests), governance pipeline tracing is `governance.debug_trace_pipeline`, and the duplicate-metric panic shim is limited to debug builds with docs updated. Env toggle inventory/guard stays current via `scripts/inventory_env_toggles.py` + `docs/source/agents/env_var_inventory.{json,md}` and `ci/check_env_config_surface.sh` (`make check-env-config-surface`, `.github/workflows/env-config-guard.yml`).
 - [x] Removed production env overrides for peer gossip/topology cadence in favour of config-driven intervals with regression coverage and warnings for retired envs.
 - [x] Added `governance.debug_trace_pipeline` to replace the `IROHA_TRACE_PIPELINE` env shim and documented the knob; `torii.data_dir` now seeds the Torii persistence root with OverrideGuard reserved for tests/dev.
 - [x] Scoped the `IROHA_METRICS_PANIC_ON_DUPLICATE` env shim to debug/test builds with the config knob as the production source.
 - [x] Removed IVM env shim for non-v1 ABI opt-out (`IVM_ALLOW_NON_V1_ABI`) and banner suppression (`IVM_SUPPRESS_BANNER`); compiler now rejects non-v1 unconditionally and banner suppression stays programmatic only.
 - [x] Retired IVM cache/GPU/prover env shims (`IVM_CACHE_CAPACITY`, `IVM_CACHE_MAX_BYTES`, `IVM_MAX_DECODED_OPS`, `IVM_MAX_GPUS`, `IVM_PROVER_THREADS`) in favour of config knobs (`pipeline.{cache_size,ivm_cache_max_bytes,ivm_cache_max_decoded_ops,ivm_prover_threads}`, `accel.max_gpus`) and runtime setters (`ivm::ivm_cache::configure_limits`, `ivm::zk::set_prover_threads`); tests now use `CacheLimitsGuard` instead of env overrides and the env inventory was regenerated. - [x] Publish an env->config migration tracker (owners, priority, target config section) so any remaining toggles are sequenced and reviewed. - [x] Classified remaining production env toggles with owner/target-config proposals in `docs/source/agents/env_var_migration.md` to stage the next migration sweep. - [x] Connect diagnostics resolve `connect.queue.root` (default `~/.iroha/connect`) with the `IROHA_CONNECT_QUEUE_ROOT` shim gated to dev/test via an explicit `allowEnvOverride` opt-in; JS helpers accept config/rootDir, config templates add the knob, and docs/inventory were refreshed. - [x] Add config-driven `ivm.banner.{show,beep}` defaults (replacing the `IROHA_BEEP` shim) and honour them when rendering the startup banner/jingle; dev/test-only env overrides remain available for diagnostics. - [x] Fence the DA spool override behind `cfg(test)` helpers (`IROHA_DA_SPOOL_DIR`) and mark it test-only in the env inventory; production paths rely solely on configured spool directories. - [x] Migrate remaining production env toggles from the inventory into `iroha_config` defaults and constructors with follow-up tests/docs. FastPQ Metal tuning now flows through `fastpq.metal_{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`, applied at lane startup via `fastpq_prover::apply_metal_overrides`, which also freezes the `FASTPQ_METAL_*`/`FASTPQ_DEBUG_*` env shims as dev/test-only fallbacks once configuration loads. Docs, the env inventory, and the migration tracker were refreshed, and unit coverage maps the config -> override struct.【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】【docs/source/agents/env_var_migration.md:37】【docs/source/agents/env_var_inventory.md:1】
 - [x] Fence IVM debug env toggles (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`, `IVM_DEBUG_REGALLOC`, Metal/CUDA debug/force/self-test shims) to debug/test builds via a shared helper so production ignores them; regenerate the env inventory/migration tracker.【crates/ivm/src/dev_env.rs:1】【crates/ivm/src/vector.rs:235】【crates/ivm/src/cuda.rs:314】【docs/source/agents/env_var_inventory.md:1】【docs/source/agents/env_var_migration.md:1】
 - [x] Move SM intrinsic overrides to the `crypto.sm_intrinsics` config policy (`auto`/`force-enable`/`force-disable`), drop the `IROHA_{DISABLE,ENABLE}_SM_INTRINSICS` and `IROHA_SM_OPENSSL_PREVIEW` shims, and keep the bench/test-only override on `CRYPTO_SM_INTRINSICS`.
 - [x] Remove the `IROHA_ALLOW_NET` shim; Izanami requires the `allow_net` CLI/config flag and persists the setting via its TUI/config snapshot.
 - [x] Izanami: persist pipeline/target/progress settings in config snapshots with Norito defaults for legacy payloads; add roundtrip/legacy decode tests.【crates/izanami/src/persistence.rs:1】
 - [x] Izanami: validate TPS is finite and yields a non-zero schedule interval; add config + TUI regression tests.【crates/izanami/src/config.rs:1】【crates/izanami/src/tui.rs:1】
 - [x] Izanami: reject ultra-low TPS values that would overflow timer intervals; add regression coverage.【crates/izanami/src/config.rs:1】
 - [x] Izanami: fix TUI CLI override detection for fault toggles (arg-id mismatch) and add coverage.【crates/izanami/src/main.rs:1】
 - [x] Izanami: wake load/fault loops promptly on stop signals to avoid waiting for long intervals; add stop-notify coverage.【crates/izanami/src/chaos.rs:1】【crates/izanami/src/faults.rs:1】
 - [x] Izanami: drain completed submission tasks to prevent unbounded handle growth; add drain helper coverage.【crates/izanami/src/chaos.rs:1】
 - [x] Izanami: clamp fault-loop sleep to the remaining deadline to avoid overshooting run duration; add bounded-delay coverage.【crates/izanami/src/faults.rs:1】
 - [x] Confined `FASTPQ_UPDATE_FIXTURES` to FASTPQ integration tests so production sources no longer read it; regenerated the env inventory/migration tracker after moving the Stage 2 proof fixture check out of `src` tests.【crates/fastpq_prover/src/digest.rs:1】【crates/fastpq_prover/src/proof.rs:1】【crates/fastpq_prover/tests/proof_fixture.rs:1】【docs/source/agents/env_var_inventory.md:441】【docs/source/agents/env_var_migration.md:1】 - [x] Codec/IVM: Norito-only serialization sweep replacing direct `serde`/`serde_json` with `norito` helpers; run `scripts/check_no_scale.sh`; add CI guardrails against new serde deps; add regressions for Norito headers (including genesis) plus canonical `SignedBlockWire` encoding/decoding. *Delivered: guardrails + inventories stay green, builder JSON roundtrips cover account/asset/NFT registration paths, and Norito docs/migration guide are refreshed.*
 - [x] Inventory direct `serde`/`serde_json` usage and deps across crates (e.g., `scripts/check_no_direct_serde.sh` + targeted `rg`) with hotspots tagged; refreshed `docs/source/norito_json_inventory.{json,md}` via `scripts/inventory_serde_usage.py` (flagged production hits now 0 with scope summary).
 - [x] Draft CI/pre-commit denylist lint to block new serde deps/usages and wire it into the AGENTS guardrail set (serde/serde_json/AoS guards now run in `make agents-preflight` and the AGENTS workflow; denylist script glob handling fixed).
 - [x] Replace highest-impact serde call sites with `norito` helpers and add Norito roundtrip coverage for the touched types (including `iroha_data_model` builders). NewAccount/NewAssetDefinition builders now carry Norito `default` + deny-unknown JSON attrs, FastJsonWrite implementations, and roundtrip/unknown-field tests so registration payloads stay canonical.【crates/iroha_data_model_derive/src/registrable_builder.rs:136】【crates/iroha_data_model/src/account.rs:1396】【crates/iroha_data_model/src/account.rs:1439】【crates/iroha_data_model/src/account.rs:1455】【crates/iroha_data_model/src/asset/definition.rs:706】【crates/iroha_data_model/src/asset/definition.rs:745】
 - [x] Add Norito header/genesis roundtrip + canonical `SignedBlockWire` regressions to enforce advertised layouts (no decode heuristics); new unit coverage locks canonical vs framed bytes and genesis deframe parity.【crates/iroha_data_model/src/block/mod.rs:1773】【crates/iroha_data_model/src/block/mod.rs:1802】
 - [x] Wire `scripts/check_no_scale.sh` into `make agents-preflight`/CI so SCALE stays isolated to the Norito benchmark harness.
 - [x] Finish the Norito JSON migration checklist (enum derives/visitor helpers, config loader swap, snapshot codec/manifest cleanup per `docs/source/norito_json_migration.md`), removing TODO markers and guarding via the serde inventory.【docs/source/norito_json_migration.md:1】
 - [x] Refresh `norito.md`/codec docs and fixtures to reflect any layout/flag updates made during the sweep.【norito.md:1】 - [x] IVM: ABI v1 enforcement—keep `abi_syscall_list` ordered, map unknown syscalls to `VMError::UnknownSyscall`, maintain pointer-type IDs/policy mapping, refresh goldens/tests (`abi_syscall_list_golden.rs`, `abi_hash_versions.rs`, `pointer_type_ids_golden.rs`), update `crates/ivm/docs/syscalls.md`, and ensure admission/manifests tests cover `abi_hash` match/mismatch. *Host/admission coverage now includes a static syscall scan that rejects unknown numbers before execution plus manifest `abi_hash` enforcement across metadata and WSV manifests with docs synced to the guardrails.*
 - [x] Re-run ABI goldens (`abi_syscall_list`/pointer-type IDs/hash versions) to capture the current baseline and note any drift; `abi_syscall_list` is now deduped/sorted with goldens asserting ordering, and Experimental(1|2) map to the v1 surface while hashing separately.【crates/ivm/src/syscalls.rs:276】【crates/ivm/tests/abi_syscall_list_golden.rs:6】【crates/ivm/tests/pointer_type_ids_golden.rs:15】
 - [x] Fix ordering/unknown-syscall handling and refresh `abi_syscall_list_golden.rs`, `abi_hash_versions.rs`, and `pointer_type_ids_golden.rs` alongside host wiring as needed.【crates/ivm/src/pointer_abi.rs:97】【crates/ivm/docs/syscalls.md:17】
 - [x] Add host/admission regressions for unknown syscalls and `abi_hash` match/mismatch (including manifest enforcement) across IVM/host boundaries. Admission now scans decoded bytecode for `SCALL` instructions and rejects syscall numbers outside the active ABI policy before runtime, and manifest `abi_hash` mismatches raise structured errors for both inline and WSV-stored manifests—including metadata vs. state conflicts—with regression coverage.【crates/iroha_core/src/tx.rs:1110】【crates/iroha_core/src/tx.rs:2714】【crates/iroha_core/tests/ivm_manifest_abi_reject.rs:136】【crates/iroha_core/tests/ivm_syscall_policy.rs:70】
 - [x] Update `crates/ivm/docs/syscalls.md`, roadmap/status notes, and any ABI tables to reflect the refreshed surface.【crates/ivm/docs/syscalls.md:17】【status.md:3】 - [x] IVM/Crypto: deterministic acceleration—Poseidon2/6 opcodes are the first target; accel-on/off parity locks Metal/CUDA config toggles with CPU fallbacks and optional CUDA checks, and docs/tests capture the workflow.
 - [x] Select the first acceleration target syscall/opcode (Poseidon2/6) based on current profiling and IVM hotspots.
 - [x] Design the deterministic accel/fallback shape (feature detection + config wiring) that keeps outputs identical and ships all syscalls/opcodes; `AccelerationConfig` now drives CPU-only vs accel-on modes with runtime status assertions to mirror host policy.
 - [x] Add benches/tests that toggle accel on/off and compare outputs across architectures, capturing expected budgets/telemetry. Regression `poseidon_instructions_match_across_acceleration_configs` runs Poseidon opcodes twice (accel disabled/enabled) and cross-checks CUDA outputs when present.【crates/ivm/tests/crypto.rs:100】
 - [x] Document acceleration toggles/fallback expectations in code comments and docs, including operator-facing knobs and the parity test recipe.【docs/source/config/acceleration.md:75】 - [x] QA/Docs: docs/testing hygiene—sweep crates to remove `allow(missing_docs)`, require crate-level docs, ensure each new/modified function gains at least one unit test (inline or `tests/`), add trybuild UI coverage for proc macros, tag partial impls with `TODO:`, and keep PR templates capturing change summary + `Testing` commands. *Status: proc-macro UI coverage + docs/testing guardrails in place; keep diagnostics stable as surfaces evolve.*
 - [x] Extend the missing-docs guard into a lint/pre-commit hook that fails on changed crates lacking crate-level docs or newly introduced undocumented items; the guard now scans crate roots and new public definitions in diffs.
 - [x] Sweep existing `#[allow(missing_docs)]` allowances crate-by-crate, replacing them with real docs or `TODO:` markers and adding at least one unit test per touched function.
 - [x] Inventory proc-macro crates lacking trybuild UI coverage and add harnesses/tests for them (e.g., `norito_derive` and other derive/proc crates). - [x] Capture stable, non-panicking diagnostics in trybuild `.stderr` fixtures and document the proc-macro testing policy/guard in the dev workflow/CONTRIBUTING. - [x] Update PR template/contributor docs with explicit doc/test evidence expectations and the new hook invocation.
 Progress: CI/pre-commit guard `ci/check_missing_docs_guard.sh` blocks new `#[allow(missing_docs)]` additions, enforces crate-level docs for touched crates (including bin-only crates), and fails on new public items without `///` docs; PR template/contributor guide call out `make check-missing-docs` and doc/test evidence expectations. The sweep now covers the data-model visitor/ISI layers and derive builders: `#[model]`/`model_single!` inject docs for items/fields/variants, visitor helpers and instruction registries are documented, and instruction wire IDs plus Sumeragi NPoS getters carry docs so no `allow(missing_docs)` shims remain. trybuild UI suites now exercise the entrypoint/parameter/permission macros across `iroha_primitives_derive`, `iroha_executor_derive`, `iroha_executor_data_model_derive`, `iroha_smart_contract_derive`, and `iroha_trigger_derive` with pass/fail fixtures and golden `.stderr` outputs for the negative paths, anchoring proc-macro diagnostics to stable fixtures; executor custom parameters implement `Identifiable` so the derives’ conversions now compile cleanly. *Next: keep diagnostics stable as surfaces evolve.*【crates/iroha_data_model/src/visit/mod.rs】【crates/iroha_data_model_derive/src/model.rs】【crates/iroha_data_model_derive/src/registrable_builder.rs】【crates/iroha_data_model/src/isi/mod.rs】【crates/iroha_data_model/src/parameter/{system,custom}.rs】【crates/iroha_primitives_derive/tests/ui.rs】【crates/iroha_executor_data_model_derive/tests/ui.rs】【crates/iroha_smart_contract_derive/tests/ui.rs】【crates/iroha_trigger_derive/tests/ui.rs】【crates/iroha_executor_derive/tests/ui.rs】 - [x] Build: std-only posture—`ci/check_std_only.sh` blocks `no_std`/`wasm32` cfgs in code/CI (Makefile, agents-preflight, pre-commit, AGENTS workflow), wasm `.cargo` configs were removed, and compute/crypto spike docs now note the std-only stance. - [x] PM/Docs: status hygiene—`ci/check_status_sync.sh` (`make check-status-sync`) now fails when `roadmap.md` or `status.md` change without the other (override via `STATUS_SYNC_ALLOW_UNPAIRED=1` with `AGENTS_BASE_REF` pinned), docs/runbooks call out the stricter guard, and the PR template requires paired roadmap/status edits.【ci/check_status_sync.sh:1】【docs/source/dev_workflow.md:35】【CONTRIBUTING.md:30】【.github/pull_request_template.md:1】


1. **KAGAMI-NPOS-NETWORKS — Generate NPoS-ready networks with Kagami** (Tooling/Genesis, Status: 🈴 Completed, target TBD)
 - [x] Extend `kagami localnet`/`swarm` to accept `--consensus-mode {permissioned,npos}` and propagate the choice into generated genesis/configs/Compose manifests with NPoS cutover wiring through the swarm signing path.【crates/iroha_kagami/src/swarm.rs:137】【crates/iroha_swarm/src/schema.rs:50】【crates/iroha_kagami/src/localnet.rs:37】【crates/iroha_kagami/src/genesis/sign.rs:50】
 - [x] Allow localnet/swarm generation to set `--mode-activation-height` (default none) so staged NPoS activation is reproducible; fail fast when NPoS is requested without `sumeragi_npos_parameters` via the shared helper.【crates/iroha_kagami/src/genesis/npos.rs:16】【crates/iroha_kagami/src/swarm.rs:145】【crates/iroha_kagami/src/localnet.rs:77】
 - [x] Add doc snippets/workflows for spinning up NPoS devnets via Kagami (bare-metal and Docker), including how to supply VRF seeds/rosters and how to pass PoPs/topology at sign time.【crates/iroha_kagami/README.md:45】【crates/iroha_kagami/docs/swarm.md:97】
 - [x] Tests: regenerate localnet/swarm fixtures under both modes; add sanity checks that generated configs parse and that NPoS manifests carry `next_mode` + `mode_activation_height` when requested.【crates/iroha_kagami/src/localnet.rs:848】【crates/iroha_swarm/src/schema.rs:177】
 - [x] Enforce Iroha3 NPoS-only consensus with no staged cutovers across Kagami generate/sign/localnet/swarm, propagate BLS PoPs into swarm signing, and update Kagami swarm/NPoS docs to use the `--next-consensus-mode` + activation-height pair.【crates/iroha_kagami/src/genesis/generate.rs:410】【crates/iroha_kagami/src/genesis/sign.rs:86】【crates/iroha_kagami/src/swarm.rs:145】【crates/iroha_swarm/src/schema.rs:74】【crates/iroha_kagami/docs/swarm.md:67】


1. **SUM-MODE-CUTOVER — Make consensus mode transitions operable and explicit** (Consensus/Genesis/Config, Status: 🈴 Completed, target TBD)
 - [x] Add an explicit `--next-consensus-mode` + `--mode-activation-height` pair to `kagami genesis generate`/`sign`, stamping `next_mode` and `mode_activation_height` together while keeping `consensus_mode` as the pre-activation fingerprint; fail fast when only one is provided.
 - [x] Tighten staging guard: reject manifests that set `mode_activation_height` without `next_mode` (and vice versa, unless `next_mode` simply restates the configured mode) and make handshake fingerprints track the effective runtime mode until activation with regressions.
 - [x] Docs/runbooks: show permissioned-only vs staged permissioned->NPoS cutover, expected `mode_tag`/fingerprint before and after activation, and the governance `SetParameter` pair required for staging.
 - [x] Integration tests: staged manifests keep the permissioned fingerprint until activation height, peers compute matching pre/post-activation fingerprints across nodes, and manifest `consensus_fingerprint` mismatches fail startup; runtime cutover flips to NPoS at the activation height with status/params parity and rejects mismatched manifest modes.【integration_tests/tests/sumeragi_mode_cutover.rs:244】【integration_tests/tests/sumeragi_mode_cutover.rs:311】【integration_tests/tests/sumeragi_mode_cutover.rs:419】【crates/irohad/src/main.rs:4256】


1. **SUM-PEER-SIG-HARDENING — Harden peer/block signature validation** (Consensus/Core/Sumeragi, Status: 🈴 Completed, target TBD)
 - Vote/RBC signature enforcement
 - [x] Remove the warn-and-accept fallback for invalid BLS votes/READY/DELIVER frames; reject on signature failure across all algorithms and emit a consistent error tag.
 - [x] Add unit tests (BLS/Ed25519/secp) for bad signatures on Vote/AvailableVote/READY/DELIVER paths; assert rejection and telemetry counters/labels for `invalid_signature`.
 - [x] Wire log/metric throttling so repeated bad frames do not spam logs; add a regression ensuring per-peer rate limits trigger.【crates/iroha_core/src/sumeragi/main_loop.rs:607】【crates/iroha_core/src/sumeragi/main_loop.rs:2727】【crates/iroha_core/src/telemetry.rs:4608】【crates/iroha_telemetry/src/metrics.rs:5397】【crates/iroha_core/src/sumeragi/main_loop.rs:17985】
 - [x] Ensure gossip/block-sync handlers drop invalid signatures before queueing; add harness coverage for mixed-valid/invalid batches.【crates/iroha_core/src/block_sync.rs:629】【crates/iroha_core/src/block_sync.rs:873】
 - QC validation hardening
 - [x] Require votes for every bit set in `signers_bitmap`; remove the “missing votes with consistent aggregate” acceptance path and keep telemetry for missing-vote rejects.
 - [x] Validate BLS aggregate signatures against signer bitmaps/topology and vote preimages; add tests for forged bitmap, subject hash, height/view/epoch, and signer-order changes.
 - [x] Add a guard that signers_bitmap length matches roster and bits beyond roster cause rejection; cover with unit and integration tests.
 - [x] Reject QCs whose BLS aggregate signature fails verification for the declared bitmap/signers; add negative tests for signature mismatch.
 - [x] Update block-sync/new-view consumers to enforce the stricter QC validation before lock/commit.
 - Block commit quorum correctness
 - [x] Count validated leader + all validators (Set A/B, incl. proxy tail) toward quorum; track Set B signatures separately for visibility.
 - [x] Reject duplicate signer indices and mismatched signature keys; add regressions for leader+proxy-tail+set-B spoof attempts and duplicate-index signatures.
 - [x] Align telemetry to report both “present signatures” and “counted quorum signatures” so ops can see Set B participation alongside quorum health.【crates/iroha_telemetry/src/metrics.rs:5413】【crates/iroha_core/src/telemetry.rs:6727】【crates/iroha_core/src/sumeragi/main_loop.rs:6270】
 - [x] Ensure commit path re-runs signature validation on replacement signatures (block sync/admission) with tests for rollback on failure.
 - Consensus-key lifecycle coverage
 - [x] Enforce consensus-key liveness/expiry for leader and proxy-tail roles (not just validators) with tests for expired/missing keys across all roles.【crates/iroha_core/src/block.rs:2599】【crates/iroha_core/src/block.rs:6603】
 - [x] Add config/WSV-driven grace window tests for overlap/expiry to prove enforcement matches parameters.【crates/iroha_core/src/block.rs:6634】
 - [x] Guard consensus-key lookup against missing roster entries during view changes and block sync; block-sync filters now reapply lifecycle checks and drop expired rotations with regression coverage for leader/validator/proxy signatures.【crates/iroha_core/src/block.rs:2629】【crates/iroha_core/src/block_sync.rs:354】
 - Quorum rules for small topologies
 - [x] Restore 3-of-4 commit threshold and update collector/proxy-tail math, telemetry, and tests for 4-node deployments; add a regression for 2-of-4 being rejected.【crates/iroha_core/src/block.rs:6396】
 - [x] Recompute any dependent thresholds (collector selection, proxy-tail index) and update docs/status to match the new quorum rule.【crates/iroha_core/src/sumeragi/network_topology.rs:940】
 - [x] Add integration tests covering 1/2/3/4-node topologies to confirm quorum math and proxy-tail selection remain consistent across rotations.【crates/iroha_core/src/sumeragi/network_topology.rs:940】【crates/iroha_core/src/block.rs:6425】


1. **GENESIS-BOOTSTRAP-GOSSIP — Fetch genesis from a trusted peer** (Bootstrap/P2P/Core, Line: Shared, Owner: mtakemiya, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Protocol surface:
 - Define `GenesisRequest` (chain id + request id with optional expected hash/pubkey) and `GenesisResponse` (hash, pubkey, size hint, optional error) frames under P2P control with `Preflight`/`Fetch` kinds; cap payload size and disable responses from non-whitelisted peers (trusted set or explicit allowlist in config).
 - Include a metadata-only preflight before sending the blob so mismatches can abort early and dedupe responses by `request_id`.
 - [x] Requester flow:
 - On empty storage with no `genesis.file`, send preflight to trusted peers with configurable retry/backoff; accept only matching chain id and configured genesis pubkey.
 - If preflight hash matches (or none provided), fetch blob, run `validate_genesis_block`, reject/abort on multiple distinct hashes or signature failures, and persist the signed blob before applying it.
 - Persist the signed blob in the existing format, apply via the current commit path, and cache the hash/pubkey locally for future boots.
 - [x] Responder flow:
 - Serve only when local genesis is present and matches the configured chain id/pubkey; refuse if storage is empty or hash differs from config.
 - Honour payload cap and rate-limit responses with allowlist/trusted-peers gating and duplicate-request guards.
 - [x] Tests:
 - Unit: responder whitelist/caps, preflight pubkey checks, size-cap rejection.
 - Integration: happy-path fetch plus negative paths were added; compilation of the wider suite is currently blocked by an existing `iroha_config` Option/`config(default)` conflict unrelated to the bootstrapper.
 - [x] Docs/runbook:
 - Operator steps to enable bootstrap-from-peer (configure trusted peer + expected genesis pubkey/hash, optional allowlist flag).
 - Failure modes and recovery (no responder, hash mismatch, invalid signature, size cap, multiple hashes); note local caching after first fetch and how to pin the fetched file for subsequent boots.


2. **KAGAMI/MOCHI IROHA3 PROFILES — One-switch genesis for dev/testus/nexus** (Tooling/Genesis, Line: Iroha3, Owner: Tooling WG, Priority: Medium, Status: 🈴 Completed, target TBD)
 - [x] Kagami profiles: add `--profile {iroha3-dev, iroha3-testus, iroha3-nexus}` that pins build-line=iroha3, DA/RBC on, pre-canned chain IDs, collectors_k/r, gas limit, and VRF seed rules (dev derives from chain id; testus/nexus require explicit seed).
 - [x] Profile validation UX: enforce roster/PoP completeness per profile (dev allows 1 peer; testus/nexus require ≥4), reject conflicting overrides, and render a post-run summary (chain, DA/RBC, collectors_k/r, VRF seed presence, consensus fingerprint, kagami version).
 - [x] Verification command: add `kagami verify --profile... --genesis...` to replay profile expectations, VRF seed rules, roster/PoPs, and consensus fingerprint before shipping bundles.
 - [x] Mochi integration: surface the same profiles in `mochi-genesis`, forward to Kagami, and emit the Kagami summary in logs/CLI output.
 - [x] Docs: add quick recipes for iroha3-dev/testus/nexus, including required inputs (seed, PoPs), expected outputs (fingerprint, summary), and failure modes for profile validation.【docs/source/kagami_profiles.md】
 - [x] Tests: add profile-specific fixtures covering DA/RBC flags, VRF seed handling, roster/PoP completeness, and `kagami verify` negative/positive paths; mirror through Mochi profile wiring.
 - [x] Developer QoL: ship sample config/genesis bundles for each profile (`defaults/kagami/iroha3-{dev,testus,nexus}/`) with a `kagami verify` transcript and a `docker-compose` snippet for smoke runs; add `cargo xtask kagami-profiles --profile <...>` to regenerate them.【xtask/src/kagami_profiles.rs】【defaults/kagami/iroha3-dev】【defaults/kagami/iroha3-testus】【defaults/kagami/iroha3-nexus】

27. **ACCOUNT-IDENTITY-REFACTOR — Canonical account IDs + alias/UAID unification** (Core/Data Model/Torii/SDK, Line: Shared, Owner: Core Protocol WG, Priority: High, Status: 🈴 Completed, target TBD)
- [x] Canonicalize AccountId to i105-only wire/display format (no `@domain`), and define a resolver spec for accepted inputs (`alias@domain`, `public_key@domain`, `uaid:<hex>`, katakana address, etc.); treat this as a first-release breaking change.
 - [x] Add WSV index: `uaid -> account` (1:1 enforced).
- [x] Add WSV indices: `alias@domain -> account`, `domain_selector -> domain`, and `opaque_id -> uaid` for PII-safe tokens; block raw email/phone values from on-chain storage.
 - [x] Enforce UAID uniqueness at admission/onboarding, and replace UAID scans/portfolio aggregation with direct lookup; errors on duplicates are invariant violations.
- [x] Refactor Torii routing/address parsing to canonicalize all account inputs without config gating (remove `strict_addresses` and ISO-only alias resolver assumptions).
- [x] Update SDKs/CLI/bridges to accept the new input forms and always render i105; refresh docs/examples for Sora Nexus account addressing.
- [x] Tests: unit coverage for UAID uniqueness and index behavior.
- [x] Tests: unit + integration coverage for alias collisions, domain-selector reverse index determinism, and input canonicalization.

28. **CLI-OUTPUT-NORMALIZATION — Unify CLI outputs/flags for first release** (Tooling/CLI, Line: Shared, Owner: DX WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Normalize output formats and error handling across `iroha_cli` subcommands (JSON vs text summaries).
 - [x] Align flag naming to avoid collisions with `--output-format` (handshake token `--token-encoding`).
 - [x] Capture CLI output normalization status in `status.md`.

33. **CI-TODO-CLEANUP — Resolve open TODOs in GitHub workflows/labeler** (Tooling/CI, Line: Shared, Owner: Release Eng, Priority: Low, Status: 🈴 Completed, target TBD)
- [x] Update `.github/labeler.yml` so config label templates reflect the actual configuration surfaces (TODO #4288).
- [x] Upload clippy artifacts in the PR workflow and wire the clippy report path into Sonar (`.github/workflows/pr.yml`).

## TODO Inventory (Repo Scan 2026-01-30)

This appendix tracks open TODO markers discovered in the repository. Items are grouped by area. Reference-only TODO mentions are called out separately so they do not get mistaken for active work items.

### Code / Runtime TODOs
- None in current scan.

### SDK TODOs
- None in current scan.

### Test TODOs
- None in current scan.

### Tooling / CI TODOs
- None in current scan.

### Docs TODOs (feature gaps)
- None in current scan.

### TODO References (non-actionable / informational)
- Documentation guidance that uses “TODO” as an instruction or historical note, not an open task:
  - `docs/source/da/ingest_plan.md` and `docs/portal/docs/da/ingest-plan.md` (TODO Resolution Summary).
  - `docs/source/dev_workflow*.md` (guardrail guidance).
  - `AGENTS*.md`, `CONTRIBUTING*.md`, `CHANGELOG*.md`, `ci/check_todo_guard.sh`, `hooks/pre-commit.sample` (policy/history references).
  - `docs/source/agents/env_var_migration*.md` (policy note for temporary shims).
  - `docs/source/sorafs_gateway_dns_design_runbook*.md` (session notes instruction).
  - `docs/source/soranet/pq_rollout_plan*.md` plus portal/i18n copies (instruction to log TODOs in rollout tracker).
  - `docs/source/sdk/android/security*.md`, `docs/source/sdk/android/samples/operator_console*.md`, `docs/source/sdk/android/manifest_codegen_parity*.md`, `docs/source/sdk/swift/issues/IOS3-CONNECT-003*.md` (references to already-closed TODOs).
  - `docs/source/rust_1_91_adoption*.md`, `docs/source/rust_1_92_adoption*.md` (TODO labels in completed checklists).
  - `docs/source/sns/steward_replacement_playbook.md` (post-mortem instruction to add TODOs if needed).
  - `docs/source/ivm_isi_kotodama_alignment.ja.md` (TODO tracking table; items now marked complete).
  - `crates/ivm/README.md` (section header “Status and TODOs”).
  - `docs/source/fastpq_plan.md` (definition note explaining TODO markers).
  - `vendor/icrate/**`, `vendor/halo2-axiom/**`, `vendor/halo2curves-axiom/**` (upstream TODO markers in vendored dependencies).
