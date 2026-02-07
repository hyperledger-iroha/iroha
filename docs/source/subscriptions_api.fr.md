---
lang: fr
direction: ltr
source: docs/source/subscriptions_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ab71fd3a1aa167891f7a3e774a15a9464967bf6489b63939e11aff843bd3ea2
source_last_modified: "2026-01-22T16:26:46.594446+00:00"
translation_last_reviewed: 2026-02-07
---

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
    "provider": "ih58...",
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
    "provider": "ih58...",
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
    "provider": "ih58...",
    "subscriber": "ih58...",
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
`crates/kotodama_lang/src/samples/subscription_billing_trigger.ko` and
`crates/kotodama_lang/src/samples/subscription_usage_recorder.ko`.

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

## Torii API Surface
- `POST /v1/subscriptions/plans` - register plan (AssetDefinition metadata).
- `GET /v1/subscriptions/plans` - list plans by provider.
- `POST /v1/subscriptions` - create subscription + billing trigger.
- `GET /v1/subscriptions` - list subscriptions with optional filters.
- `GET /v1/subscriptions/{subscription_id}` - fetch one subscription.
- `POST /v1/subscriptions/{subscription_id}/pause` - set `status=paused`, cancel triggers.
- `POST /v1/subscriptions/{subscription_id}/resume` - set `status=active`, re-schedule.
- `POST /v1/subscriptions/{subscription_id}/cancel` - set `status=canceled`, unregister trigger.
- `POST /v1/subscriptions/{subscription_id}/usage` - record usage (by-call trigger).
- `POST /v1/subscriptions/{subscription_id}/charge-now` - execute billing immediately.

### POST /v1/subscriptions/plans
Registers a plan on an asset definition. `authority` must match `plan.provider`.
```json
{
  "authority": "ih58...",
  "private_key": "<hex>",
  "plan_id": "aws_compute#subscriptions",
  "plan": { "provider": "ih58...", "billing": { "...": "..." }, "pricing": { "...": "..." } }
}
```
Response:
```json
{ "ok": true, "plan_id": "aws_compute#subscriptions", "tx_hash_hex": "<hex>" }
```

### GET /v1/subscriptions/plans
Query params:
- `provider` (optional) - filter by provider account id.
- `limit`, `offset` (optional) - pagination.
Response:
```json
{ "items": [ { "plan_id": "...", "plan": { "...": "..." } } ], "total": 1 }
```

### POST /v1/subscriptions
Creates a subscription NFT and billing trigger. `authority` must be the subscriber (NFT owner).
```json
{
  "authority": "ih58...",
  "private_key": "<hex>",
  "subscription_id": "sub-6f3a9c$subscriptions",
  "plan_id": "aws_compute#subscriptions",
  "billing_trigger_id": "optional",
  "usage_trigger_id": "optional",
  "first_charge_ms": 1704067200000,
  "grant_usage_to_provider": true
}
```
Defaults:
- `billing_trigger_id` and `usage_trigger_id` are derived when omitted using
  `sub_bill_<hash>` and `sub_usage_<hash>` (BLAKE2b of the subscription id).
- `first_charge_ms` defaults to the next cadence boundary:
  - Monthly cadence: next anchor at or after the current network time.
  - Fixed period: `now` for `bill_for=next_period`, `now + period_ms` for `previous_period`.
- `grant_usage_to_provider` defaults to `true` (usage plans only).
Response:
```json
{
  "ok": true,
  "subscription_id": "sub-6f3a9c$subscriptions",
  "billing_trigger_id": "sub_bill_<hash>",
  "usage_trigger_id": "sub_usage_<hash>",
  "first_charge_ms": 1704067200000,
  "tx_hash_hex": "<hex>"
}
```

### GET /v1/subscriptions
Query params:
- `owned_by` (optional) - filter by subscriber account id.
- `provider` (optional) - filter by provider account id.
- `status` (optional) - one of `active`, `paused`, `past_due`, `canceled`, `suspended`.
- `limit`, `offset` (optional) - pagination.
Response:
```json
{
  "items": [
    {
      "subscription_id": "sub-6f3a9c$subscriptions",
      "subscription": { "...": "..." },
      "invoice": { "...": "..." },
      "plan": { "...": "..." }
    }
  ],
  "total": 1
}
```

### GET /v1/subscriptions/{subscription_id}
Returns the subscription state, latest invoice (if any), and plan metadata (if present).

