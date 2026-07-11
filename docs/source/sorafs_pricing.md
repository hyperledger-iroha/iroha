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

Tiers are keyed by `StorageClass` and are looked up for every telemetry window. The first-release
schedule must contain exactly one Hot, Warm, and Cold row in that canonical order; lookup never
falls back from one explicit class to another. If a provider or manifest does not specify a storage
class, admission selects the schedule’s `default_storage_class` (Hot) before the exact lookup.
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
less. `RecordCapacityTelemetry` also applies egress charges from `egress_bytes` through the active
pricing tier, adds the result to `expected_settlement_nano`, stores it in the capacity fee ledger,
and debits provider credit alongside the health-adjusted storage charge.

All multiplication/division uses an exact wide-intermediate algorithm. The runtime rounds storage,
collateral, health, and threshold ratios half-up and preserves the existing floor rule for byte-level
egress. A zero divisor, invalid schedule, future-to-past epoch transition, insufficient credit, or
final value outside `u128` rejects the transaction before any fee, credit, penalty, or telemetry
ledger field changes. Values are never saturated or silently clamped.

## Collateral & Bonds

The governance collateral policy enforces a minimum bond so that providers always have skin in the
game:

- Required collateral = `monthly_storage_fee × collateral_multiplier_bps / 10_000`.
- Launch multiplier is 30_000 bps (3× monthly storage earnings).
- New providers receive a 50 % discount during the first 30 days (onboarding period).

`required_collateral_nano` is recomputed every telemetry window and stored in the
`ProviderCreditRecord`; the associated `CapacityFeeLedgerEntry` retains the exact storage, egress,
settlement, and penalty totals used to audit that update.

## Credit Policy & Low-Balance Alerts

Provider credit accounts track prepaid balances used to settle nightly batches. The launch policy is
encoded in `CreditPolicy`:

- Settlement window: 7 days (`604 800` seconds).
- Grace period: 2 days (`172 800` seconds) past the settlement deadline.
- Low balance alert threshold: 20 % of the expected settlement fee (2 000 bps).

When telemetry is recorded the expected settlement charge for the next window is computed from the
pricing schedule and stored in both the fee ledger and the provider credit record. The deal engine
tracks `low_balance_since_epoch` when balances fall under the threshold so operators can top up
credit before settlement failure. A debit larger than the available balance is rejected atomically;
the runtime does not convert an unpayable debit into a zero balance and discard the remainder.

## Schedule Admission Rules

`SetPricingSchedule` accepts only the first-release `xor` currency and the complete canonical
Hot/Warm/Cold tier inventory. Storage and egress rates, settlement windows, onboarding periods, and
ticket-relevant thresholds must be positive. Onboarding, loyalty, commitment, and low-balance basis
points are bounded; commitment thresholds are strictly increasing, discounts are monotonic, and
stacked discounts cannot exceed 100%. Commitment tiers (64 maximum) and canonical control-free
governance notes (4 KiB maximum) are resource bounded. Settlement plus grace must fit in `u64`.

The manifest crate also provides a separate threshold-governed admission
foundation for future pricing services. `PricingTrustPolicyV1` binds strong
Ed25519 signer keys, threshold, revocations, currency, policy validity, and the
maximum future activation window. `GovernedPricingManifestV1` binds every
signature to the exact policy digest, pricing payload, and predecessor id.
`GovernedPricingSeriesV1` retains that exact chain in a bounded canonical
checkpoint, rejects replay, forks, clock rollback, policy substitution,
non-monotonic activation, and retroactive activation before admission, and
selects the active schedule deterministically by observation time. This library
state machine is also integrated into `sorafs_node`: operators may configure a
canonical `pricing_trust_policy_path`, after which the node admits only bounded
exact-canonical governed envelopes and persists the replay-validated series in
`economics/governed-pricing.to`. Mutations roll back on pre-commit persistence
failure and uncertain post-rename durability forces the node's durable mutation
surface fail-closed. Restart rejects missing, oversized, noncanonical, tampered,
or policy-substituted checkpoints, and the runtime exposes deterministic
active-price lookup. This local trusted boundary is not yet a replacement for
the on-chain `SetPricingSchedule` instruction and does not provide a daemon that
forwards accepted schedules on-chain. Torii exposes the boundary through
canonical-request-authenticated `POST /v1/sorafs/economics/pricing/manifests`,
`GET /v1/sorafs/economics/status`, and
`GET /v1/sorafs/economics/pricing/active`; every route additionally requires the
`sorafs_economics_operator` role, signs the exact method/URI/body, returns
private no-store responses, and rejects malformed, noncanonical, replayed,
forked, policy-substituted, or clock-rollback admissions without mutation.

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

   - Storage fee = `10 GiB × 500_000_000 nano × 2_592_000 / 2_592_000 = 5 000 000 000 nano-XOR`
     (5 XOR).
   - Required collateral = `5 000 000 000 × 30_000 / 10_000 = 15 000 000 000 nano-XOR` (15 XOR).
   - Expected settlement charge for the next window (7 days) = 1 166 666 667 nano-XOR (≈1.17 XOR).
   - Low-balance alert threshold = `1 166 666 667 × 0.2 = 233 333 333` nano-XOR.

2. **Warm tier, weekly window** — Utilisation averages 200 GiB over one settlement window (7 days)
   with 99 % uptime and 95 % PoR success.

   - Nominal charge = `200 × 0.20 XOR × 7 / 30 = 9.333… XOR` = `9 333 333 333` nano-XOR.
   - Health multiplier = `0.99 × 0.95 = 0.9405`, so charged storage fee = `8 775 000 000 nano-XOR`.
   - Expected settlement remains `9 333 333 333 nano-XOR`; collateral is still computed from the
     full monthly rate, not the health-adjusted fee.

## Operational Reconciliation

Egress billing is authoritative from `RecordCapacityTelemetry.egress_bytes`: it is charged through the pricing schedule, recorded in the capacity fee ledger, folded into expected settlement, and debited from provider credit. Telemetry submitters may also include optional `gateway_egress_bytes` and `orchestrator_egress_bytes` counters for the same pricing window. Torii records those as reconciliation-only metrics (`torii_sorafs_egress_bytes{source="gateway|orchestrator"}`) and computes `torii_sorafs_egress_drift_ratio` against the billing bytes.

Operator dashboards in `dashboards/grafana/sorafs_capacity_health.json` show billing, gateway, and orchestrator byte counters plus drift. Alert `SoraFSEgressCounterDrift` fires when a gateway or orchestrator source stays more than 10% away from billing for 10 minutes. Resolve drift by aligning the gateway/orchestrator export window with the signed capacity telemetry before approving settlement; do not override settlement from observer counters alone.
