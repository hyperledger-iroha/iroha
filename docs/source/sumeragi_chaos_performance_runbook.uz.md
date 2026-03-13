---
lang: uz
direction: ltr
source: docs/source/sumeragi_chaos_performance_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a47a0821ea992aaa9a65e4e0875faa2ba22733560c1dde311f306dac073ee1e6
source_last_modified: "2026-01-22T14:48:32.765693+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sumeragi NPoS Chaos & Performance Validation Runbook

This runbook closes the Milestone A6 requirement in `roadmap.md` by
documenting how to execute the Sumeragi chaos/performance harness, capture
evidence, and wire the resulting telemetry dashboards/alerts. Pair it with
`docs/source/sumeragi.md`, the soak matrix (`docs/source/sumeragi_soak_matrix.md`),
and the A6 tracker in `docs/source/project_tracker/npos_sumeragi_phase_a.md`.

## 1. Scope & Success Criteria

- **Coverage.** Run the baseline throughput harness, targeted chaos tests, and
  the multi-peer soak matrix so every RBC, pacemaker, DA, and redundant-send
  path is exercised.
- **Evidence.** Produce refreshed Markdown/JSON reports, Grafana snapshots, and
  zipped artefact packs that can be attached to the GA sign-off packet.
- **Telemetry.** Ensure the dashboards fed by
  `docs/source/grafana_sumeragi_overview.json` and the alert rules in
  `docs/source/references/prometheus.rules.sumeragi_vrf.yml` are populated with
  current data. Alerts must fire (and be acknowledged) when thresholds are
  breached during fault injection.
- **Reporting.** Update `status.md` and the Phase A tracker with the artefact
  paths and dates after every full run.

## 2. Prerequisites

1. **Hardware.** Dedicated host with ≥16 physical cores, 64 GiB RAM, and fast
   local SSD. Disable background workloads that could mask pacemaker timing.
2. **Build profile.** `cargo build --profile release --workspace` (matching the
   commit you intend to certify) and ensure `iroha_cli`, `irohad`, and the
   integration test binaries are available.
3. **Environment.** Export `SUMERAGI_NPOS_STRESS_*` overrides only when testing
   bespoke matrices; the helpers set them automatically. Keep `RUST_LOG` at
   `info,iroha=debug`.
4. **Dashboards & alerts.** Import
   `docs/source/grafana_sumeragi_overview.json` into the staging Grafana, and
   deploy the Prometheus alert rules via
   `scripts/check_prometheus_rules.sh --apply` (or lint-only with `--lint`).
5. **Paths.** Use a dedicated artifact root (e.g.,
   `artifacts/sumeragi-a6-$(date +%Y%m%d-%H%M)`); all commands below assume the
   caller has write access to that directory.

## 3. Baseline Throughput & DA Harness

1. Execute the deterministic baseline run:

   ```bash
   SUMERAGI_BASELINE_ARTIFACT_DIR=artifacts/sumeragi-baseline-live \
     python3 scripts/run_sumeragi_baseline.py \
     --fail-on-fixture \
     --report-dest docs/source/generated/sumeragi_baseline_report.md
   ```

2. Capture the DA-specific metrics:

   ```bash
   python3 scripts/run_sumeragi_da.py \
     --artifacts artifacts/sumeragi-da-$(date +%Y%m%d-%H%M) \
     --report-dest docs/source/generated/sumeragi_da_report.md
   ```

3. Verify the reports now show the expected budgets listed in
   `docs/source/sumeragi.md` (RBC delivery ≤ 3.6 s, commit ≤ 4.0 s, throughput
   ≥ 2.7 MiB/s, queue depth ≤ 32, and zero P2P drops). Commit the regenerated
   Markdown to keep the public evidence bundle current.

## 4. Targeted Chaos & Stress Harness

Run each stress scenario individually so failures are easy to triage:

```bash
python3 scripts/run_sumeragi_stress.py \
  --artifacts artifacts/sumeragi-stress-$(date +%Y%m%d-%H%M)
```

The helper executes the tests in
`integration_tests/tests/sumeragi_npos_performance.rs`
(queue backpressure, RBC overflow, redundant send retries, jitter, chunk loss)
and appends pass/fail rows to `summary.json`. When a scenario fails:

1. Fetch the linked stdout/stderr logs from the artifact directory.
2. Correlate the timestamp with Grafana (phases, RBC backlog, redundant-send
   counts, VRF deadlines).
3. File a regression ticket referencing the scenario label and attach the logs.

Once green, render the Markdown summary:

```bash
python3 scripts/render_sumeragi_stress_report.py \
  --summary artifacts/sumeragi-stress-*/summary.json \
  --out docs/source/generated/sumeragi_stress_report.md
```

## 5. Multi-Peer Soak Matrix

Follow the soak matrix instructions verbatim; this step proves the stress
signals hold across realistic peer counts:

```bash
python3 scripts/run_sumeragi_soak_matrix.py \
  --artifacts-root artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M) \
  --pack artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M)/signoff.zip
```

Review `matrix_report.md` to ensure every scenario row shows `pass`. Store the
`.zip` bundle alongside the Grafana screenshots for auditability.

## 6. Telemetry & Alert Validation

During the runs above (especially the chaos suite), confirm:

- `sumeragi_phase_latency_ms` and `sumeragi_qc_assembly_latency_ms` stay within
  the configured pacemaker budgets.
- DA and RBC counters (`sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  legacy `sumeragi_rbc_da_reschedule_total`, `_backpressure_deferrals_total`,
  `_evictions_total`) match the fault you injected.
- Alert rules from `prometheus.rules.sumeragi_vrf.yml` fire once when their
  respective conditions trigger (e.g., `VRFNoParticipation`, `SumeragiCommitStall`).
  Capture the firing notification plus the acknowledgement screenshot.
- The Sumeragi Grafana overview dashboard shows the hardware, commit hash, and
  success counters for the current run.

Document alert firings and dashboard URLs in the artefact README for later
audits.

## 7. Evidence Packaging & Reporting

1. Gather the following into the run’s artifact directory:
   - `summary.json`, `matrix_report.{md,json}`, zipped soak pack.
   - Updated generated reports:
     `docs/source/generated/sumeragi_{baseline,da,stress}_report.md`.
   - Grafana snapshot `.json`/`.png` exports and alert acknowledgement logs.
2. Update `status.md` with a bullet summarising the run and linking to the
   artefact root.
3. Append the run entry to
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` so the tracker shows
   the fresh date/host/commit triple.
4. Share the ZIP + note with the SRE governance mailing list; archive the email
   under `docs/source/sdk/android/readiness/archive/` if it contains runbook
   decisions.

## 8. Failure Triage Checklist

When a scenario breaches its budget:

1. Consult the relevant section in `docs/source/sumeragi.md` (collector &
   witness telemetry, VRF pipeline, DA retry cadence) for the metric-driven
   triage flow.
2. Inspect `/v2/sumeragi/status` and `/v2/sumeragi/telemetry` using
   `iroha_cli --output-format text ops sumeragi status` for hot-path validation.
3. For DA-specific stalls, pair the logs with
   `docs/source/sumeragi_da.md` so operators can differentiate between RBC
   backlog and witness upload failures.
4. Record the remediation (tuning knobs, restarts, manifest fixes) in the
   incident record and mirror the root cause in `status.md`.

Following this runbook ensures every chaos and performance deliverable spelled
out in Milestone A6 has reproducible evidence, alert coverage, and clear
operator guidance.