### POST /v1/subscriptions/{subscription_id}/pause
```json
{ "authority": "ih58...", "private_key": "<hex>" }
```
Sets `status=paused` and unregisters the billing trigger.

### POST /v1/subscriptions/{subscription_id}/resume
```json
{ "authority": "ih58...", "private_key": "<hex>", "charge_at_ms": 1704067200000 }
```
Sets `status=active`, resets `failure_count`, recomputes the current period, and re-schedules billing.
`charge_at_ms` follows the same defaults as `first_charge_ms` when omitted.

### POST /v1/subscriptions/{subscription_id}/cancel
```json
{ "authority": "ih58...", "private_key": "<hex>", "cancel_mode": "immediate" }
```
or
```json
{ "authority": "ih58...", "private_key": "<hex>", "cancel_mode": "period_end" }
```
`cancel_mode=immediate` sets `status=canceled` and unregisters the billing trigger.
`cancel_mode=period_end` keeps the subscription active until the current period ends, then stops
future billing without charging the next period.

### POST /v1/subscriptions/{subscription_id}/keep
```json
{ "authority": "ih58...", "private_key": "<hex>" }
```
Clears `cancel_at_period_end` and keeps the subscription active for future billing cycles. Returns
an error if the subscription is not scheduled to cancel at period end.

### POST /v1/subscriptions/{subscription_id}/usage
```json
{
  "authority": "ih58...",
  "private_key": "<hex>",
  "unit_key": "compute_ms",
  "delta": "3600000",
  "usage_trigger_id": "optional"
}
```
Executes the usage trigger with `SubscriptionUsageDelta`. `delta` must be non-negative.

### POST /v1/subscriptions/{subscription_id}/charge-now
```json
{ "authority": "ih58...", "private_key": "<hex>", "charge_at_ms": 1704067200000 }
```
Updates `next_charge_ms` and re-registers the billing trigger to execute at `charge_at_ms`
(defaults to current network time when omitted).

## CLI Helpers
The CLI mirrors the Torii endpoints for plan and subscription management.

Register a plan from a JSON file (or stdin when `--plan-json` is omitted):
```bash
iroha_cli subscriptions plan create \
  --authority ih58... \
  --private-key <hex> \
  --plan-id aws_compute#commerce \
  --plan-json plan.json
```

List plans for a provider:
```bash
iroha_cli subscriptions plan list --provider ih58... --limit 10
```

Create a subscription:
```bash
iroha_cli subscriptions subscription create \
  --authority ih58... \
  --private-key <hex> \
  --subscription-id sub-001$subscriptions \
  --plan-id aws_compute#commerce
```

Pause, resume, cancel, or charge now:
```bash
iroha_cli subscriptions subscription pause --subscription-id sub-001$subscriptions \
  --authority ih58... --private-key <hex>
iroha_cli subscriptions subscription resume --subscription-id sub-001$subscriptions \
  --authority ih58... --private-key <hex>
iroha_cli subscriptions subscription cancel --subscription-id sub-001$subscriptions \
  --authority ih58... --private-key <hex> --cancel-at-period-end
iroha_cli subscriptions subscription keep --subscription-id sub-001$subscriptions \
  --authority ih58... --private-key <hex>
iroha_cli subscriptions subscription charge-now --subscription-id sub-001$subscriptions \
  --authority ih58... --private-key <hex>
```

Record usage:
```bash
iroha_cli subscriptions subscription usage --subscription-id sub-001$subscriptions \
  --authority ih58... --private-key <hex> \
  --unit-key compute_ms --delta 3600000
```

## Query Patterns
- List subscriptions for an account:
  - `FindNfts` with predicate `owned_by == <account>` and `exists("content.subscription")`.
- List plans for a provider:
  - `FindAssetsDefinitions` with predicate `metadata.subscription_plan.provider == <account>`.

## Events and Telemetry
- Billing trigger completion events: subscribe to `TriggerCompletedEvent` and filter by the billing trigger id for a subscription.
  - Rust helper: `TriggerCompletedEventFilter::new().for_trigger(trigger_id)`
- Usage recorder events: `ExecuteTriggerEvent` for usage trigger invocations.
- Prometheus metrics:
  - `iroha_subscription_billing_attempts_total{pricing="fixed|usage"}`
  - `iroha_subscription_billing_outcomes_total{pricing="fixed|usage",result="paid|failed|suspended|skipped"}`

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
