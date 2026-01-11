# Trigger-Based Subscription API (Design)

This document defines a simple, real-world subscription model built on existing
Iroha primitives: AssetDefinitions, NFTs, metadata, and time triggers. It covers
both usage billing in arrears (e.g., cloud usage billed on the 1st) and fixed
advance billing (e.g., a flat monthly plan).

The model is application-layer: no new core data-model primitives are required.
All logic must remain deterministic across peers and hardware.

## Goals
- Support usage billing in arrears (bill the previous period on the first).
- Support fixed-price subscriptions billed in advance.
- Keep the on-chain model simple: one plan, one subscription record, one trigger.
- Keep computation deterministic (UTC time, integer math, no floats).

## Data Model Mapping
- Plan: AssetDefinition with metadata describing billing cadence and pricing.
- Subscription: NFT owned by the subscriber, storing live billing state.
- Billing trigger: time trigger that executes a shared IVM billing contract.
- Usage accumulator: subscription metadata updated by a by-call contract.
- Optional invoice: `subscription_invoice` metadata stored on the subscription NFT for the latest attempt (apps can mint separate invoice NFTs if they need a full history).

## Canonical Metadata Schemas (Norito JSON)
The schema types are defined in `crates/iroha_data_model/src/subscription.rs` and
match the JSON layouts below (Norito JSON).

### Plan (AssetDefinition metadata key: `subscription_plan`)
```json
{
  "subscription_plan": {
    "provider": "acme@commerce",
    "billing": {
      "cadence": {
        "kind": "monthly_calendar",
        "detail": {
          "anchor_day": 1,
          "anchor_time_ms": 0
        }
      },
      "bill_for": { "period": "previous_period", "value": null },
      "retry_backoff_ms": 86400000,
      "max_failures": 3,
      "grace_ms": 604800000
    },
    "pricing": {
      "kind": "usage",
      "detail": {
        "unit_price": "0.024",
        "unit_key": "compute_ms",
        "asset_definition": "usd#pay"
      }
    }
  }
}
```

Fixed-price plans use:
```json
{
  "subscription_plan": {
    "provider": "openai@commerce",
    "billing": {
      "cadence": {
        "kind": "monthly_calendar",
        "detail": {
          "anchor_day": 1,
          "anchor_time_ms": 0
        }
      },
      "bill_for": { "period": "next_period", "value": null },
      "retry_backoff_ms": 86400000,
      "max_failures": 3,
      "grace_ms": 604800000
    },
    "pricing": {
      "kind": "fixed",
      "detail": {
        "amount": "120",
        "asset_definition": "usd#pay"
      }
    }
  }
}
```

### Subscription (NFT content key: `subscription`)
```json
{
  "subscription": {
    "plan_id": "aws_compute#commerce",
    "provider": "aws@commerce",
    "subscriber": "alice@users",
    "status": { "status": "active", "value": null },
    "current_period_start_ms": 1730419200000,
    "current_period_end_ms": 1733011200000,
    "next_charge_ms": 1733011200000,
    "failure_count": 0,
    "usage_accumulated": {
      "compute_ms": "129600000"
    },
    "billing_trigger_id": "sub-6f3a9c"
  }
}
```

### Billing trigger (Trigger action metadata key: `subscription_ref`)
Schema type: `iroha_data_model::subscription::SubscriptionTriggerRef`.
```json
{
  "subscription_ref": {
    "subscription_nft_id": "sub-6f3a9c$subscriptions"
  }
}
```

### Optional invoice (subscription NFT content key: `subscription_invoice`)
The billing syscall updates the subscription NFT with the latest invoice attempt.
```json
{
  "subscription_invoice": {
    "subscription_nft_id": "sub-6f3a9c$subscriptions",
    "period_start_ms": 1730419200000,
    "period_end_ms": 1733011200000,
    "attempted_at_ms": 1733011200000,
    "amount": "31.104",
    "asset_definition": "usd#pay",
    "status": { "status": "paid", "value": null },
    "tx_hash": "..."
  }
}
```
`tx_hash` is optional; `subscription_bill()` leaves it null so off-chain indexers can populate it later.

## Cadence and Period Rules (Deterministic UTC)
Norito tagged enums encode unit variants as `{ <tag>: <variant>, value: null }` and data variants as
`{ kind: <variant>, detail: { ... } }`.

`cadence.kind` options:
- `monthly_calendar`: true calendar months (UTC).
- `fixed_period`: fixed millis period (e.g., 30 days) for simplified plans.

Monthly calendar rules:
- Use UTC only; no locale time zones.
- If `anchor_day` is greater than the number of days in the month, clamp to
  the last day of the month.
- `anchor_time_ms` is the time-of-day in milliseconds from 00:00 UTC.
- Period boundaries are derived from calendar month starts with the anchor.
- Use `iroha_primitives::calendar` helpers to keep month math deterministic.

