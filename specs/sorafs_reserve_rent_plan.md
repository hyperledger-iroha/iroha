---
title: SoraFS Reserve+Rent V1
summary: Canonical chain-authoritative reserve, rent, credit, lifecycle, and appeal contract for SoraFS V1.
---

# SoraFS Reserve+Rent V1

## Release contract

SoraFS V1 treats the native reserve ledger as the only authority for provider
underwriting, custody, rent settlement, credit, lifecycle state, and appeals.
Torii and `sorafs_node` may retain durable delivery state and rebuildable
projections, but they do not own an independent reserve balance or lifecycle
state.

The canonical implementation consists of:

- `ReserveAuthorityPolicyV1` and `ReserveAuthorityPolicyRecordV1`;
- `ReserveProviderAccountV1`, including its compare-and-set revision and
  `rent_charged_through_unix` settlement anchor;
- native reserve ISIs for policy activation, provider registration, movement
  request/decision, rent charge, lifecycle advancement, credit draw/repayment,
  and appeal submission/decision;
- finalized, typed policy/provider/movement/appeal/event queries;
- the durable reserve transaction forwarder; and
- the supervised Torii reserve worker, which derives work from one immutable
  finalized ledger view, signs through an injected external software-signer boundary, and
  submits only through strict Torii transaction ingress.

Pre-release reserve state encoded without the V1 settlement anchor is not
compatible. Development deployments must discard and reseed that state; there
is no legacy decoder or state migration.

## Deterministic economics

All amounts use exact `XorQuantity` arithmetic. The governed economics policy
defines storage-class rent, commitment-duration factors, underwriting ratios,
credit caps, APR, and the reserve top-up threshold.

The canonical quote is:

- `monthly_rent = class_rate × capacity_gib × duration_factor`;
- `reserve_requirement = underwriting_ratio × monthly_rent`;
- `reserve_offset = min(reserve_balance ÷ underwriting_ratio, monthly_rent)`;
- `effective_rent = monthly_rent - reserve_offset`; and
- `top_up_threshold = reserve_requirement × top_up_threshold_bps`.

Consensus execution and worker generation use only integer/fixed-point
operations. Validator or Torii wall clocks never participate.

## Rent anchor and billing cadence

V1 has one consensus-visible billing period:

`RESERVE_RENT_BILLING_PERIOD_SECONDS_V1 = 30 × 86,400`.

Registration initializes `rent_charged_through_unix` to the registering block's
timestamp. Whole periods due at finalized time `T` are:

`floor((T - rent_charged_through_unix) / billing_period_seconds)`.

A timestamp before the anchor is invalid. One `ChargeSorafsReserveRent`
instruction may settle from 1 through
`RESERVE_RENT_MAX_BILLING_PERIODS_V1` (12) whole periods and may never advance
past its executing block timestamp. Backlog beyond 12 periods remains due for a
later compare-and-set transaction.

The native charge:

1. verifies the exact operations authority, active policy digest, provider
   revision, monotonic provider timestamp, and number of due periods;
2. derives rent from the governed policy and authoritative reserve partition;
3. performs the provider-to-treasury transfer atomically when rent is non-zero;
4. advances the settlement anchor only in the successful provider after-state;
5. resets overdue days and derives the current `Active` or `Warning` stage;
6. increments the provider revision; and
7. emits the committed `RentCharged` event.

A zero-rent period still executes `ChargeRent` and advances the anchor; it does
not generate a transfer and cannot be converted into lifecycle aging.

## Lifecycle derivation

Lifecycle days are derived from the first unsettled boundary, not from
`updated_at_unix`:

`max(0, floor((T - (rent_charged_through_unix + billing_period_seconds)) / 86,400))`.

The exact due boundary is day zero. The native representation saturates only at
`u16::MAX`.

`AdvanceSorafsReserveLifecycle` rejects:

- a supplied day count different from the anchor-derived value;
- a block timestamp before the provider's latest mutation;
- a transition that changes neither day count nor lifecycle stage; and
- lifecycle aging when at least one rent period is due and one whole
  `effective_rent` is affordable from the provider's exact governed-asset
  balance.

