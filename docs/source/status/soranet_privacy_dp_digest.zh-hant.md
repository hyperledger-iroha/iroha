---
lang: zh-hant
direction: ltr
source: docs/source/status/soranet_privacy_dp_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 84068de9c3725d88a6b75b2342f2c20eb3dc3cfad37853e61ac5d1ef8130a0f7
source_last_modified: "2025-12-29T18:16:36.213452+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Privacy DP Governance Digest
summary: Weekly governance update on the SNNet-8 telemetry differential privacy calibration effort.
---

# Week of 2024-03-18

## Executive Summary

- Differential privacy artefacts (`summary.json`, `suppression_matrix.csv`, and accompanying notebook outputs) were generated deterministically via `scripts/telemetry/run_privacy_dp.py`.
- Calibrated ε spans 0.85–1.20 for the three guard roles under review, with suppression correctly engaging for contributor counts below the `min_contributors = 12` threshold.
- No regressions were observed in telemetry ingestion after replaying synthetic workloads through the privacy collector; metrics landed in Prometheus with the expected Laplace noise applied.

## Artefact Status

| Artefact | Owner | Status | Notes |
|----------|-------|--------|-------|
| `artifacts/soranet_privacy_dp/summary.json` | Telemetry | ✅ Published | Captures ε/δ, Laplace scale, and sensitivity for each guard role. Baseline entry relays emit ε = 1.20, δ = 5×10⁻⁶; exit relays stay within ε ≤ 0.85 following suppression. |
| `artifacts/soranet_privacy_dp/suppression_matrix.csv` | Telemetry | ✅ Published | Verifies suppression for contributor counts `< 12`; exit scenarios with 9 contributors stay suppressed. |
| `notebooks/soranet_privacy_dp.ipynb` | Telemetry | ✅ Published | Replays workloads, recomputes artefacts, and exports PNG tables for governance packet. |
| `scripts/telemetry/run_privacy_dp.py` | Tools | ✅ Published | Provides batch regeneration entrypoint; automation wrapper executes the notebook headlessly via `papermill`. |
| `scripts/telemetry/run_privacy_dp_notebook.sh` | Tools | ✅ Published | Runs the harness and notebook together; dispatched via `.github/workflows/release-pipeline.yml`. |

## Key Metrics

| Scenario | Role | Contributors | ε | δ | Suppressed | Notes |
|----------|------|--------------|---|---|------------|-------|
| entry-baseline | Entry guard | 18 | 1.20 | 5×10⁻⁶ | No | Meets governance cap ε ≤ 1.3 for entry relays. |
| middle-congested | Middle guard | 12 | 0.95 | 1×10⁻⁵ | No | Operates at the suppression threshold but remains visible. |
| exit-low-sample | Exit guard | 9 | 0.85 | 1×10⁻⁵ | Yes | Suppressed as required; aligns with Prometheus alert `soranet_privacy_rules`. |

## Risks & Mitigations

- **First workflow rollout** — The new release pipeline workflow executes the DP
  notebook on every promotion; monitor the initial runs to catch dependency
  regressions. *Mitigation*: archive logs/artifacts from the first execution and
  keep `papermill`/`jupyter` versions pinned if drift causes failures.
- **Operator onboarding** — New relays must inherit the deterministic seeds to observe consistent artefacts. *Mitigation*: documenting seed derivation in `scripts/telemetry/README.md` (in-progress) and confirming guard enrolment scripts set the required ENV vars.

## Decisions & Approvals

- DA WG validated the noise envelope against governance thresholds on 2024-03-19; no parameter adjustments requested.
- Telemetry WG approved publishing the suppression matrix to the operator portal; action item to expose alert drill-down dashboards is tracked separately (SNNet-8 DA-6).

## Upcoming Actions

1. Run `.github/workflows/release-pipeline.yml` for the next promotion and archive artefacts/logs — owner: Tools (due 2024-03-25).
2. Extend Prometheus rules to surface suppression streak lengths per lane — owner: Telemetry (due 2024-03-27).
3. Publish operator-facing FAQ summarising ε/δ impacts — owner: Governance (draft by 2024-03-29).
