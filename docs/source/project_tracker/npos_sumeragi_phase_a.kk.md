---
lang: kk
direction: ltr
source: docs/source/project_tracker/npos_sumeragi_phase_a.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44a2d7fd2d7034d6d785465ac54474e2a5db07d4a56533b102ebc0ef13db78ad
source_last_modified: "2025-12-29T18:16:36.011793+00:00"
translation_last_reviewed: 2026-02-07
---

% NPoS Sumeragi Phase A Tracker Stub (Dec 2025)

Dispatched 2025-12-03 — Sequencing table circulated to `@sumeragi-core`, `@telemetry-ops`, `@torii-sdk`, `@governance`, `@qa-consensus`, `@performance-lab`, and `@operator-docs` for confirmation. Owners should reply on the shared tracker thread (#npos-phase-a-sync) with acceptance updates or dependency risks.

| Ticket ID | Milestone | Summary | Owners | Dependencies | Target exit | Notes |
|-----------|-----------|---------|--------|--------------|-------------|-------|
| GA-A4.1 | A4 — Collector & Randomness Pipeline | Finalise PRF-driven collector selection and expose deterministic seed snapshots in `status`/telemetry. | `@sumeragi-core` | Pacemaker/DA metrics (A3) | 2026-01-05 | Include CLI flag review with `@torii-sdk` before merge. |
| GA-A4.2 | A4 — Collector & Randomness Pipeline | Surface reveal participation metrics and CLI inspection commands; ship Norito manifest updates. | `@telemetry-ops`, `@torii-sdk` | GA-A4.1 | 2026-01-19 | Add Prometheus alert templates for reveal slippage. Completed (telemetry summary + CLI landed Dec 2025). |
| GA-A4.3 | A4 — Collector & Randomness Pipeline | Codify late-reveal recovery and zero-participation epoch tests under `integration_tests/tests/sumeragi_randomness.rs`. | `@sumeragi-core` | GA-A4.1 | 2026-01-31 | Completed (telemetry counters locked in by `npos_late_vrf_reveal_clears_penalty_and_preserves_seed` + `npos_zero_participation_epoch_reports_full_no_participation`). |
| GA-A5.1 | A5 — Joint Reconfiguration & Evidence | Enforce joint-consensus activation gate (old set commits, new set activates +1); extend integration coverage. | `@sumeragi-core` | GA-A4.3 | 2026-02-21 | Completed — integration tests now cover activation lag semantics; rehearsal notes archived with governance. |
| GA-A5.2 | A5 — Joint Reconfiguration & Evidence | Update governance docs/CLI for slashing and jailing flows; add mdbook doc-tests. | `@governance`, `@torii-sdk` | GA-A5.1 | 2026-03-05 | Completed — docs, CLI helpers, and mdBook doctests landed with Norito examples refreshed. |
| GA-A5.3 | A5 — Joint Reconfiguration & Evidence | Expand negative-path evidence tests (duplicate signer, forged signature, stale epoch replay). | `@sumeragi-core`, `@qa-consensus` | GA-A5.1 | 2026-03-14 | Completed — fuzz fixtures and nightly runs guard duplicate signer, forged signature, stale-horizon, and mixed manifest cases. |
| GA-A6.1 | A6 — Tooling, Docs, and Validation | Automate VRF-enabled 4-peer happy-path test with telemetry thresholds and RBC gating assertions. | `@qa-consensus`, `@telemetry-ops` | GA-A5.3 | 2026-04-07 | Completed — NPoS happy-path integration test runs in CI with pacemaker/RBC thresholds documented in the runbook. |
| GA-A6.2 | A6 — Tooling, Docs, and Validation | Capture NPoS performance baseline (1 s blocks, k=3) and record metrics in `status.md`/operator docs. | `@performance-lab`, `@telemetry-ops` | GA-A6.1 | 2026-04-21 | Completed — Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0); see `docs/source/generated/sumeragi_baseline_report.md`. |
| GA-A6.3 | A6 — Tooling, Docs, and Validation | Publish operator troubleshooting guides for RBC/pacemaker/backpressure instrumentation. | `@operator-docs`, `@telemetry-ops` | GA-A6.1 | 2026-04-28 | Completed — troubleshooting runbook added in `docs/source/telemetry.md:523`; automated log correlation now ships via `scripts/sumeragi_backpressure_log_scraper.py` so operators can pull deferral/RBC pairings without manual grepping. |
| GA-A6.4 | A6 — Tooling, Docs, and Validation | Extend performance harness with RBC store backpressure and chunk-loss scenarios (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`). | `@performance-lab`, `@telemetry-ops` | GA-A6.2 | 2026-05-05 | Completed — redundant-send fan-out retries covered by `npos_redundant_send_retries_update_metrics` and pacemaker jitter bands validated via `npos_pacemaker_jitter_within_band` (see targeted `cargo test -p integration_tests --test sumeragi_npos_performance`). |
