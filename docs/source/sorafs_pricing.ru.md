---
lang: ru
direction: ltr
source: docs/source/sorafs_pricing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b30eef1206d7287de2c0719f0bfef3502a550af296f0989bf41079f75afaf376
source_last_modified: "2026-01-03T18:07:57.737132+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Pricing Schedule
summary: Default pricing tiers, collateral policy, and credit settlement rules for storage providers.
---

# SoraFS Pricing Schedule

This document captures the launch configuration for the SoraFS deal engine (roadmap item SF-8a).
It describes the default pricing schedule pushed by governance, how collateral is derived, and
how provider credit balances are monitored. All values are encoded on-chain via
`PricingScheduleRecord` and can be updated atomically using the `SetPricingSchedule` instruction.

## Default Pricing Tiers

All rates are denominated in nano-XOR (`1 XOR = 1_000_000_000 nano-XOR`). Storage pricing is billed
per GiB·month, egress is billed per logical GiB retrieved through gateways or the orchestrator.

| Tier | Storage price (XOR/GiB·month) | Egress price (XOR/GiB) | Typical usage |
|------|-------------------------------|-------------------------|---------------|
| Hot  | 0.50                          | 0.05                    | latency-sensitive content, developer flows |
| Warm | 0.20                          | 0.02                    | balanced durability/price, mainstream datasets |
| Cold | 0.05                          | 0.01                    | archival replicas, compliance retention |

Tiers are keyed by `StorageClass` and are looked up for every telemetry window. If a provider or
manifest does not specify a storage class, the schedule’s `default_storage_class` (Hot) is applied.
Capacity declarations may override the tier by adding the metadata entry `sorafs.storage_class`
(string value `hot`, `warm`, or `cold`) to the canonical declaration payload. The runtime mirrors
this entry into the `CapacityDeclarationRecord` and rejects mismatched out-of-band overrides.
Telemetry rejects unknown values and falls back to the schedule
default when the metadata is absent. Capacity telemetry submissions also include an `egress_bytes`
counter so the deal engine can apply the corresponding egress fees alongside storage charges.

### Storage charge calculation

Storage fees are proportional to both utilisation and window length. Let
`SECONDS_PER_BILLING_MONTH = 2_592_000` (30 days) and `rate_nano` be the tier storage rate in
nano-XOR. For a telemetry window of `window_secs` seconds with average utilisation `utilised_gib`
GiB, the nominal fee is:

```
storage_fee = utilised_gib × window_secs × rate_nano / SECONDS_PER_BILLING_MONTH
```

`RecordCapacityTelemetry` multiplies the nominal fee by the uptime and PoR success multipliers
(rounded to the nearest nano-XOR) so providers with degraded performance are charged proportionally
less. Egress fees follow the same pattern once egress telemetry is wired; until then they remain
zero and are reported separately for transparency.

## Collateral & Bonds

The governance collateral policy enforces a minimum bond so that providers always have skin in the
game:

- Required collateral = `monthly_storage_fee × collateral_multiplier_bps / 10_000`.
- Launch multiplier is 30_000 bps (3× monthly storage earnings).
- New providers receive a 50 % discount during the first 30 days (onboarding period).

`required_collateral_nano` is recomputed every telemetry window and stored in both the
`CapacityFeeLedgerEntry` and the `ProviderCreditRecord` so governance can audit bonds over time.

## Credit Policy & Low-Balance Alerts

Provider credit accounts track prepaid balances used to settle nightly batches. The launch policy is
encoded in `CreditPolicy`:

- Settlement window: 7 days (`604 800` seconds).
- Grace period: 2 days (`172 800` seconds) past the settlement deadline.
- Low balance alert threshold: 20 % of the expected settlement fee (2 000 bps).

When telemetry is recorded the expected settlement charge for the next window is computed from the
pricing schedule and stored in both the fee ledger and the provider credit record. The deal engine
tracks `low_balance_since_epoch` when balances fall under the threshold so operators can top up
credit before settlement failure.

## Provider Credit Ledger Fields

`ProviderCreditRecord` persists the runtime view of each provider’s credit state:

