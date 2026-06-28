---
lang: ja
direction: ltr
source: docs/source/sorafs_pricing_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e251ff6fe06e13dd96b63291b85257651fefd8afdb995da8465742a0d86d1c64
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Pricing Model & Credit Policy Status

This page records the SF-8a launch model. The active operator reference is
`docs/source/sorafs_pricing.md`, which maps these values to
`PricingScheduleRecord`, `SetPricingSchedule`, `RecordCapacityTelemetry`, the
capacity fee ledger, provider credit records, and egress reconciliation metrics.

## Pricing Structure

| Tier | Storage Price (XOR/GiB-month) | Egress Price (XOR/GB) | Notes |
|------|-------------------------------|------------------------|-------|
| Hot  | 0.50                          | 0.05                   | Low latency targets |
| Warm | 0.20                          | 0.02                   | Relaxed latency |
| Cold | 0.05                          | 0.01                   | Archival |

## Collateral & Bonds

- Provider collateral = 3x monthly storage earnings.
- Bonds held in XOR escrow; slashing triggered on sustained proof failures.
- Grace period for new providers (reduced collateral for first 30 days).
- Local implementation records the derived `required_collateral_nano` in both
  `CapacityFeeLedgerEntry` and `ProviderCreditRecord`.

## Credit Policy

- Credits denominated in XOR; accounts maintain credit balance.
- Credit settlement window: weekly (7-day cycles).
- Automatic top-up threshold (alert when balance < 20% of expected weekly cost).
- Discount tiers for committed spend.
- Provider credit accounts are updated through `UpsertProviderCredit`; telemetry
  windows debit storage plus egress charges from the active pricing schedule.

## Governance Reporting

- Monthly pricing review (Economics WG).
- Publish governance updates as `SetPricingSchedule` payloads with effective
  dates and archive the resulting ledger/credit snapshots.
- Capacity dashboards track storage, egress, drift, collateral, and proof-health
  settlement effects.

## Economics Sign-off & Pricing Adjustments

- **Baseline rates** (as above) represent launch defaults. Economics WG reviews market data monthly and can
  adjust `storage`/`egress` rates via governance proposal. Changes propagate to `pricing.json` manifest and
  the Reserve+Rent plan (reserve multipliers reevaluated accordingly).
- **Tier modifiers:** On high demand, introduce surge multipliers (e.g., `hot` ×1.2) with 7-day advance notice.
  Operators receive notification through Torii and dashboards.

## Currency Conversion & Dashboards

- Hedging service (see `sorafs_hedging_plan.md`) publishes XOR/USD rate. Dashboards show:
  - `effective_price_usd = storage_price_xor * xor_usd_rate`.
  - Historical chart combining rate + usage (per tier).
- Finance dashboards integrate with oracle/hedging logs to reconcile XOR revenue
  vs USD equivalents. Public converted-price APIs remain rollout/UI work unless
  explicitly enabled by governance.

## Promotional Credits & Incentives

- **Join bonus:** Governance allocates promotional credits for new providers. Disbursed via Norito envelope
  `PromoCreditGrantV1` with terms (amount, expiry, clawback conditions). Bonus reduces initial reserve requirements.
- **Loyalty discounts:** Operators committing to 12-month contracts receive `discount_multiplier` (e.g., 0.9) applied to storage price. Reflected in pricing manifest and billing statements.
- **Free tiers:** For developer/test accounts, optional `free_usage` quota (e.g., 50 GiB-month hot storage) tracked via billing pipeline.
- Promotional programs should be tracked in governed configuration and surfaced
  in docs for transparency. Promo-credit envelopes remain policy-level rollout
  work until their settlement path is explicitly enabled.
