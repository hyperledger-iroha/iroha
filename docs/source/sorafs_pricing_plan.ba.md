---
lang: ba
direction: ltr
source: docs/source/sorafs_pricing_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d08e44826c576ee6e9882d67b951c19fb3176350e7561dc4a3e449de392847
source_last_modified: "2025-12-29T18:16:36.176534+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Pricing Model & Credit Policy Plan (Draft)
summary: Outline for SF-8a pricing tiers, collateral, and settlement windows.
---

# SoraFS Pricing Model & Credit Policy Plan (Draft)

## Pricing Structure (Draft)

| Tier | Storage Price (XOR/GiB-month) | Egress Price (XOR/GB) | Notes |
|------|-------------------------------|------------------------|-------|
| Hot  | 0.50                          | 0.05                   | Low latency targets |
| Warm | 0.20                          | 0.02                   | Relaxed latency |
| Cold | 0.05                          | 0.01                   | Archival |

## Collateral & Bonds

- Provider collateral = 3x monthly storage earnings.
- Bonds held in XOR escrow; slashing triggered on sustained proof failures.
- Grace period for new providers (reduced collateral for first 30 days).

## Credit Policy

- Credits denominated in XOR; accounts maintain credit balance.
- Credit settlement window: weekly (7-day cycles).
- Automatic top-up threshold (alert when balance < 20% of expected weekly cost).
- Discount tiers for committed spend.

## Governance Reporting

- Monthly pricing review (Economics WG).
- Publish `pricing.json` manifest with tiers, effective dates.
- Provide historical pricing chart in docs.

## Economics Sign-off & Pricing Adjustments

- **Baseline rates** (as above) represent launch targets. Economics WG reviews market data monthly and can
  adjust `storage`/`egress` rates via governance proposal. Changes propagate to `pricing.json` manifest and
  the Reserve+Rent plan (reserve multipliers reevaluated accordingly).
- **Tier modifiers:** On high demand, introduce surge multipliers (e.g., `hot` ×1.2) with 7-day advance notice.
  Operators receive notification through Torii and dashboards.

## Currency Conversion & Dashboards

- Hedging service (see `sorafs_hedging_plan.md`) publishes XOR/USD rate. Dashboards show:
  - `effective_price_usd = storage_price_xor * xor_usd_rate`.
  - Historical chart combining rate + usage (per tier).
- Provide `GET /v1/pricing/current?currency=USD` API returning converted prices. CLI uses this for human-readable output.
- Finance dashboards integrate with hedging logs to reconcile XOR revenue vs USD equivalents.

## Promotional Credits & Incentives

- **Join bonus:** Governance allocates promotional credits for new providers. Disbursed via Norito envelope
  `PromoCreditGrantV1` with terms (amount, expiry, clawback conditions). Bonus reduces initial reserve requirements.
- **Loyalty discounts:** Operators committing to 12-month contracts receive `discount_multiplier` (e.g., 0.9) applied to storage price. Reflected in pricing manifest and billing statements.
- **Free tiers:** For developer/test accounts, optional `free_usage` quota (e.g., 50 GiB-month hot storage) tracked via billing pipeline.
- Promotional programs tracked in `pricing_promotions.toml` and surfaced in docs for transparency.
