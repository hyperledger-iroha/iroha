---
lang: az
direction: ltr
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4e218c114dc00289446e7392b648ca53a1ee5061078dc4370cc73805970ee50
source_last_modified: "2026-01-18T16:28:26.568375+00:00"
translation_last_reviewed: 2026-02-07
---

## Sumeragi + NPoS Task Breakdown

This note expands the Phase A roadmap into bite-sized engineering tasks so we can land the
remaining Sumeragi/NPoS work incrementally. Status annotations follow the convention:
`✅` done, `⚙️` in progress, `⬜` not started, and `🧪` needs tests.

### A2 — Wire-Level Message Adoption
- ✅ Surface Norito `Proposal`/`Vote`/`CommitCertificate` types in `BlockMessage` and exercise encode/decode
  round-trips (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- ✅ Gate the former `BlockSigned/BlockCommitted` frames; migration toggle defaulted to `false`
  before retirement.
- ✅ Retire the migration knob that toggled the old block messages; Vote/commit-certificate mode is now the only
  wire path.
- ✅ Update Torii routers, CLI commands, and telemetry consumers to prefer
  `/v2/sumeragi/*` JSON snapshots over the older block frames.
- ✅ Integration coverage exercises `/v2/sumeragi/*` endpoints purely over the Vote/commit-certificate pipeline
  (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- ✅ Remove the old frames once feature parity & interop tests are in place.

### Frame Removal Plan
1. ✅ Multi-node soak tests ran for 72 h on both telemetry and CI harnesses; captured Torii snapshots showed stable proposer throughput and commit certificate formation with no regressions.
2. ✅ Integration test coverage now runs purely on the Vote/commit-certificate path (`sumeragi_vote_qc_commit.rs`), ensuring mixed peers reach consensus without the old frames.
3. ✅ Operator documentation and CLI help no longer mention the previous wire path; troubleshooting guidance now points at the Vote/commit-certificate telemetry.
4. ✅ Former message variants, telemetry counters, and pending commit caches were deleted; the compatibility matrix now reflects the Vote/commit-certificate-only surface.

### A3 — Engine & Pacemaker Enforcement
- ✅ Lock/Highest QC invariants enforced in `handle_message` (see `block_created_header_sanity`).
- ✅ Data-availability tracking validates the RBC payload hash when recording delivery (`Actor::ensure_block_matches_rbc_payload`) so mismatched sessions cannot be treated as delivered.
- ✅ Wire precommit commit-certificate requirement (`require_precommit_qc`) into default configs and add negative tests (default now `true`; tests cover both gated and opt-out paths).
- ✅ Replace view-wide redundant-send heuristics with EMA-backed pacemaker controllers (`aggregator_retry_deadline` now derives from the live EMA and drives redundant send deadlines).
- ✅ Gate proposal assembly on queue backpressure (`BackpressureGate` now halts the pacemaker when the queue is saturated and records deferrals for status/telemetry).
- ✅ Availability votes are emitted after proposal validation whenever DA is required (without waiting for local RBC `DELIVER`), and availability evidence is tracked via `availability evidence` as the safety proof while commit proceeds without waiting. This avoids circular waits between payload transport and voting.
- ✅ Restart/liveness coverage now exercises cold-start RBC recovery (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) and pacemaker resume after downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- ✅ Add deterministic restart/view-change regression tests covering lock convergence (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 — Collector & Randomness Pipeline
- ✅ Deterministic collector rotation helpers live in `collectors.rs`.
- ✅ GA-A4.1 — PRF-backed collector selection now records deterministic seeds and height/view in `/status` and telemetry; VRF refresh hooks propagate the context after commits and reveals. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md` (closed).
- ✅ GA-A4.2 — Surface reveal participation telemetry + CLI inspection commands and update Norito manifests. Owners: `@telemetry-ops`, `@torii-sdk`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:6`.
- ✅ GA-A4.3 — Codify late-reveal recovery and zero-participation epoch tests under `integration_tests/tests/sumeragi_randomness.rs` (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`), exercising penalty-clearing telemetry. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 — Joint Reconfiguration & Evidence
- ✅ Evidence scaffolding, WSV persistence, and Norito roundtrips now cover double-vote, invalid proposal, invalid commit certificate, and double exec variants with deterministic deduplication and horizon pruning (`sumeragi::evidence`).
- ✅ GA-A5.1 — Joint-consensus activation (old set commits, new set activates on the next block) enforced with targeted integration coverage.
- ✅ GA-A5.2 — Governance docs and CLI flows for slashing/jailing updated, complete with mdBook synchronization tests to lock defaults and evidence horizon wording.
- ✅ GA-A5.3 — Negative-path evidence tests (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) plus fuzz fixtures landed and run nightly to guard Norito roundtrip validation.

### A6 — Tooling, Docs, Validation
- ✅ RBC telemetry/reporting in place; DA report generates real metrics (including eviction counters).
- ✅ GA-A6.1 — VRF-enabled 4-peer NPoS happy-path test now runs in CI with pacemaker/RBC thresholds enforced via `integration_tests/tests/sumeragi_npos_happy_path.rs`. Owners: `@qa-consensus`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:11`.
- ✅ GA-A6.2 — Capture NPoS performance baseline (1 s blocks, k=3) and publish in `status.md`/operator docs with reproducible harness seeds + hardware matrix. Owners: `@performance-lab`, `@telemetry-ops`. Report: `docs/source/generated/sumeragi_baseline_report.md`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:12`. Live run recorded on Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) using the command documented in `scripts/run_sumeragi_baseline.py`.
- ✅ GA-A6.3 — Operator troubleshooting guides for RBC/pacemaker/backpressure instrumentation landed (`docs/source/telemetry.md:523`); log correlation is now handled by `scripts/sumeragi_backpressure_log_scraper.py`, so operators can pull pacemaker deferral/missing-availability pairings without manual grepping. Owners: `@operator-docs`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:13`.
- ✅ Added RBC store/chunk-loss performance scenarios (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`), redundant fan-out coverage (`npos_redundant_send_retries_update_metrics`), and a bounded-jitter harness (`npos_pacemaker_jitter_within_band`) so the A6 suite exercises store soft-limit deferrals, deterministic chunk drops, redundant-send telemetry, and pacemaker jitter bands under stress.【integration_tests/tests/sumeragi_npos_performance.rs:633】【integration_tests/tests/sumeragi_npos_performance.rs:760】【integration_tests/tests/sumeragi_npos_performance.rs:800】【integration_tests/tests/sumeragi_npos_performance.rs:639】

### Immediate Next Steps
1. ✅ Bounded-jitter harness exercises pacemaker jitter metrics under a deterministic band (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. ✅ Promote RBC deferral assertions in `npos_queue_backpressure_triggers_metrics` by priming deterministic RBC store pressure (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. ✅ Extend `/v2/sumeragi/telemetry` soak to cover long-running epochs and adversarial collectors,
   comparing snapshots against Prometheus counters over multiple heights. Covered by
   `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`.

Tracking this list here keeps `roadmap.md` focused on milestones while giving the team a live
checklist to burn down. Update entries (and mark completion) as patches land.