- `available_credit_nano`: spendable balance after debiting the latest telemetry fees.
- `bonded_nano`: currently bonded collateral.
- `required_bond_nano`: collateral requirement derived from the pricing schedule.
- `expected_settlement_nano`: projected debit for the next settlement window.
- `onboarding_epoch`: Unix epoch when the provider entered the programme (used for discounts).
- `last_settlement_epoch`: Unix epoch of the last debit applied.
- `low_balance_since_epoch`: optional Unix epoch when the balance first dipped below the alert
  threshold (cleared once balances recover).
- `metadata`: arbitrary annotations supplied by governance.

## Proof Failure Thresholds

`RecordCapacityTelemetry` now reports the proof-health counters required by roadmap item DA-5:

- `pdp_challenges` / `pdp_failures` capture the number of PDP challenges issued and the subset that
  failed verification during the telemetry window.
- `potr_windows` / `potr_breaches` capture the number of PoTR windows evaluated and the subset that
  breached latency/SLA guarantees.

Governance can tune the reaction via the extended `SorafsPenaltyPolicy`:

| Field | Default | Behaviour |
|-------|---------|-----------|
| `max_pdp_failures` | `0` | Maximum PDP failures tolerated per telemetry window before the runtime forces an under-delivery strike (0 = any failure triggers). |
| `max_potr_breaches` | `0` | Maximum PoTR SLA breaches tolerated per telemetry window before forcing a strike. |

When either threshold is exceeded the runtime immediately elevates the strike counter to the policy’s
`strike_threshold`, guaranteeing that the next fee application slashes collateral and logs the event
in the fee ledger. This “instant quarantine” path is independent of utilisation/uptime/PoR floors so
PDP/PoTR violations can be enforced even when capacity/uptime remain healthy.

Whenever an instant quarantine is triggered the ledger now emits
`SorafsGatewayEvent::ProofHealth`, exposing the provider identifier, telemetry window, PDP/PoTR
counts, configured thresholds, strike bookkeeping, cooldown status, and the exact slashed amount (if
any). Consumers of Torii’s `/v1/events/sse` feed can subscribe to the `Sorafs` channel to export the
alerts into governance evidence stores or realtime telemetry dashboards without scraping ledger
state.

## Governance Interface

Governance (or authorised operations tooling) manages the schedule and credit accounts via the
following instructions:

- `SetPricingSchedule` replaces the on-chain `PricingScheduleRecord` after validation. The new
  schedule applies to the next telemetry window processed by the deal engine.
- `RecordCapacityTelemetry` consumes provider telemetry, calculates storage fees using the current
  schedule, applies uptime/PoR multipliers, updates the fee ledger, and debits provider credit
  accounts when present.
- `UpsertProviderCredit` seeds or updates the governance view of a provider’s credit record (e.g.,
  after manual top-ups or collateral adjustments). Providers must have a registered capacity
  declaration before a credit record can be inserted.

## Worked Examples

1. **Hot tier, full billing month** — A provider stores 10 GiB in the Hot tier for an entire billing
   month with perfect uptime and PoR performance.

   - Storage fee = `10 GiB × 1_000_000_000 nano × 2_592_000 / 2_592_000 = 10 000 000 000 nano-XOR`
     (10 XOR).
   - Required collateral = `10 000 000 000 × 30_000 / 10_000 = 30 000 000 000 nano-XOR` (30 XOR).
   - Expected settlement charge for the next window (7 days) = 2 333 333 333 nano-XOR (≈2.33 XOR).
   - Low-balance alert threshold = `2 333 333 333 × 0.2 = 466 666 666` nano-XOR.

2. **Warm tier, weekly window** — Utilisation averages 200 GiB over one settlement window (7 days)
   with 99 % uptime and 95 % PoR success.

   - Nominal charge = `200 × 0.20 XOR × 7 / 30 = 9.333… XOR` = `9 333 333 333` nano-XOR.
   - Health multiplier = `0.99 × 0.95 = 0.9405`, so charged storage fee = `8 775 000 000 nano-XOR`.
   - Expected settlement remains `9 333 333 333 nano-XOR`; collateral is still computed from the
     full monthly rate, not the health-adjusted fee.

## Future Work

Egress telemetry is currently wired through the data model but not yet emitted by storage workers.
Once the orchestrator and gateways report logical bytes served, the same pricing schedule will
apply automatically and egress fees will populate the ledger and credit records alongside storage
fees.
