# Reconciliation Checklist (2026-11)

1. Pull 2026-11 meter snapshots from the billing spool.
2. Recompute charges using billing_meter_catalog.json + billing_rating_plan.toml (bps multipliers and tiers are deterministic); confirm CSV/Parquet exports match the JSON view.
3. Validate ledger postings using billing_ledger_hooks.toml and billing_ledger_projection.json (invoice -> payment -> dispute hold/release -> refund/treasury skim).
4. Compare Alertmanager exports against billing_guardrails.yaml to confirm caps and warnings fired correctly.
5. Attach invoice and dispute templates plus any credit memos; publish evidence to artifacts/soranet/gateway_m0/billing/<period>/.
