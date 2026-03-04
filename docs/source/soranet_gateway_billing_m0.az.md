---
lang: az
direction: ltr
source: docs/source/soranet_gateway_billing_m0.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ab232fba530de14a767bf1832bc00e19a4c82e6ca431ef5c875a9f62c321509d
source_last_modified: "2025-12-29T18:16:36.205271+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraGlobal Gateway Billing M0 Pack

SN15-M0-9 captures the billing preview artifacts used during early PoP drills.
The `soranet-gateway-billing-m0` helper writes a deterministic bundle so
treasury/ops/governance teams can attach the same evidence to tickets and
rehearsals.

## Usage

```
cargo xtask soranet-gateway-billing-m0 \
  --billing-period 2026-11 \
  --output-dir artifacts/soranet/gateway_m0/billing_demo
```

Defaults:
- Billing period: `2026-11`
- Output directory: `configs/soranet/gateway_m0/billing/`

## Bundle contents

- `billing_meter_catalog.json` — meter ids/units, region multipliers, and caps.
- `billing_rating_plan.toml` — rating/discount/tier rules (bps, micros).
- `billing_ledger_hooks.toml` — receivable/revenue/escrow/refund posting rules.
- `billing_guardrails.yaml` — alert thresholds for caps, disputes, verification lag.
- Templates: `billing_invoice_template.md`, `billing_dispute_template.md`,
  `billing_reconciliation_template.md`.
- `billing_m0_summary.json` — relative paths to all files for quick attachment.

Keep the bundle unchanged for M0 drills; regenerate with `--output-dir` to
produce environment-specific copies without touching the canonical defaults in
`configs/soranet/gateway_m0/billing/`.
