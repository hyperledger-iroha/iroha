---
lang: ar
direction: rtl
source: docs/source/soranet_billing_m0.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 62a3cc35e474922cd7c3ff8a23b89f776a49f5a0aa24fdad56890fd5f7d46a81
source_last_modified: "2026-01-03T18:07:58.038711+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraGlobal Gateway Billing (SN15-M0-9)

The `soranet-gateway-billing-m0` helper ships the billing preview pack for the
SoraGlobal Gateway CDN. It keeps the meter catalog and billing artefacts
deterministic so PoP drills and governance packets cite the same evidence.

## Running the generator

```bash
cargo run -p xtask --bin xtask -- soranet-gateway-billing-m0 \
  --billing-period 2026-11 \
  --output-dir configs/soranet/gateway_m0/billing
```

Outputs:

- `billing_meter_catalog.json` + `billing_meter_catalog.{csv,parquet}` — six
  meters (HTTP requests/egress, DoH queries, WAF decisions, CAR verification,
  storage) with tiers, discounts, region multipliers, and burst caps.
- `billing_rating_plan.toml` — contract knobs (commit/prepay discounts, treasury
  skim, dispute hold) plus per-meter rate cards.
- `billing_ledger_hooks.toml` + `billing_ledger_projection.json` — receivable /
  revenue / escrow / treasury posting rules and a worked projection based on
  the sample usage bundle.
- `billing_guardrails.yaml` — per-meter caps and alert routes for promtool /
  Alertmanager validation.
- Invoice, dispute, and reconciliation templates kept in sync with the rating
  + ledger artefacts.
- `billing_m0_summary.json` — relative paths to every artefact for governance
  evidence packets.

## Notes

- Region multipliers and discounts are applied in the CSV/Parquet exports so
  billing reconcilers can diff against ledger entries without bespoke scripts.
- The ledger projection uses the M0 sample usage (requests, egress, DoH, WAF,
  CAR verification, storage) and applies commit + prepay discounts plus the
  dispute hold and treasury skim defined in the rating plan.
- Guardrails mirror `burst_cap` per meter; alerts cover usage spikes, egress
  growth, trustless verification lag, and dispute backlog.
