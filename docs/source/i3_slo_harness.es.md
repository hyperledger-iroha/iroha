---
lang: es
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-01-30
---

% Iroha 3 SLO Harness

The Iroha 3 release line carries explicit SLOs for the critical Nexus paths:

- finality slot duration (NX‑18 cadence)
- proof verification (commit certs, JDG attestations, bridge proofs)
- proof endpoint handling (Axum path proxy via verification latency)
- fee and staking paths (payer/sponsor and bond/slash flows)

## Budgets

Budgets live in `benchmarks/i3/slo_budgets.json` and map directly to the bench
scenarios in the I3 suite. Objectives are per‑call p99 targets:

- Fee/staking: 50 ms per call (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Commit cert / JDG / bridge verify: 80 ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Commit cert assembly: 80 ms (`commit_cert_assembly`)
- Access scheduler: 50 ms (`access_scheduler`)
- Proof endpoint proxy: 120 ms (`torii_proof_endpoint`)

Burn-rate hints (`burn_rate_fast`/`burn_rate_slow`) encode the 14.4/6.0
multi-window ratios for paging vs. ticket alerts.

## Harness

Run the harness via `cargo xtask i3-slo-harness`:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Outputs:

- `bench_report.json|csv|md` — raw I3 bench suite results (git hash + scenarios)
- `slo_report.json|md` — SLO evaluation with pass/fail/budget-ratio per target

The harness consumes the budgets file and enforces `benchmarks/i3/slo_thresholds.json`
during the bench run to fail fast when a target regresses.

## Telemetry and dashboards

- Finality: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Proof verification: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana starter panels live in `dashboards/grafana/i3_slo.json`. Prometheus
burn-rate alerts are provided in `dashboards/alerts/i3_slo_burn.yml` with the
budgets above baked in (finality 2s, proof verify 80 ms, proof endpoint proxy
120 ms).

## Operational notes

- Run the harness in nightlies; publish `artifacts/i3_slo/<stamp>/slo_report.md`
  alongside the bench artefacts for governance evidence.
- If a budget fails, use the bench markdown to identify the scenario, then drill
  into the matching Grafana panel/alert to correlate with live metrics.
- Proof endpoint SLOs use the verification latency as a proxy to avoid per-route
  cardinality blowup; the benchmark target (120 ms) matches the retention/DoS
  guardrails on the proof API.
