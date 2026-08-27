% NPoS Sumeragi Phase A Tracker Stub (Dec 2025)

> **Archived milestone record:** The rows below preserve the retired 2025
> consensus-VRF/global-RBC plan and its historical completion notes; they are
> not first-release protocol or release evidence. Current NPoS randomness uses
> finalized global threshold-BLS pulses, and revision-4 DA uses signed RS16
> manifests/chunks. See `specs/sumeragi_v2.md` and
> `specs/sumeragi_randomness_evidence_runbook.md`.

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
| GA-A6.2 | A6 — Tooling, Docs, and Validation | Capture NPoS performance baseline (1 s blocks, k=3) and record metrics in `status.md`/operator docs. | `@performance-lab`, `@telemetry-ops` | GA-A6.1 | 2026-04-21 | Completed — Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0); see `specs/generated/sumeragi_baseline_report.md`. |
| GA-A6.3 | A6 — Tooling, Docs, and Validation | Publish operator troubleshooting guides for RBC/pacemaker/backpressure instrumentation. | `@operator-docs`, `@telemetry-ops` | GA-A6.1 | 2026-04-28 | Completed — troubleshooting runbook added in `specs/telemetry.md:523`; automated log correlation now ships via `scripts/sumeragi_backpressure_log_scraper.py` so operators can pull deferral/RBC pairings without manual grepping. |
| GA-A6.4 | A6 — Tooling, Docs, and Validation | Exercise revision-4 baseline and transaction-queue pressure; exact protocol-message faults use the authenticated Sumeragi v2 runner. | `@performance-lab`, `@telemetry-ops` | GA-A6.2 | 2026-05-05 | Completed — the stress launcher runs `npos_baseline_1s_captures_metrics` and `npos_queue_backpressure_triggers_metrics`; retired V1 RBC store/chunk config scenarios are no longer release evidence. |
