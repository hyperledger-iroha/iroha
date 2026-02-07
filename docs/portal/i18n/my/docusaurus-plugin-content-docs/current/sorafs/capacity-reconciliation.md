---
id: capacity-reconciliation
lang: my
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Reconciliation
description: Nightly workflow for matching capacity fee ledgers to XOR transfer exports.
---

Roadmap item **SF-2c** mandates that treasury proves the capacity fee ledger
matches the XOR transfers executed each night. Use the
`scripts/telemetry/capacity_reconcile.py` helper to compare the
`/v1/sorafs/capacity/state` snapshot against the executed transfer batch and
emit Prometheus textfile metrics for Alertmanager.

## Prerequisites
- Capacity state snapshot (`fee_ledger` entries) exported from Torii.
- Ledger export for the same window (JSON or NDJSON with `provider_id_hex`,
  `kind` = settlement/penalty, and `amount_nano`).
- Path to the node_exporter textfile collector if you want alerts.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Exit codes: `0` on a clean match, `1` when settlements/penalties are missing
  or overpaid, `2` on invalid inputs.
- Attach the JSON summary + hashes to the treasury packet in
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- When the `.prom` file lands in the textfile collector, the alert
  `SoraFSCapacityReconciliationMismatch` (see
  `dashboards/alerts/sorafs_capacity_rules.yml`) fires whenever missing,
  overpaid, or unexpected provider transfers are detected.

## Outputs
- Per-provider statuses with diffs for settlements and penalties.
- Totals exported as gauges:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Expected Ranges and Tolerances
- Reconciliation is exact: expected vs actual settlement/penalty nanos should match with zero tolerance. Any non-zero diff should page operators.
- CI pins a 30-day soak digest for the capacity fee ledger (test `capacity_fee_ledger_30_day_soak_deterministic`) to `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. Refresh the digest only when pricing or cooldown semantics change.
- In the soak profile (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) penalties stay at zero; production should only emit penalties when utilisation/uptime/PoR floors are breached and respect the configured cooldown before successive slashes.