The last rule also covers zero effective rent. A funded provider must settle
through `ChargeRent`, preventing a faulty or compromised worker from defaulting
an account that can pay.

Day-zero lifecycle convergence remains valid. A reserve top-up, withdrawal,
credit change, or policy rotation can change `Active`/`Warning` even before the
first rent boundary; the worker emits an exact day-zero lifecycle instruction
only when the authoritative stage differs.

## Supervised worker

The Torii worker reads all generation inputs from one immutable finalized view:

- the finalized height, block hash, and signed block timestamp;
- the active policy and digest;
- a bounded, provider-ID-ordered account page;
- each provider's authoritative reserve partition; and
- the exact balance of the governed asset held by that provider.

For every provider it:

1. computes whole due periods from the settlement anchor;
2. caps the candidate charge at 12 periods;
3. selects the largest whole-period batch affordable by the exact balance;
4. emits `ChargeRent` when at least one period is affordable;
5. otherwise derives lifecycle days and stage; and
6. emits `AdvanceLifecycle` only when day count or stage differs.

Generation is deterministic across replicas. The operation identity includes
the active policy and provider revision, so same-tip replay is idempotent and a
policy rotation produces new governed semantics. Concurrent submissions are
resolved by the native provider revision; only one stale-revision competitor
can commit.

`[sorafs.storage.reserve_worker].enabled` controls generation. The worker starts
when either SoraFS storage or reserve generation is enabled, so storage-enabled
nodes continue durable outbox drain and finalized reconciliation with
generation disabled. When both controls are disabled, the worker is paused
before task creation and makes zero external signing, submission, or
reconciliation progress. Opening the local `NodeHandle` may still durably
normalize an interrupted signer-only `Signing` claim back to `Ready` without
refunding its attempt. Scan cadence, page size, queue bounds, attempt bounds,
and checkpoint bounds come from `iroha_config`.

The former process-local reserve lifecycle scheduler, lifecycle/movement
routes, local reserve checkpoint, and CLI adapters are removed from the V1
surface. Test fixtures submit the same native instructions used in production;
there is no second reserve record format that can feed admission, settlement,
reputation, or compliance decisions.

## Torii and client boundary

Reserve mutations accept one exact caller-signed `SignedTransaction` encoded as
versioned Norito. The transaction must contain exactly one route-matching
native instruction, use the active chain ID and governed authority, carry the
exact five-minute V1 TTL, and omit nonce, metadata, attachments, and
compatibility payloads. Torii binds provider revision and policy digest against
one immutable ledger view before strict durable ingress.

The V1 HTTP surface is:

- `POST /v1/sorafs/reserve/top-up` and `/withdraw`;
- `POST /v1/sorafs/reserve/movements/{movement_id_hex}/decision`;
- `POST /v1/sorafs/reserve/credit/draw` and `/credit/repay`;
- `POST /v1/sorafs/reserve/appeals` and
  `/appeals/{appeal_id_hex}/decision`;
- authenticated finalized reads under `/policy`, `/providers`, `/movements`,
  `/appeals`, and `/events`; and
- committed event streaming under `/events/stream` and `/events/ws`.

There is no lifecycle update/advance route, custody callback route, balance
alias, credit-line alias, local JSON mutation body, or fallback endpoint. The
Rust client submits the same signed transaction bytes and exposes finalized
exclusive-cursor filters; operators construct governance-only policy,
registration, rent, and lifecycle instructions through the normal transaction
tooling or the supervised worker.

The event-stream authority is not a connection-time capability. Torii checks
the subscriber account against the current reserve policy before every query
page and before emitting every buffered or live event. A policy rotation that
removes the account therefore terminates SSE with a final authorization error
and closes WebSocket delivery without releasing any later event.

## Durable signing, submission, and reconciliation

Generated operations enter the bounded reserve transaction forwarder before
signing. A signing attempt is consumed atomically; restart recovery changes a
stranded signer claim back to ready without refunding the attempt.