Period semantics:
- `bill_for.period = previous_period`:
  - Charge for `[current_period_start_ms, current_period_end_ms)`.
  - Charge time is `next_charge_ms` which equals the start of the next period.
- `bill_for.period = next_period`:
  - Charge for the upcoming period `[next_start, next_end)`.
  - On success, set `current_period_*` to that next period.

## Billing Trigger Execution (IVM contract)
The billing trigger runs a shared IVM bytecode contract with these steps:
1. Load subscription NFT and plan metadata.
2. Reject `paused/canceled/suspended` subscriptions; allow `active` and `past_due` billing retries.
3. Compute the billable period based on `bill_for` and cadence.
4. Compute amount:
   - `pricing.kind = fixed`: amount from `pricing.detail.amount`.
   - `pricing.kind = usage`: `usage_accumulated[pricing.detail.unit_key] * pricing.detail.unit_price`.
5. Transfer `amount` of `asset_definition` from subscriber to provider.
6. On success:
   - Reset usage accumulator (if usage-based).
   - Advance period boundaries and `next_charge_ms`.
   - Set `failure_count = 0`.
   - Update `subscription_invoice` on the subscription NFT with `status = paid`.
7. On failure:
   - Increment `failure_count`.
   - Set `status = past_due` if beyond `grace_ms`.
   - Reschedule `next_charge_ms = now + retry_backoff_ms`.
   - Update `subscription_invoice` on the subscription NFT with `status = failed`.
8. If `failure_count > max_failures`, set `status = suspended` and stop.

Implementation note: the shared contract is a thin wrapper around IVM helpers
(`subscription_bill()`), which uses trigger metadata `subscription_ref` and
Norito-encoded subscription metadata to perform deterministic billing, retries,
and trigger rescheduling without leaking non-determinism into bytecode.
Sample Kotodama wrappers live in
`crates/ivm/src/kotodama/samples/subscription_billing_trigger.ko` and
`crates/ivm/src/kotodama/samples/subscription_usage_recorder.ko`.

## Usage Recording (Simple and Permissioned)
Usage is recorded by a by-call trigger or a small IVM contract:
- Input: subscription NFT id, `unit_key`, `delta`.
- Action: increment `usage_accumulated[unit_key]` deterministically.
- Authorization: provider account must hold `CanExecuteTrigger` for the usage
  recorder trigger owned by the subscriber.

### Usage recording payload (trigger args)
Schema type: `iroha_data_model::subscription::SubscriptionUsageDelta`.
```json
{
  "subscription_nft_id": "sub-6f3a9c$subscriptions",
  "unit_key": "compute_ms",
  "delta": "3600000"
}
```

The usage recorder contract calls `subscription_record_usage()` which parses
the args payload and updates the subscription metadata.

## Authority Model (Simple Default)
Default:
- Billing trigger authority = subscriber.
- Usage recorder trigger authority = subscriber; provider granted
  `CanExecuteTrigger` to record usage.

Alternative:
- Billing agent account owns the billing trigger and receives explicit
  `CanTransferAsset` grants from the subscriber. Use only if needed.

## Torii API Surface (Proposed)
- `POST /v1/subscriptions/plans` - register plan (AssetDefinition metadata).
- `GET /v1/subscriptions/plans` - list plans by provider.
- `POST /v1/subscriptions` - create subscription + billing trigger.
- `GET /v1/subscriptions` - list subscriptions owned by requester.
- `GET /v1/subscriptions/{id}` - fetch one subscription.
- `POST /v1/subscriptions/{id}/pause` - set `status=paused`, cancel triggers.
- `POST /v1/subscriptions/{id}/resume` - set `status=active`, re-schedule.
- `POST /v1/subscriptions/{id}/cancel` - set `status=canceled`, unregister trigger.
- `POST /v1/subscriptions/{id}/usage` - record usage (by-call trigger).
- `POST /v1/subscriptions/{id}/charge-now` - execute billing immediately.

## Query Patterns
- List subscriptions for an account:
  - `FindNfts` with predicate `owned_by == <account>` and `exists("content.subscription")`.
- List plans for a provider:
  - `FindAssetsDefinitions` with predicate `metadata.subscription_plan.provider == <account>`.

## Events and Telemetry
- Use `TriggerCompletedEvent` to observe billing successes/failures.
- Use `ExecuteTriggerEvent` for usage recorder invocations.
- Emit subscription-specific counters in telemetry as follow-up work.

## Examples

### Usage billing in arrears (AWS-style)
- Plan: `monthly_calendar`, `bill_for = previous_period`.
- Usage accumulates during November.
- Billing trigger fires Dec 1 00:00 UTC and charges for November usage.

### Fixed monthly in advance (ChatGPT-style)
- Plan: `monthly_calendar`, `bill_for = next_period`, `amount = 120`.
- Trigger fires on the first and charges for the upcoming month.

## Determinism Checklist
- All times are UTC; month math is pure integer logic.
- Amounts are `Numeric` strings; no floating point arithmetic.
- No environment toggles; all behavior is configured via metadata and config.
