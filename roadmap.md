# Roadmap (Open Work Only)

This roadmap enumerates the outstanding efforts required to ship the optional
NPoS Sumeragi mode and keep the broader Nexus transition on track. For every task listed here we are preparing the first public release, so teams can design and implement with a clean slate. Completed
items continue to live in `status.md`; only tasks that still need engineering
work appear here.
Latest sync: ignore non-extending pending blocks for proposal/view-change gating (localnet hang fix); reject zero-chunk RBC INITs and add empty-payload hydration guardrails; WSV mock unregister permission cleanup; merge QC view lane-tip wiring, VRF epoch snapshot persistence for NPoS scheduling/telemetry, and censorship evidence attribution; parallelize localnet status polling to avoid serial Torii stalls; SoraFS proof token decode invariants hardened (empty entries/expiry ordering); integration-test re-runs remain open (see `status.md`).

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
 - [x] Re-run `cargo test -p integration_tests sumeragi_rbc_da_large_payload_four_peers -- --nocapture` to confirm DA large-payload RBC flow completes after READY/DELIVER queue routing.
 - [x] Re-run `cargo test -p integration_tests sumeragi_rbc_da_large_payload_six_peers -- --nocapture` after wiring `sumeragi.debug.rbc.force_deliver_quorum_one` to confirm the 6-peer large-payload scenario stays within delivery budgets.
 - [x] Re-run `cargo test -p integration_tests --test address_canonicalisation -- --nocapture` to confirm the suite completes without timeouts.
 - [x] Re-run `cargo test -p integration_tests --test sumeragi_da sumeragi_rbc_recovers_after_peer_restart -- --nocapture --test-threads=1` after replay-roster fixes to confirm restart recovery completes.
 - [x] Limit integration-test network concurrency in the sandbox harness (CPU-scaled default + `IROHA_TEST_NETWORK_PARALLELISM`) so `--test-threads=1` is no longer required.
 - [x] Add a restart regression that simulates a crash after persisting a finalized VRF epoch record but before writing the seed-only next-epoch snapshot.
 - [ ] Re-run `cargo test -p integration_tests -- --nocapture` after the targeted suite completes cleanly (compiles; current sandbox denies loopback binds required for runtime).
 - [x] Re-run `cargo test -p integration_tests --test sumeragi_localnet_smoke -- --nocapture` to confirm localnet tx-status fallbacks no longer emit WARN noise.
 - [ ] Re-run integration tests that previously stalled on tx confirmation streams to validate the poll-priority + queued/approved tracking, base64 rejection decoding, and listener-connect gating for early events (focus on trigger failure and pipeline event cases).
- [ ] Re-run `cargo test -p integration_tests --test mod multiple_blocks_created -- --nocapture` (and unregister/soft-fork cases) after the merge QC view wiring (lane-tip view) and commit-certificate/roster sidecar persistence fix to confirm block-sync roster validation no longer drops updates.
 - [ ] Re-run `cargo test -p integration_tests --test sumeragi_kagami_localnet -- --nocapture` after the BlockSyncUpdate backpressure, missing-parent fetch, RBC chunk background drop, and proposal/RBC broadcast-order fixes; current sandbox run fails loopback binds and an escalated run timed out after 10m—retry outside the sandbox with `IROHA_KAGAMI_LOCALNET_KEEP=1` to retain logs.
 - [ ] Re-run `cargo test -p integration_tests --test mod network_stable_after_add_and_after_remove_peer -- --nocapture` after the RBC roster fallback; test now expects removed peers to sync, but sandboxed loopback binds (and a local `iroha3d` build error) still block execution here.
 - [ ] Re-run `cargo test -p integration_tests --test mod -- --nocapture` with `API_ADDRESS`/`PUBLIC_KEY` env overrides set to confirm test-network peers ignore host env config overrides.
 - [ ] Re-run `cargo test -p integration_tests --test mod` after the payload-hash stabilization fix (strip results/extra signatures from DA/RBC payload bytes) and the block-sync seen-block filtering; attempt 2025-12-31 timed out after 5m with pipeline event failures, peers waiting for block 1, and status endpoint connection refused—confirm the gating condition is resolved.
 - [ ] Re-run `cargo test -p integration_tests --test mod` after stabilizing NPoS PRF seed handling (seed fixed within epoch + next-epoch record persisted at rollover + replay PRF rotation) to confirm event/connected-peers suites no longer hang.
 - [ ] Re-run `cargo test -p integration_tests --test mod` after clearing consensus caches on commit-topology changes to confirm peer membership tests no longer stall consensus.
 - [ ] Re-run `cargo test --workspace` after the Sumeragi gap fixes; last attempt timed out after ~4m during compile.