The signer boundary receives only the exact fee-quoted `TransactionPayload`.
It must return a transaction with the governed authority, active chain ID, and
single expected native reserve instruction. Production injects an independently
administered external software signer; file keys and environment overrides are
test/development-only.

Canonical signed bytes are persisted before submission. Their retained digest
is BLAKE3 over those exact bytes. Submission uses strict durable Torii routing;
relayers are never accepted as substitute signers.

Reconciliation reads one finalized view and verifies:

- the current semantic provider/policy state;
- the transaction index;
- the exact Kura block at the indexed height and hash; and
- exactly one matching external transaction entrypoint and its committed
  applied/rejected result.

Queued, approved, committed, or applied local pipeline evidence suppresses an
unsafe absence retry. Exact application, semantic application, rejection,
authoritative absence, retry exhaustion, and conflict use durable transitions
or bounded dead letters. The pending cursor is circular, so a small scan batch
cannot strand older deferred entries.

## Native query surface

Production consumers use finalized typed queries:

- `FindSorafsReservePolicy`;
- `FindSorafsReserveProviderById` and `FindSorafsReserveProviders`;
- `FindSorafsReserveMovementById` and `FindSorafsReserveMovements`;
- `FindSorafsReserveAppealById` and `FindSorafsReserveAppeals`; and
- `FindSorafsReserveEvents`.

Paged queries are exclusive-cursor, bounded, canonically encoded, and tied to a
`ReserveFinalizedCursorV1`. A supplied cursor must match the immutable view.
Reputation, orderbook, compliance, and transparency consumers must use these
committed projections rather than local reserve summaries.

## Finalized telemetry projection

Reserve dashboards are no longer fed by a `sorafs_node` runtime snapshot. The
supervised Torii worker rebuilds movement and appeal totals from the contiguous
typed finalized event journal, then scans current provider accounts in the same
immutable finalized view. It publishes only after journal-derived pending
movement and appeal totals exactly match the committed per-provider counters.
Until that check succeeds, or after any query/cursor/arithmetic/capacity error,
`torii_sorafs_reserve_finalized_projection_ready` is zero and no partial
economic gauges replace the last complete projection.

Provider credit metrics are aggregated by the five fixed lifecycle stages;
movement metrics use the three fixed native statuses. Provider IDs, movement
IDs, appeal IDs, account IDs, and payload material are never metric labels.
Each complete publication exposes its finalized height, while failed refreshes
increment a payload-free counter. Event catch-up is bounded to 1,024 records per
scan and the current provider scan fails closed above the explicit 4,096-record
V1 telemetry capacity instead of publishing a truncated view.

## Required validation evidence

`scripts/build_sorafs_reserve_rent_canary.py` builds the payload-free evidence
envelopes consumed by the reserve rollout checker. It is evidence tooling only:
it cannot create, replace, or attest native ledger state, and production
promotion must use observations collected from the reviewed deployment.

Reserve/rent promotion evidence must cover:

- due-period and timestamp rollback bounds;
- the 12-period cap and multi-batch catch-up;
- largest-affordable partial batches and exact-balance settlement;
- zero-rent anchor advancement;
- insufficient-funds and failed-transfer rollback;
- lifecycle exact-boundary, day-zero convergence, and no-op suppression;
- funded-provider lifecycle rejection;
- active-policy rotation and stale-revision concurrency;
- finalized-event catch-up, cursor-gap/underflow rejection, atomic page replay,
  provider-counter reconciliation, bounded labels, and projection capacity;
- a fresh metrics scrape digest with projection readiness equal to one, a
  positive represented finalized height, and zero refresh failures over the
  preceding five minutes;
- restart during signing/submission and retry exhaustion;
- exact applied/rejected/absent transaction observation;
- corrupt signed bytes, digest mismatch, missing/duplicate Kura entrypoints, and
  stale finalized cursors; and
- circular scan behavior with a batch size of one.

The reserve lane is release-ready only when these tests, the full workspace and
SDK gates, the four-validator deployment exercise, security review, disaster
recovery rehearsal, and signed aggregate readiness evidence all pass. This
document describes the V1 contract; it does not substitute for those promotion
artifacts.
