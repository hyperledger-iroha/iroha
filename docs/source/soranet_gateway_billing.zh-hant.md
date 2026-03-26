---
lang: zh-hant
direction: ltr
source: docs/source/soranet_gateway_billing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da70a025bd187824807c5926a34e68c1a5daec0f3035bf31ced3a1d3f9a18252
source_last_modified: "2026-01-22T16:26:46.593903+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraGlobal Gateway Billing (SN15-M0-9)

This note documents the M0 billing catalog for the SoraGlobal Gateway CDN and
provides the tooling needed to rate usage, enforce guardrails, and emit ledger
projections. All amounts are denominated in **micro-XOR (1e-6 XOR)** unless
otherwise stated.

## Canonical artefacts
- Meter catalog: `configs/soranet/gateway_m0/meter_catalog.json`
- Guardrails: `configs/soranet/gateway_m0/billing_guardrails.json`
- Sample usage snapshot: `configs/soranet/gateway_m0/billing_usage_sample.json`
- Reconciliation template: `docs/examples/soranet_gateway_billing/reconciliation_report_template.md`

## Meter catalog
| Meter id | Unit | Base price (micro-XOR) | Region multipliers (bps) | Discount tiers (bps @ threshold) |
|----------|------|------------------------|--------------------------|----------------------------------|
| `http.request` | request | 5 | NA 10000, EU 9500, APAC 11000, LATAM 10250 | 500 @ 1,000,000; 900 @ 5,000,000 |
| `http.egress_gib` | gibibyte | 250000 | NA 10000, EU 9500, APAC 11000, LATAM 10250 | 300 @ 100; 1000 @ 300 |
| `dns.doh_query` | request | 3 | NA 10000, EU 9800, APAC 10500, LATAM 10000 | 1000 @ 2,000,000 |
| `waf.decision` | decision | 20 | GLOBAL 10000 | — |
| `car.verification_ms` | ms | 10 | GLOBAL 10000 | 1500 @ 500,000 |
| `storage.gib_month` | gib-month | 180000 | NA 10000, EU 9700, APAC 10300, LATAM 10000 | 500 @ 50; 1000 @ 200 |

Notes:
- Region multipliers and discounts are applied to the total line-item spend,
  not the per-unit price, to preserve determinism at high volumes.
- If a region is missing from the multiplier map the default of `10000` (1.0x)
  is used.

## Billing harness (`cargo xtask soranet-gateway-billing`)

Run the rating pipeline against a usage snapshot:

```
cargo xtask soranet-gateway-billing \
  --usage configs/soranet/gateway_m0/billing_usage_sample.json \
  --catalog configs/soranet/gateway_m0/meter_catalog.json \
  --guardrails configs/soranet/gateway_m0/billing_guardrails.json \
  --payer soraカタカナ... \
  --treasury soraカタカナ... \
  --asset 4cuvDVPuLBKJyN6dPbRQhmLh68sU \
  --out artifacts/soranet/gateway_billing
```

Outputs (all paths live under the `--out` directory):
- `meter_catalog.json` — copy of the catalog used for the run.
- `billing_usage_snapshot.json` — normalized usage input.
- `billing_invoice.json` — canonical Norito JSON invoice with guardrail verdicts.
- `billing_invoice.csv` — tabular line-item export.
- `billing_invoice.parquet` — Parquet line-item export (Arrow schema) for BI tooling.
- `billing_ledger_projection.json` — `TransferAssetBatch` targeting the XOR ledger
  with payer/treasury wiring and the total due.
- `billing_reconciliation_report.md` — pre-filled reconciliation/dispute report
  derived from the template and run metadata.
- Invoice metadata now includes `normalized_entries` (uppercased regions, merged
  duplicates) and `merge_notes` so reconciliation tools can diff usage merges.

Guardrail behaviour:
- Soft cap (default 140,000,000 micro-XOR) triggers an alert flag; hard cap
  (default 220,000,000 micro-XOR) blocks the ledger projection unless the
  caller explicitly continues.
- Minimum invoice floor (default 1,000,000 micro-XOR) prevents tiny invoices
  from being emitted.
- Unknown regions are rejected (catalog must enumerate the region multiplier);
  duplicate meter/region tuples are merged with uppercase regions and recorded
  in the invoice for audit replay.

## Dispute and reconciliation flow
1. Validate the usage snapshot against the catalog (meter ids, units, regions).
2. Confirm `billing_invoice.json` matches the CSV/Parquet exports.
3. Check guardrail flags and attach alert/override evidence when triggered.
4. Verify `billing_ledger_projection.json` total equals the invoice `total_micros`
   and references the expected payer/treasury accounts and asset definition.
5. Fill in `billing_reconciliation_report.md` (or start from the template in
   `docs/examples/soranet_gateway_billing/reconciliation_report_template.md`)
   with evidence links, approvals, and any requested adjustments.

## Rating and rounding rules
- Region multipliers and discount tiers are expressed in basis points (bps) and
  applied to the full line-item total (quantity × base price), rounded to the
  nearest micro-XOR.
- Discount tiers pick the highest applicable threshold per line.
- Amounts are stored and exported in micro-XOR; ledger projections convert the
  final total to `Numeric` with six decimal places before emitting the transfer
  batch.