4. **LOCALNET-DEMO-FLOW — Verify training-script localnet bootstrap** (Consensus/Tooling, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Align consensus rebroadcast cooldowns (RBC/pending/precommit) to on-chain `block_time` with a 200ms floor and 2x base multiplier, keeping payload replays at an additional 2x to avoid queue saturation on 1s targets.
 - [x] Widen Sumeragi worker loop drain budgets and block `BlockSyncUpdate` enqueue under backpressure to prevent DA-localnet asset test hangs.【crates/iroha_core/src/sumeragi/mod.rs】
 - [x] Switch localnet + defaults to per-queue Sumeragi channel caps (votes/payload/RBC/block) tuned for 20k TPS and 1s block times.【crates/iroha_kagami/src/localnet.rs】【crates/iroha_config/src/parameters/defaults.rs】
 - [x] Run the updated `training_script_2` workflow against a fresh 4-peer localnet and confirm block 2+ finalize quickly with DA on and no dev bypasses (full script completed; peer0 shows height 2 committed at 10:18:12.75 and height 10 at 10:18:20.84 in ~1s steps; localnet shut down).
 - [x] If Torii readiness or CLI timeouts still stall the run, adjust the script with bounded retries/timeouts and capture the needed Torii readiness checks.
 - [x] Allow DA empty-child fallback to reuse committed parent payloads when pending entries are pruned, preventing idle view-change stalls; unit test + docs updated.【crates/iroha_core/src/sumeragi/main_loop.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】
 - [x] Validate the block-sync lock realignment path (commit-certificate override) against the localnet stall scenario; 20-run sweep shows no height-49 view-change loop and ~1s block deltas at height 49.
 - [x] Let validated block-sync precommit QCs realign locks even without commit certificates to cover QC-only commits; regression ensures non-extending QC caches and advances the lock.【crates/iroha_core/src/sumeragi/main_loop/block_sync.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】
 - [x] Repeat `training_script_2` runs (>=3) on a fresh 4-peer localnet to confirm 1s cadence stability and capture any hangs/timeouts after the empty-child fallback fix (17/20 completed; 3 tx confirmation timeouts captured).
 - [x] Catch up on higher NEW_VIEW frames and gossip NEW_VIEW with a fanout cap (shared with block sync) so leaders converge on quorum without broadcast storms; tests/docs updated.【crates/iroha_core/src/sumeragi/main_loop/view_change.rs】【crates/iroha_core/src/sumeragi/main_loop/commit.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】【docs/source/p2p.md】【docs/source/references/peer.template.toml】
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

5. **INTEGRATION-TEST-STABILITY — Replace transient skips with deterministic readiness** (QA/Consensus, Line: Shared, Owner: QA WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Audit integration tests that now skip on timeouts (events pipeline, sumeragi_da, unstable_network, genesis/offline peers, triggers/time triggers) and replace with deterministic readiness probes or tuned deadlines.
 - [x] Investigate serial-guard port contention and long startup waits; tighten network teardown or add cleanup hooks to reduce startup stalls.
 - [x] Re-run `cargo test -p integration_tests --test mod` without skip paths and confirm stability across at least three consecutive runs.

6. **SUM-ACTOR-SPLIT-OBSERVABILITY — Decompose Sumeragi actor and harden inflight commit safety** (Consensus/Sumeragi, Line: Shared, Owner: Consensus WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Split DA/VRF/commit state into sub-actors/state modules with explicit interfaces (DA state isolated, VRF actor extracted, commit inflight tracking isolated).
 - [x] Added backpressure/queue diagnostics plus inflight duration metrics and `/v1/sumeragi/status` hooks for queue depths/diagnostics + commit inflight snapshots.
 - [x] Added inflight timeout/abort logic (requeue stalled commits) with unit coverage to prevent permanent stalls.

7. **OFFLINE-FLOW-BLOCKERS — Complete issuer APIs, FASTPQ proofs, and platform proof plumbing** (Core/Torii/SDK, Line: Shared, Owner: Offline WG, Priority: High, Status: 🈴 Completed, target TBD)
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

14. **CLEANUP — Remove shims and labels** (Core/SDK/Docs, Line: Shared, Owner: Release Eng, Priority: High, Status: 🈺 In Progress, target TBD)
 - [x] Remove SoraFS anon policy and SoraNet capability labels across bindings, stubs, and tests.
 - [x] Drop query-lane and genesis-bootstrap shims; fail executor validation on undecodable verdicts.
 - [x] Remove Torii `/server_version` endpoint and contract-registration `code_bytes` shims.
 - [x] Remove Torii filter-expression shim types and enforce strict JSON field shapes.
 - [x] Drop IVM header magic/padding parsing and require registration-height metadata for time triggers.
 - [x] Standardize IVM header major to v1 across code, tests, fixtures, and docs.
 - [x] Remove `iroha_python` shims and migration docs.
 - [x] Remove Python SDK governance lock stats helpers and Torii connect ping alias parsing.
 - [x] Remove Swift SDK ConnectKeyStore plaintext migration + OfflineVerdictStore shim; update the iOS demo to drop pipeline guidance.
 - [x] Remove width inference in rANS table tooling (require explicit `width_bits`).
 - [x] Clean Torii contract API and IVM docs to remove alias notes.
 - [x] Harden Norito length decoding to reject u64→usize overflows across core and AoS columnar views; add regression coverage.
 - [x] Tighten Norito StreamMapIter packed-seq offset validation and payload length accounting; add regression coverage.
 - [x] Validate NCB name/opt-str offsets for monotonicity and use canonical DecodeFromSlice in Norito streaming types; add regression coverage.
 - [x] Harden NCB columnar views with checked length multipliers, enum dict code validation, and u32-delta bounds; add regression coverage.
 - [x] Validate NCB dict codes, harden opt-column length parsing, and guard SIMD bundle length handling; add regression coverage.
 - [x] Reject id-delta underflow in NCB views and add regression coverage for delta-coded ids.
 - [x] Guard opt-column presence-bitset masking against 32-bit overflow in optstr/optu32 NCB views.
 - [x] Enforce ABI v1 only (remove Experimental policy paths) and update related tests/docs.
 - [ ] Remove Norito toggles and length/offset decode paths (update `norito.md`).
 - [x] Standardize Norito length-prefix flag scoping (COMPACT_LEN/COMPACT_SEQ_LEN/VARINT_OFFSETS) and update the spec/docs.
 - [x] Sweep docs/translations for remaining references and clean up tooling flags.

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
 - [ ] Replace stub translation files across repo root, `docs/source`, and `docs/portal` (preserve front matter, keep `source_hash`/`translation_last_reviewed` aligned).
 - [x] Complete translations for core policy/docs (CODE_OF_CONDUCT, PATENTS, README, AGENTS, status/roadmap, configuration/runbooks).
 - [ ] Prioritize portal docs: `docs/portal/i18n/*/docusaurus-plugin-content-docs` (especially SoraFS/SoraNet) and mark completion per locale.
 - [x] Portal SoraFS quickstart translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS capacity simulation runbook translations completed across all locales in `docs/portal/docs`, `docs/portal/i18n`, and `docs/source/sorafs/runbooks`.
 - [x] Portal SoraFS runbooks index translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS manifest pipeline translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS orchestrator ops runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS orchestrator tuning guide translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS multi-source rollout runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS orchestrator configuration guide translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.

25. **NEXUS-STORAGE-BUDGETS — Enforce global storage budgets + DA offload** (Storage/Core, Line: Iroha 3, Owner: Nexus Core WG, Priority: High, Status: 🈺 In Progress, target TBD)
 - [x] Add Nexus storage budget configuration with per-component weight splits, clamp Kura/WSV cold tier/SoraFS/SoraNet spool caps, and wire defaults into the Nexus profile config.
 - [x] Account for merge-ledger + sidecar + roster journal bytes (including queued blocks + retired lanes) in Kura budget enforcement.
 - [ ] Surface operator-facing warnings/telemetry when caps trigger.
 - [ ] Implement live hot-tier eviction using measured WSV memory usage (not snapshot size estimates).
 - [ ] Wire dedicated SoraVPN spool caps and DA-backed cold/Wsv retrieval for Iroha 3 while keeping Iroha 2 full-replica behavior (SoraVPN budget currently folds into SoraNet spool caps).
 - [ ] Add operator guidance + metrics for sizing `nexus.storage` budgets and monitoring cap pressure.
 - [x] Portal SoraFS node operations runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS node storage design translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS node implementation plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS provider admission policy translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS multi-source provider advert translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS reserve ledger digest translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS migration ledger translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS migration roadmap translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS pin registry implementation plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS pin registry operations runbook translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.
 - [x] Portal SoraFS pin registry validation plan translations completed across all locales in `docs/portal/docs` and `docs/portal/i18n`.

25. **RUST-1-92-ROLLOUT — Adopt Rust 1.92 lints and APIs** (Tooling/Core, Line: Shared, Owner: Release Eng, Priority: Medium, Status: 🈳 Not Started, target TBD)
 - [ ] Run `ci/check_rust_1_92_lints.sh` and fix any new deny-by-default findings (never-type fallback, macro-export arguments).
 - [ ] Sweep std `RwLock` write-then-read handoffs and use `RwLockWriteGuard::downgrade` where it preserves semantics.
 - [ ] Audit const helpers for rotate usage now that `slice::rotate_left`/`rotate_right` are const-stable.
 - [ ] Track additional Rust 1.92 API adoption in `docs/source/rust_1_92_adoption.md`.
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
 - [ ] TODO: Continue SoraFS/SoraNet portal translations beyond the completed SoraFS docs set (quickstart, runbooks, pipeline/orchestrator guides, node plan/storage, node operations, provider admission policy, multi-source provider advert, provider advert rollout, priority snapshot 2025-03, taikai monitoring dashboards, sf6 security review, sf1 determinism dry-run, orchestrator GA parity report, sf2c capacity soak report, ai moderation calibration report 2026-02, capacity marketplace validation report, GAR jurisdictional review, GAR operator onboarding, pq primitives, pq ratchet runbook, pq rollout plan, puzzle service operations guide, privacy metrics pipeline, constant-rate profiles, transport overview, testnet rollout, reserve ledger digest, migration ledger, migration roadmap, pin registry plan, pin registry ops, pin registry validation plan, portal publish plan, signing ceremony, storage capacity marketplace, chunker docs, developer docs, SDK docs), then fill remaining portal stubs per locale.
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
 - [x] Decompose Sumeragi loops (`main_loop.rs`, `mode.rs`, `validation.rs`, `pending_block.rs`, `exec_qc.rs`, `qc.rs`, `reschedule.rs`, `votes.rs`) and `block_sync.rs` validation flow.
 - [x] Refactor `account_admission.rs`, `settlement.rs` error payloads, and multisig ownership/borrowing to remove lint suppressions.【crates/iroha_core/src/smartcontracts/isi/account_admission.rs:1】【crates/iroha_core/src/smartcontracts/isi/settlement.rs:1】【crates/iroha_core/src/smartcontracts/isi/multisig.rs:1】
 - [x] Consolidate boolean policy knobs in `state.rs` into enums/bitflags and split large validation helpers.

34. **EVENTS-PIPELINE-HANG — Unblock mod.rs integration event tests** (QA/Consensus, Line: Shared, Owner: QA WG, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Reproduce `cargo test -p integration_tests --test mod` hangs and confirm DA-enabled single-peer networks stall on post-genesis commits.
 - [x] Enforce a 4-peer minimum in the integration-test harness and update direct peer-start paths to avoid single-peer stalls.
 - [x] Allow pending-block quorum reschedules to drop/requeue after a retry even with partial precommit votes; add unit test coverage.
 - [x] Re-run the full `cargo test -p integration_tests --test mod` suite to confirm stability.

41. **NEXUS-STORAGE-DA-RETENTION — DA-backed storage eviction under disk caps** (Core/DA/WSV, Line: Iroha 3, Owner: Nexus Core WG, Priority: High, Status: 🈳 Not Started, target TBD)
 - [ ] Define deterministic eviction order across Kura block bodies, tiered-state cold snapshots, and storage spools once `nexus.storage.max_disk_usage_bytes` is reached (no hardware-dependent heuristics).
 - [ ] Implement DA-backed rehydration for evicted blocks/WSV cold shards with cache hit/miss + churn telemetry.
 - [ ] Add 4-peer DA integration tests covering eviction + rehydrate with strict disk caps.

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
   - [x] Wire pacemaker tick → commit path (both lines): `tick` now drives `process_commit_candidates` with an early return on empty pending sets and records status/telemetry counters (`commit_pipeline_tick_total`, `sumeragi_commit_pipeline_tick_total{mode,outcome}`) so timer-driven commits stay visible; DA-off/permissioned and DA-off NPoS smoke runs keep quorum-timeout liveness.
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
 - Gas→stack policy surface: (a) expose a configurable gas→stack multiplier/strategy (with validation) instead of the fixed heuristic; (b) surface the derived stack cap and which constraint applied (route/config/gas) via status/telemetry; (c) document the policy, defaults, and operator tuning guidance.
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
 - [x] Roster lookup order: block sync prefers persisted commit-roster snapshots (journal → sidecar) before caches, logs the chosen source, and drops uncertified rosters; pending-block repair uses `BlockCreated` replies instead of block-sync updates.
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
 - [x] Tests: regressions cover penalty decay → eviction → recovery plus the public-mode allowance so dial sets stay open when penalties are suppressed.【crates/iroha_core/src/peers_gossiper.rs:824】【crates/iroha_core/src/peers_gossiper.rs:894】【crates/iroha_core/src/peers_gossiper.rs:966】
 - [x] Templates/docs refreshed with the reason label and reinstatement flow so operators can alarm on `unknown_peer` penalties without misreading skips.【docs/source/references/peer.template.toml:51】【docs/source/references/configuration.md:16】【docs/source/p2p_trust_gossip.md:8】

1. **AXT-COMPOSABILITY — Cross-dataspace atomic transactions** (Nexus/IVM/Core, Line: Shared, Owner: mtakemiya, Priority: High, Status: 🈴 Completed, target TBD)
 - [x] Define AXT policy snapshot builder (Space Directory → lane map) with Norito goldens and error vectors; keep `current_slot`/lane gating deterministic and sorted.【crates/iroha_data_model/tests/axt_policy_vectors.rs:1】
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
 - [x] Migrate remaining production env knobs into `iroha_config` defaults (user→actual) and host constructors; env shims (`IROHA_P2P_TOPOLOGY_UPDATE_MS`, `IVM_COMPILER_DEBUG`, `NORITO_CRC64_GPU_LIB`/`NORITO_GPU_CRC64_MIN_BYTES`, `NORITO_DISABLE_PACKED_STRUCT`, `TORII_DEBUG_SORT`) are removed entirely so both release and debug/test builds rely solely on configuration/defaults.
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
 - [x] Add a governance/election instruction or scheduled hook to flip `PendingActivation` → `Active` at epoch boundaries.【crates/iroha_core/src/state.rs:3914】【crates/iroha_core/src/smartcontracts/isi/staking.rs:87】
 - [x] Track activation epoch/height monotonically; reject out-of-order activations; tests for epoch-boundary activation and roster refresh.【crates/iroha_core/src/smartcontracts/isi/staking.rs:150】【crates/iroha_core/src/smartcontracts/isi/staking.rs:1772】
 - [x] Telemetry/metrics for activation events; Norito payloads documented.【crates/iroha_core/src/telemetry.rs:1281】【docs/source/nexus_public_lanes.md:44】
 - [x] Genesis/permissioned handling:
 - [x] Keep genesis peers `Active` without staking admission and document the invariant.
 - [x] In permissioned mode, ensure admin multisig `RegisterPeer`/`Unregister` bypasses staking guards; add regression covering admin path.【crates/iroha_core/src/smartcontracts/isi/staking.rs:62】【crates/iroha_core/src/smartcontracts/isi/staking.rs:1957】
 - [x] Exit/unregister:
 - [x] Add an exit instruction (`ExitPublicLaneValidator`) that sets `Exiting(releases_at_ms)` → `Exited`, unblocks capacity, and gates rewards.
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
 - [x] Add API/SDK/Torii docs for new instructions/endpoints; add regression tests for pending→active, capacity leak prevention, peer removal effects, re-registration after exit/slash, and mixed-mode behavior.


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
 - [x] NoritoBridge policy: default builds require `dist/NoritoBridge.xcframework` with `IROHASWIFT_USE_BRIDGE=optional/0` enabling Swift-only/disabled modes; Connect/crypto/TxBuilder errors surface the bridge-mode hint.
 - [x] Bridge coverage: Connect codec + transfer encoder now have regressions for the bridge-disabled path (using the runtime overrides) that assert `bridgeUnavailable` errors carry the env hint.
 - [x] SwiftPM guardrails: Package.swift fails fast when the xcframework is missing unless `IROHASWIFT_USE_BRIDGE=optional/0` is set, emits a fallback warning, and threads an explicit `useBridge` boolean into the target defines/docs.
 - [x] SPM validation: add an SPM resolution/build job (with and without the bridge) to CI to lock the manifest behaviour; fail the job when the bridge is missing and `useBridge` is on.【scripts/check_swift_spm_validation.sh:1】【ci/check_swift_spm_validation.sh:1】【.github/workflows/swift-packaging.yml:1】
 - [x] Deterministic signing: add an injectable clock (creationTimeMs provider) to `TxBuilder`/`SwiftTransactionEncoder` and queue replay; wire default to `Date()` and allow overrides; add unit tests proving hash stability under a fixed clock and advancement under real time.
 - [x] Input validation: introduce ID validators or small wrapper types for chain/account/asset IDs in Transfer/Mint/Burn/etc.; fail fast with clear errors before bridge/Torii; add negative fixtures and assertion tests in `IrohaSwiftTests`.
 - [x] Swift parity fixtures: add a regen helper to rebuild `swift_parity_*` fixtures from payload JSON and refresh the Swift parity manifest/fixtures to match current encoder output.【crates/connect_norito_bridge/src/bin/swift_parity_regen.rs】【IrohaSwift/Fixtures/swift_parity_manifest.json】
 - [x] CocoaPods packaging: refresh `IrohaSwift.podspec` metadata (author/version/homepage/license matches repo); define how NoritoBridge is sourced (hosted URL vs vendored path) and add a `pod lib lint`/CI step that fails when the framework is missing.【IrohaSwift/IrohaSwift.podspec:1】【scripts/check_swift_pod_bridge.sh:1】【.github/workflows/swift-packaging.yml:1】【ci/check_swift_pod_bridge.sh:1】
 - [x] Install docs: README + Swift quickstart now call out `IROHASWIFT_USE_BRIDGE` values, bridge-present/absent behaviour, and the fallback/disabled troubleshooting steps.


1. **NEXUS-AMX-AUDIT — Close AXT/AMX cross-dataspace gaps** (Nexus/IVM/Core/Telemetry, Line: Nexus, Owner: Nexus Core WG, Priority: High, Status: 🈴 Completed, target TBD) - [x] Data model + WSV surfaces: define Norito structs/schema for per-DS AXT fragments (touch/proof/handle) with lane binding and commit markers; thread executor outputs into WSV/block artifacts with codec roundtrips and gossip/replication hooks. - [x] Admission/scheduler validation: add pre-exec checks rejecting conflicting/partial fragments and keep per-tx state hashes stable, including deterministic reschedule/rollback handling.
 - [x] Proof/PVO verification + cache: implement proof-service client integration binding proofs to DS roots/DA commitments with expiry checks; add a verify-once-per-slot cache with telemetry reasons and retry/backoff policy using `expiry_slot`.
 - [x] UAID manifest/role enforcement: evaluate Space Directory manifests for `AXT_TOUCH`/`USE_ASSET_HANDLE`; enforce lane binding, era/sub-nonce monotonicity, manifest_root match, expiry/clock skew, target_lane, and deterministic budget consumption with replay protection.
 - [x] WSV policy surface (schema + data): add a `DataspaceAxtPolicy` Norito struct and extend WSV/Space Directory snapshots with `HashMap<DataSpaceId, policy>` plus helper `current_slot = current_time_ms / slot_length_ms` (slot length configurable or defaulted).
 - [x] WSV policy persistence: add an `axt_policies` map to State/WSV populated from Space Directory manifest activation/expiry/revocation (manifest hash → manifest_root, lane/era/sub_nonce/slot), and emit `AxtPolicySnapshot` for hosts/gossip.
 - [x] Host/admission wiring: default CoreHost/DefaultHost construction pulls `AxtPolicySnapshot` from State; admission rejects handles on lane/manifest_root/era/sub_nonce/expiry mismatch with telemetry counters for each reason.
 - [x] Block/gossip persistence: include `AxtPolicySnapshot` (or hash) in block metadata/relay payloads and hydrate it deterministically during sync.
 - [x] Policy implementation: replace `WsvAxtPolicy` with `SpaceDirectoryAxtPolicy::from_snapshot(map, current_slot)` enforcing expiry_slot > current_slot, target_lane, manifest_root, handle_era >= min_handle_era, sub_nonce >= min_sub_nonce (unknown DS -> PermissionDenied); leave binding/scope/budget/proof checks untouched.
 - [x] Host wiring: CoreHost now derives AXT policy from WSV Space Directory snapshots (lane catalog + manifest roots/activation era/current slot) instead of allow-all defaults; WsvHost refreshes policy at `AXT_BEGIN`/state restore/current_time updates.
 - [x] Tests/fixtures/docs: add accept/deny unit tests for lane/root/expiry/era/sub_nonce + missing DS for CoreHost via injected snapshots; add golden policy/handle/descriptor vectors; document policy fields/failure modes in AMX/AXT docs (WsvHost coverage landed; CoreHost now has policy snapshot/WSV tests).【crates/iroha_data_model/tests/fixtures/axt_golden.rs:1】【crates/iroha_data_model/tests/axt_policy_vectors.rs:1】【crates/ivm/tests/core_host_policy.rs:800】【docs/amx.md:71】
 - [x] Budget/telemetry: run `ivm::analysis::enforce_amx_budget` per descriptor, map overruns to `AMX_TIMEOUT`/`AMX_LOCK_CONFLICT` with DS labels, align heavy-instruction allowlists/PVO requirements across hosts, and surface metrics (`iroha_amx_prepare_ms`, `iroha_amx_commit_ms`, abort counters) plus receipt labels.
 - [x] Tests/docs: add cross-DS commit/failure coverage (missing/expired proof, manifest deny, handle replay/budget/expiry/lane mismatch, cache cold/hit, heavy-instruction gating) with golden bindings/proof vectors; update `docs/amx*.md`, `docs/source/nexus.md`, status, SDK runbooks, and operator checklist for cache validation/telemetry alarms.


2. **SUM-QC-COVERAGE — Harden Sumeragi QC/vote validation with tests** (Consensus/Sumeragi, Status: 🈴 Completed, target TBD)
   - Subject binding: QC validation now rejects votes whose block hash differs from the QC subject with a regression guarding the path.
   - NEW_VIEW gating: HighestQC on NEW_VIEW must be a precommit QC; validation rejects other phases.
   - Lock extension: non-extending precommit QCs are rejected by the lock-extension helper.
   - Lock/phase safety: unit tests for rejecting precommit QCs that do not extend `locked_qc`; prevent lock regression on same-height hash mismatch; drop NEW_VIEW with wrong-phase highest QC; assert lock remains after back-to-back view changes.
   - Roster/view/epoch binding: enforce bitmap length == roster and signer IDs in range; reject QCs replayed across roster/epoch change; NEW_VIEW highest QC must match current view/epoch; add roster-change regression covering bitmap length increase/decrease.
   - Bitmap edge cases: reject empty bitmap; count duplicate bits once; reject high-bit-only sparse maps beyond roster; reject oversized bitmaps early; ensure signer-count math matches min_votes_for_commit across small/large rosters.
   - Payload/DA handling: QC-first arrival without payload must not lock/commit; with `da_enabled=true`, availability evidence is tracked (not local RBC delivery) while commit proceeds once payload is available; DA-disabled path still handles QC-first safely; add DA timeout + DA-disabled regression.
   - Byzantine leader: equivocation with two proposals same height/view emits evidence, accepts only extending QC, ignores conflicting QC; QC for already committed height with divergent hash rejected; rebuild ignores invalid signatures and refuses forged QC.
   - Recovery/sync: node starting from snapshot consumes highest QC but still enforces lock and DA before progressing; late payload arrival triggers QC rebuild and commit if quorum valid; integration test: QC-first then payload-late happy path; integration test: sync via checkpoint + highest QC with missing payload triggers block fetch before lock/commit.
   - Evidence validation: enforce cryptographic vote/exec-vote signature checks (Torii evidence ingest validates against commit topology), rejecting forged payloads before persistence.【crates/iroha_core/src/sumeragi/evidence.rs:383】【crates/iroha_torii/src/routing.rs:4432】
   - ExecutionQC integrity: validate aggregate signature/bitmap/quorum on receipt before caching/persisting and block strict WSV mode without placeholder backfill; regressions cover valid/invalid aggregates.【crates/iroha_core/src/sumeragi/main_loop/qc.rs:1118】【crates/iroha_core/src/sumeragi/main_loop/exec_qc.rs:209】【crates/iroha_core/src/sumeragi/main_loop/tests.rs:4443】
   - ExecutionQC vote routing: exec votes/witness target deterministic collectors per `(height, view)` with commit-topology fallback when collectors are empty/local-only/below quorum, and stale-view exec votes are recorded for known blocks so ExecutionQCs can form after view changes; tests/docs updated.【crates/iroha_core/src/sumeragi/main_loop/commit.rs】【crates/iroha_core/src/sumeragi/main_loop/votes.rs】【crates/iroha_core/src/sumeragi/main_loop/tests.rs】【docs/source/sumeragi.md】
   - NEXT: refine into executable work items:
     1) Double-vote evidence:
        - [x] Added a shared `record_double_vote` helper and prevote/precommit regressions to persist/deduplicate equivocation evidence across the in-memory store and WSV, covering duplicate suppression on repeated votes.
     2) DA sequencing (QC-first/DA-required):
        - [x] Simulate QC arrival before payload with `da_enabled=true`, assert no lock/commit until payload fetch succeeds while availability evidence is tracked (pending block test now exercises QC-first + DA-enabled with late payload and availability QC).【crates/iroha_core/src/sumeragi/main_loop.rs:13940】
        - [x] Simulate DA-disabled path to prove QC-first still progresses (liveness) without gating (pending block regression keeps gates open when DA is off even if RBC is enabled).【crates/iroha_core/src/sumeragi/main_loop.rs:13960】
        - [x] Add late payload fetch path that triggers QC rebuild → lock/commit; payload fetch proceeds independently of availability status and records `availability evidence` when observed.【crates/iroha_core/src/sumeragi/main_loop.rs:4181】【crates/iroha_core/src/sumeragi/main_loop.rs:15672】
     3) NEW_VIEW validation tightening:
        - [x] Reject NEW_VIEW frames whose highest QC is not precommit or whose height/view regress relative to the local pacemaker/locked QC via a freshness guard.
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
 - [x] Add a deterministic harness that replays QC→payload and payload→QC arrival orders across views/epochs to pin expected logs/metrics.【crates/iroha_core/src/sumeragi/main_loop.rs:13121】
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
 - [x] Introduce provider→account binding storage (WSV table + query) populated from config/genesis/CLI; add invariants/tests for uniqueness and deletion.
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
 - [x] Owners/priorities recorded for all AGENTS items (Line: Shared across Iroha 2/Nexus unless noted). - [x] DevEx: contributor runbook + helper target wrapping `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build/test --workspace`, and `swift test` with runtime caveats; pre-commit/CI guards that block `Cargo.lock` edits and unapproved new workspace members, enforce fmt/clippy/test by default, and nudge large changes into TODO-backed follow-ups. Delivered via `docs/source/dev_workflow.md`, `scripts/dev_workflow.sh` (`make dev-workflow`), `.github/workflows/agents-guardrails.yml`, `ci/check_agents_guardrails.sh`, and the guard hook in `hooks/pre-commit.sample`. - [x] DevEx: dependency discipline—guardrails fail on new dependencies/workspace members and lockfile edits via `ci/check_agents_guardrails.sh` (Makefile, workflow, pre-commit) with `ci/check_dependency_discipline.sh` available as an explicit lint; docs/dev workflow/contributing guides cover the policy, and `make agents-preflight` runs all guards (including missing-docs/std-only/status-sync). - [x] Config: inventory env toggles and replace production knobs with `iroha_config` user→actual→defaults parameters threaded through constructors/hosts; keep env overrides only in dev/test harnesses, document defaults, and add regressions proving production paths source config only. Progress: block scheduler/overlay traces now live under `pipeline.debug_trace_scheduler_inputs`/`pipeline.debug_trace_tx_eval`, Torii filter debug tracing is `torii.debug_match_filters`, P2P topology updates now warn on `IROHA_P2P_TOPOLOGY_UPDATE_MS` and use config cadence, Torii attachments/webhooks/DA spools use `torii.data_dir` (env ignored outside tests), governance pipeline tracing is `governance.debug_trace_pipeline`, and the duplicate-metric panic shim is limited to debug builds with docs updated. Env toggle inventory/guard stays current via `scripts/inventory_env_toggles.py` + `docs/source/agents/env_var_inventory.{json,md}` and `ci/check_env_config_surface.sh` (`make check-env-config-surface`, `.github/workflows/env-config-guard.yml`).
 - [x] Removed production env overrides for peer gossip/topology cadence in favour of config-driven intervals with regression coverage and warnings for retired envs.
 - [x] Added `governance.debug_trace_pipeline` to replace the `IROHA_TRACE_PIPELINE` env shim and documented the knob; `torii.data_dir` now seeds the Torii persistence root with OverrideGuard reserved for tests/dev.
 - [x] Scoped the `IROHA_METRICS_PANIC_ON_DUPLICATE` env shim to debug/test builds with the config knob as the production source.
 - [x] Removed IVM env shim for non-v1 ABI opt-out (`IVM_ALLOW_NON_V1_ABI`) and banner suppression (`IVM_SUPPRESS_BANNER`); compiler now rejects non-v1 unconditionally and banner suppression stays programmatic only.
 - [x] Retired IVM cache/GPU/prover env shims (`IVM_CACHE_CAPACITY`, `IVM_CACHE_MAX_BYTES`, `IVM_MAX_DECODED_OPS`, `IVM_MAX_GPUS`, `IVM_PROVER_THREADS`) in favour of config knobs (`pipeline.{cache_size,ivm_cache_max_bytes,ivm_cache_max_decoded_ops,ivm_prover_threads}`, `accel.max_gpus`) and runtime setters (`ivm::ivm_cache::configure_limits`, `ivm::zk::set_prover_threads`); tests now use `CacheLimitsGuard` instead of env overrides and the env inventory was regenerated. - [x] Publish an env→config migration tracker (owners, priority, target config section) so any remaining toggles are sequenced and reviewed. - [x] Classified remaining production env toggles with owner/target-config proposals in `docs/source/agents/env_var_migration.md` to stage the next migration sweep. - [x] Connect diagnostics resolve `connect.queue.root` (default `~/.iroha/connect`) with the `IROHA_CONNECT_QUEUE_ROOT` shim gated to dev/test via an explicit `allowEnvOverride` opt-in; JS helpers accept config/rootDir, config templates add the knob, and docs/inventory were refreshed. - [x] Add config-driven `ivm.banner.{show,beep}` defaults (replacing the `IROHA_BEEP` shim) and honour them when rendering the startup banner/jingle; dev/test-only env overrides remain available for diagnostics. - [x] Fence the DA spool override behind `cfg(test)` helpers (`IROHA_DA_SPOOL_DIR`) and mark it test-only in the env inventory; production paths rely solely on configured spool directories. - [x] Migrate remaining production env toggles from the inventory into `iroha_config` defaults and constructors with follow-up tests/docs. FastPQ Metal tuning now flows through `fastpq.metal_{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`, applied at lane startup via `fastpq_prover::apply_metal_overrides`, which also freezes the `FASTPQ_METAL_*`/`FASTPQ_DEBUG_*` env shims as dev/test-only fallbacks once configuration loads. Docs, the env inventory, and the migration tracker were refreshed, and unit coverage maps the config → override struct.【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】【docs/source/agents/env_var_migration.md:37】【docs/source/agents/env_var_inventory.md:1】
 - [x] Fence IVM debug env toggles (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`, `IVM_DEBUG_REGALLOC`, Metal/CUDA debug/force/self-test shims) to debug/test builds via a shared helper so production ignores them; regenerate the env inventory/migration tracker.【crates/ivm/src/dev_env.rs:1】【crates/ivm/src/vector.rs:235】【crates/ivm/src/cuda.rs:314】【docs/source/agents/env_var_inventory.md:1】【docs/source/agents/env_var_migration.md:1】
 - [x] Move SM intrinsic overrides to the `crypto.sm_intrinsics` config policy (`auto`/`force-enable`/`force-disable`), drop the `IROHA_{DISABLE,ENABLE}_SM_INTRINSICS` and `IROHA_SM_OPENSSL_PREVIEW` shims, and keep the bench/test-only override on `CRYPTO_SM_INTRINSICS`.
 - [x] Remove the `IROHA_ALLOW_NET` shim; Izanami requires the `allow_net` CLI/config flag and persists the setting via its TUI/config snapshot.
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
 - [x] Docs/runbooks: show permissioned-only vs staged permissioned→NPoS cutover, expected `mode_tag`/fingerprint before and after activation, and the governance `SetParameter` pair required for staging.
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
