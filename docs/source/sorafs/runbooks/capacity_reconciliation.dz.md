---
lang: dz
direction: ltr
source: docs/source/sorafs/runbooks/capacity_reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a4467c6231b394713d2792e440159bff74606e2078d540c797150114bc7d1419
source_last_modified: "2025-12-29T18:16:36.126761+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Reconciliation Runbook
summary: Nightly workflow for reconciling capacity fee ledgers against XOR transfer exports.
---

# SoraFS Capacity Reconciliation Runbook

Roadmap item **SF-2c** requires treasury to prove that the capacity fee ledger
matches the XOR transfers executed each night. This runbook standardises that
reconciliation using the `capacity_reconcile.py` helper and the alert hook in
`dashboards/alerts/sorafs_capacity_rules.yml`.

## Prerequisites

- A capacity state snapshot captured from `/v1/sorafs/capacity/state` that
  includes `fee_ledger` entries.
- The XOR ledger export for the same window, encoded as JSON or NDJSON with
  `provider_id_hex`, `kind` (`settlement` or `penalty`), and `amount_nano`
  fields. The export should reflect the transfers treasury executed after
  reviewing the state snapshot.
- Access to the node_exporter textfile collector path if you want Prometheus
  alerts (`SORAFS_CAPACITY_RECONCILE_TEXTFILE` or explicit `--prom-out` path).

## Steps

1. **Export inputs**
   ```bash
   # Capacity snapshot (state API)
   curl -sS "$TORII/v1/sorafs/capacity/state" \
     > artifacts/sorafs/capacity/state_$(date +%F).json

   # Ledger export (after XOR transfers are executed)
   jq -c '.transfers[]' /var/lib/iroha/exports/sorafs-capacity-ledger.json \
     > artifacts/sorafs/capacity/ledger_$(date +%F).ndjson
   ```
2. **Run reconciliation**
   ```bash
   python3 scripts/telemetry/capacity_reconcile.py \
     --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
     --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
     --label nightly-capacity \
     --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
     --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
   ```
   The helper exits `0` when settlements and penalties match, `1` if any
   provider is missing/overpaid or if unexpected provider transfers appear, and
   `2` on input errors.
3. **Attach evidence** — archive the JSON summary, ledger export, state snapshot,
   and XOR transfer batch hashes inside the nightly treasury packet. Governance
   and SRE reviewers expect to see the labelled JSON summary in
   `docs/examples/sorafs_capacity_marketplace_validation/<date>_treasury_signoff.md`.
4. **Wire alerts** — if `--prom-out` writes into the node_exporter textfile
   collector, Alertmanager will evaluate `SoraFSCapacityReconciliationMismatch`
   from `dashboards/alerts/sorafs_capacity_rules.yml`. A non-zero missing,
   overpaid, or unexpected transfer count triggers a warning alert until the
   next reconciliation run succeeds.

## Output Fields

The JSON summary contains per-provider rows with `expected_settlement_nano`,
`actual_settlement_nano`, `expected_penalty_nano`, and diffs plus a `"status"`
label (`ok`, `settlement_missing`, `penalty_missing`, etc.). Totals mirror the
Prometheus gauges emitted in the `.prom` file:

- `sorafs_capacity_reconciliation_missing_total{kind}` — number of providers
  missing transfers by kind.
- `sorafs_capacity_reconciliation_overpaid_total{kind}` — providers that received
  more than the ledger expected.
- `sorafs_capacity_reconciliation_unexpected_transfers_total` — transfers for
  providers not present in the snapshot.
- `sorafs_capacity_reconciliation_expected_nano{kind}` and
  `sorafs_capacity_reconciliation_actual_nano{kind}` — aggregated XOR amounts.

Use these gauges to annotate Grafana tiles in `sorafs_capacity_health` or to
block treasury hand-off when reconciliation drifts.

## Expected Ranges and Tolerances

- Capacity reconciliation is strict: expected vs actual settlement/penalty nanos should match exactly (no rounding leeway). Alerts fire on any non-zero diff.
- CI pins a 30-day soak digest for the capacity fee ledger (test `capacity_fee_ledger_30_day_soak_deterministic`) to `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. If pricing/cooldown semantics change, rerun the test to refresh the digest and the runbook.
- Penalty/cooldown expectations: with `penalty_bond_bps=0` and `strike_threshold=u32::MAX` the soak keeps `penalty_events=0`. In production, expect penalties only when utilisation/uptime/PoR floors are breached and respect the configured cooldown before consecutive slashes.
- Treasury dashboards should treat any unexpected penalty events or reconciliation diffs as regressions and file an incident; the correct tolerance is zero for both counters and nanos.
