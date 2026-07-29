---
title: SoraFS XOR Orderbook & Streaming Settlement
summary: SFM-2 target architecture and current gap status for XOR orderbook, streaming settlement, APIs, telemetry, and rollout.
---

# SoraFS XOR Orderbook & Streaming Settlement

## Status
SFM-2 now has an initial Norito payload and reference-validator foundation.
`crates/sorafs_manifest` ships the `orderbook` module with versioned order,
cancel, trade, settlement-channel, and settlement-receipt payloads plus
structural/accounting validators. The module also exposes pure deterministic
helpers for matching one maker/taker pair,
matching full order-book snapshots with price-time priority, calculating fees
and escrow requirements, opening settlement channels for trades, and applying
settlement receipts to channel snapshots. It also exposes domain-separated
canonical Ed25519 digest and verification helpers for order, cancel, and
settlement-receipt payloads.
`sorafs_manifest::reference` exposes
`validate_orderbook_payload_bytes` and `OrderbookValidationPayloadKindV1`, and
the `sorafs-validate orderbook` CLI validates those payloads by kind or alias.
The composite fixture-bundle validator also accepts the committed orderbook
payloads so release smoke checks catch drift in order, trade, channel, and
settlement-receipt fixtures.

The authoritative native ledger now owns policy, admission sequence, nonce
high-water, remaining quantity, lifecycle, book revision, deterministic trades,
settlement channels, receipt ranges, and committed events. Bounded
`MatchSorafsOrderbook` execution applies price-time priority against an exact
expected revision, partitions funded buyer custody, and creates channels
atomically. Bounded maintenance expires orders/channels and refunds custody.
Receipt admission verifies the provider signature and settles the native lock
atomically. The active policy commits exact matcher and settlement accounts;
open channels prevent settlement-authority rotation.

Torii POST routes accept one already-signed native transaction and validate the
exact route instruction, policy digest, canonical payload, embedded signer, and
transaction authority before queueing it. Book, trade, channel, receipt, and
event reads are finalized-chain typed projections with bounded pagination,
explicit finalized cursors, stale-cursor rejection, and ETags. SSE and
WebSocket event streams poll only the finalized typed event journal.

The pre-cutover `sorafs_node` mirror, checkpoint, policy/config surface,
mutation/event API, settlement publication outbox, and test-only Torii
authority are deleted. The supervised worker derives bounded match,
maintenance, and settlement operations from one finalized view, persists them
before runtime/HSM signing, forwards them through strict durable ingress, and
reconciles exact or semantic application, rejection, absence, retry, and
conflict against committed state. Its bounded observability projection consumes
the typed finalized event journal, validates complete cursor pages, scans
authoritative active orders/channels under the same immutable finalized anchor,
and publishes ready/height/freshness/failure signals only after all counters
reconcile. Remaining production work is source validation, published SDK release
artifacts, four-peer recovery tests, and genuine rollout evidence. The Rust reference API, C ABI,
CLI, committed fixtures, SDK validator/builders, target observability fixtures,
Prometheus metric handles, and bundle-validation selectors are present.
`scripts/check_sorafs_orderbook_rollout_evidence.py` now provides the
fail-closed SFM-2 rollout evidence gate for deployed orderbook and
streaming-settlement promotion packets, and
`scripts/run_sorafs_orderbook_rollout_evidence.py` provides the matching
reviewed evidence collection planner/runner.
`scripts/build_sorafs_orderbook_canary.py` is a payload-free SFM-2 orderbook canary builder
for contract surface, matcher service, settlement service, API gateway, event
streams, SDK release, observability, reconciliation, and governance approval
evidence. It takes reviewed deployment facts, validates each generated artifact
against the rollout gate, rejects duplicate `--artifact` ids for SDK release
canaries, rejects SDK release canaries with fewer than one distinct artifact
per reviewed SDK language or artifact IDs outside the reviewed SDK language
prefixes, rejects duplicate or unknown closed-set
`--verified-claim`, `--route`, `--stream`, `--language`, `--metric`, and
`--source` inputs before writing any canary JSON, rejects malformed or
non-production `--accepted-order`,
`--matched-order`, `--open-channel`, `--settled-receipt`, and `--peer`
inventory labels before writing any canary JSON, and writes atomically without
following output symlinks. The checker
exports its required
  top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the runner
  dry-run emits the checker-backed `evidence_contract` map for selected SFM-2
  evidence kinds. The shared runner plan guard rejects non-canonical nested
  required-kind, threshold, external-evidence, evidence-contract, and
  command-step shapes before dry-run output or verifier execution. Matcher,
  settlement, API gateway, event stream, SDK release, observability,
  reconciliation, and governance approval artifacts must carry a
  `contract_digest_hex` that matches a valid contract-surface artifact in the
  same bundle. Contract-digest mismatches are
recorded on the offending artifact in the JSON summary before required-kind
validity is reported. Contract-surface evidence must also carry
`policy_digest_hex`, the checker publishes valid contract-surface policies as
`valid_policy_digests`, and governance approval `policy_digest_hex` must match
one of those valid contract-surface policy digests. Promotion summaries must
expose exactly one active contract digest and exactly one active policy digest;
mixed valid contract or policy anchors fail closed before binding checks can
satisfy final promotion.

Other foundations that this work can build on include generic settlement and
deal payloads, SoraFS pricing/reserve helpers, repair/PoR governance evidence,
Torii SoraFS route patterns, and the reference validation surface in
`sorafs_manifest`. The service and rollout sections below are target design
until they are implemented and verified.

## Goals & Scope
- Deliver a governance-controlled limit orderbook that prices SoraFS storage and egress in XOR while supporting USD visibility through oracle feeds.
- Provide streaming settlement so buyers pay per delivered byte/range while orders remain on-chain and auditable.
- Integrate economics, incentives, telemetry, reputation, and governance evidence with the rest of SoraFS.
- Expose typed APIs and SDK hooks so providers, buyers, and orchestrators can interact deterministically.

## Target Architecture
| Component | Responsibility | Current workspace status |
|-----------|----------------|--------------------------|
| On-chain orderbook contract | Store bids/asks, match orders, record fills, and enforce escrow requirements. | Shipped locally: governance-chained policy with exact matcher/settlement identities, canonical signed admission/cancellation, price-time matching against an expected revision, atomic buyer-custody partitioning, channel creation, expiry/refund, replay-safe receipt settlement, typed finalized queries, and committed events. Four-peer promotion evidence remains external. |
| Off-chain matching engine | Reconcile finalized state and submit bounded matching/maintenance transactions. | The supervised HSM-capable durable worker derives one native operation from a finalized view per scan, persists it before signing, drains regardless of generation policy, and reconciles exact/semantic finality with bounded retries and conflict dead letters. Deployment and four-peer evidence remain external. |
| Escrow and streaming-settlement service | Manage escrow channels and debit buyers per delivered chunk range. | Native matching creates funded channel locks, native maintenance refunds expiry, and native receipt admission atomically credits provider/treasury. The same durable worker forwards retained provider-signed receipts and reconciles settlement finality; live governance publication evidence remains open. |
| Pricing oracle feeder | Supply XOR/USD price, tier multipliers, and fee schedules. | Generic oracle/pricing foundations exist, but no SoraFS orderbook feeder is wired. |
| API gateway | Expose order placement, cancellation, depth, trades, settlement receipt submission, and event streams. | Torii POST routes validate and forward already-signed native transactions. GET/SSE/WebSocket routes return bounded finalized typed projections with cursor consistency and stale-anchor rejection. Client release smoke and live multi-peer stream evidence remain open. |
| Analytics and dashboards | Publish pricing, fee, utilization, depth, and settlement reports. | The supervised worker now emits bounded finalized-ledger flow, depth, matcher, channel-age/runway, revision, projection-ready/height/freshness/failure metrics. API outcomes are counters with closed route/outcome labels; dashboards and alert rules consume those real emitters. Live scrape, alert-routing, and rollout evidence remain external. |

### Target Data Flow
1. Provider or buyer submits a signed order through the API.
2. Torii validates the signed envelope and submits the canonical native transaction without a book mutation.
3. The contract executes deterministic price-time matching and emits fill/cancel events.
4. Consumers query or stream the typed finalized event journal; any cache is rebuildable and non-authoritative.
5. Filled orders open settlement channels. Delivered ranges generate signed receipts that debit buyer escrow and credit providers.
6. Trades and settlement receipts are written to governance evidence for audit and reconciliation.

## Implemented Data Model Foundation
- `OrderRequestV1`: order id, side, tier, price, quantity, remaining quantity, owner account bytes, expiry, nonce, maker/taker fee basis points, and signature material.
- `OrderCancelV1`: order id, owner account bytes, reason, nonce, and signature material.
- `TradeEventV1`: trade id, maker/taker order ids, tier, price, filled quantity, maker/taker fees, and timestamp.
- `SettlementChannelV1`: channel id, trade id, buyer account bytes, provider id, total/remaining bytes, locked XOR, status, and timestamps.
- `SettlementReceiptV1`: receipt id, channel id, trade id, byte range, chunk hash, bytes delivered, XOR debited, provider credit, fee amount, issued timestamp, and signature material.
- `ByteRangeV1`, `OrderSideV1`, `OrderTierV1`, `OrderCancelReasonV1`, `SettlementChannelStatusV1`, and `OrderbookSignatureV1`.

The payloads use Norito, deterministic `XorAmount` micro-XOR values, and
explicit schema-version constants. `ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1`
caps canonical order/cancel owner accounts at 256 bytes before order-id hashing,
signature hashing, SDK/native field building, or durable nonce-high-water use.
The validators reject zero identifiers, empty or oversized owner accounts, zero
prices/quantities/escrow/debits, invalid remaining
quantities/bytes, invalid byte ranges, self-trades, fee basis points above 100%,
bad Ed25519 key/signature lengths, and settlement receipts where provider credit
plus fee does not equal the buyer debit. The separate signature helpers derive
BLAKE3 digests over domain-separated canonical Norito payloads with only the
mutable signature bytes cleared, then verify Ed25519 signatures against the
embedded public key.

The manifest payload/helper layer is intentionally not an authority. The
native ISI layer supplies durable order/channel/escrow transitions; Torii
authenticates and forwards exact signed native transactions; the supervised
forwarder owns only retry-safe delivery state. No residual book mirror,
compatibility state, or alternate production API exists.

## Implemented Authoritative Ledger Foundation

The native ledger surface now exposes six first-release ISIs:

- `SetSorafsOrderbookPolicy` activates an exactly chained policy revision under
  `CanSetSorafsPricing`. Revision one has no predecessor; later revisions must
  advance by one, bind the active digest, and preserve the non-zero market id.
- `SubmitSorafsOrderbookOrder` admits exact canonical bounded
  `OrderRequestV1` bytes only after verifying the embedded Ed25519 signature,
  canonical I105 owner, transaction-authority signer, current policy digest,
  pause/quantity/tick/fee/expiry constraints, provider registration for asks,
  duplicate order id, and the owner's shared order/cancel nonce high-water.
- `CancelSorafsOrderbookOrder` verifies the same owner, signer, active-policy,
  and shared-nonce invariants before atomically changing an open order to its
  terminal cancelled record with the exact canonical cancellation bytes.
- `MatchSorafsOrderbook` requires the exact matcher account committed in the
  active policy and an exact expected book revision. It executes a bounded
  price-time transition, partitions funded buyer custody atomically, commits
  partial/final order state, immutable trades, and settlement channels, and
  binds every new channel to the policy's exact settlement authority.
- `MaintainSorafsOrderbook` requires that same governed matcher account and an
  exact expected revision before atomically expiring a bounded number of orders
  and channels and refunding their native custody.
- `RecordSorafsOrderbookSettlementReceipt` requires the exact settlement
  account committed in the active policy and bound into the channel. It verifies
  exact canonical provider-signed receipt bytes, active policy, receipt
  age/future skew/byte bounds, global receipt-id replay protection,
  channel/trade consistency, and a bounded sorted non-overlapping byte-range
  index. It then requires the deterministic channel-derived native asset lock
  to use the configured SoraFS XOR fee asset, name the same governed settlement
  account as release authority, target a registered provider, remain active and
  sufficiently funded, and pass all transfer-policy and destination-overflow
  checks before atomically crediting the provider and configured treasury and
  persisting the immutable receipt/index. A predecessor-linked policy may
  rotate the matcher at any revision; settlement-authority rotation fails
  closed while any channel bound to the preceding authority remains open.

These records use durable `smart_contract_state` keys and fail closed if stored
bytes are oversized, malformed, non-canonical, internally inconsistent, or no
longer verify cryptographically. Typed signed-query variants expose the active
policy, order-by-id, cancellation-by-order-id, receipt-by-id, constant-time
ledger status counters, exclusive-cursor/status-filtered order pages, and
exclusive-cursor/channel-filtered receipt pages. Page sizes are restricted to
`1..=500`, and records remain ordered by canonical id. Signed-query visitors
remain executor-permissioned; the production Torii read routes execute these
typed queries against one finalized state anchor and reject stale cursors.
Order, cancellation, and receipt POST routes accept one already-signed native
transaction, validate the exact route instruction and authority binding, and
forward that transaction without mutating a local book.

The settlement-receipt ledger deliberately does not invent or fund channels.
The matcher/channel-opening path must first create the deterministic
channel-derived generic native asset lock, funded by the buyer, targeting the
provider, and naming the settlement authority. Once that binding exists,
receipt admission and provider/treasury asset movement are one fail-closed
state transition; missing, expired, mis-authorized, wrong-asset, underfunded,
or overflowed locks leave balances, custody, receipt state, and replay indexes
unchanged.

## Implemented Deterministic Mechanics
- `match_orders_v1` validates one maker/taker pair, rejects expired orders,
  same-side orders, tier mismatches, self-crossing accounts/orders, and
  non-crossing prices, then emits a `TradeEventV1` at the maker price.
- `OrderFillOutcomeV1` reports the trade, maker/taker remaining GiB, and gross
  fill value.
- `match_order_book_v1` accepts `OrderBookEntryV1` snapshots with canonical
  admission sequences, skips expired orders, rejects duplicate order ids or
  sequences, returns expired-order tombstones in sequence order, matches
  independently by tier, and applies deterministic price-time priority with
  partial fills.
- `derive_orderbook_trade_id_v1` derives domain-separated BLAKE3 trade ids for
  full-book fills from fill index, timestamp, order ids, and pre-fill
  remaining quantities.
- `trade_gross_value_v1` and `trade_escrow_requirement_v1` calculate gross
  fill value and the conservative escrow floor covering gross value plus maker
  and taker fees.
- `open_settlement_channel_for_trade_v1` maps filled GiB to bytes with
  `BYTES_PER_GIB`, opens a `SettlementChannelV1`, and locks the deterministic
  escrow requirement.
- `apply_settlement_receipt_v1` checks channel/trade binding, receipt time,
  byte coverage, remaining bytes, and escrow sufficiency before returning the
  next channel snapshot.
- `order_request_signature_digest_v1`,
  `order_cancel_signature_digest_v1`, and
  `settlement_receipt_signature_digest_v1` derive canonical signable payload
  digests for Ed25519 verification.
- `verify_order_request_signature_v1`, `verify_order_cancel_signature_v1`, and
  `verify_settlement_receipt_signature_v1` validate payload structure and
  verify embedded Ed25519 signatures.
These helpers are deterministic local mechanics only. They verify the embedded
payload signer but do not by themselves bind that signer to a
request/capability, submit transactions, or persist runtime escrow/order state.
Only native ISIs persist order/channel/escrow state; `sorafs_node` retains
bounded delivery checkpoints for exact native transaction forwarding, not a
book.

## Implemented Reference Validation Surface
- `validate_orderbook_payload_bytes` accepts `OrderRequestV1`, `OrderCancelV1`,
  `TradeEventV1`, `SettlementChannelV1`, and `SettlementReceiptV1` bytes through
  `OrderbookValidationPayloadKindV1`.
- The validator emits stable `ValidationOutcomeV1` records and maps orderbook
  structural, settlement-accounting, policy, signature, and Norito decode
  failures into the reference SDK error catalogue.
- The `sorafs-validate orderbook` CLI supports `--kind <payload-kind> --input
  <path>` plus payload aliases: `--order`, `--cancel`, `--trade`, `--channel`,
  and `--receipt`. The retired runtime-snapshot alias is rejected.
- The `sorafs-validate sign --kind orderbook --payload-kind
  order-request|order-cancel|settlement-receipt` CLI path signs those
  orderbook payloads with runtime-only Ed25519 seeds, validates the signed
  Norito bytes, and writes output only after validation succeeds.
- `sorafs_manifest::sign_orderbook_payload_bytes_ed25519_v1(...)` exposes the
  same encoded-payload signing path for downstream bindings. JavaScript
  `signOrderbookPayload(...)`, Python `sign_orderbook_payload(...)`,
  Kotlin/JVM `SorafsReferenceValidators.signOrderbookPayload(...)`, Java
  Android `SorafsReferenceValidators.signOrderbookPayload(...)`, and Swift
  `SorafsReferenceValidators.signOrderbookPayload(...)` wrap it for
  already-encoded order, cancel, and settlement-receipt bytes.
- `sorafs_manifest::build_signed_orderbook_*_bytes_ed25519_v1(...)` builds,
  signs, validates, and encodes canonical field-level order request, order
  cancel, and settlement receipt payloads. JavaScript wraps these as
  `buildSignedOrderbookOrderRequest(...)`,
  `buildSignedOrderbookOrderCancel(...)`, and
  `buildSignedOrderbookSettlementReceipt(...)`; Python wraps them as
  `build_signed_orderbook_order_request(...)`,
  `build_signed_orderbook_order_cancel(...)`, and
  `build_signed_orderbook_settlement_receipt(...)`; Kotlin/JVM, Java Android,
  and Swift wrap them through the shared `connect_norito_bridge` C/JNI/native
  facade.
- The `reference_ffi` C ABI exposes `sorafs_reference_validate_orderbook_json`
  with stable `SORAFS_REFERENCE_ORDERBOOK_KIND_*` selectors for SDK bindings.
- Fixture-bundle validation accepts orderbook order, cancel, trade, settlement
  channel, and settlement-receipt payloads alongside the existing
  manifest-linked SoraFS artifacts.

## Target Matching And Settlement Rules
- Native matching preserves the `match_order_book_v1` price-time priority and
  partial-fill semantics under an exact expected book revision.
- Deterministic expiry handling and stale order cancellation.
- The authoritative ledger enforces one governance-chained, environment-free
  policy digest covering exact matcher/settlement accounts, quantity, tick, fee,
  expiry, receipt freshness/size, and per-channel retention bounds. The retired
  `sorafs.storage.orderbook` local policy is rejected by configuration parsing.
- Maker/taker fee accounting with deterministic treasury accrual.
- Escrow checks before order acceptance; settlement debits must preserve double-entry accounting.
- Provider failures must close or breach settlement channels in a way that feeds reputation, repair/slashing, and governance evidence.

## API Surface

Implemented authoritative Torii routes:

- `POST /v1/sorafs/orderbook/orders`, `/cancel`, and `/receipts` accept a
  header-bearing Norito or JSON `SignedTransaction`. Each decoded transaction
  is canonically re-encoded and must fit the 2 MiB native transaction ceiling;
  the embedded order, cancellation, or receipt retains its independent 64 KiB
  canonical ceiling. Each route requires exactly one
  native orderbook instruction of the route-specific kind, verifies the
  transaction signature and active finalized policy digest, validates the
  canonical embedded payload and Ed25519 signature, binds order/cancel owner to
  transaction authority, and binds receipt authority to the finalized channel.
  Accepted requests enter the normal transaction pipeline; Torii does not
  mutate an independent book.
- `GET /v1/sorafs/orderbook/book`, `/trades`, `/channels`, and `/receipts`
  execute typed queries against one finalized state view. They use bounded
  `limit` and exclusive record cursors, accept an optional complete expected
  finalized height/hash pair, and return conflict for stale anchors.
- `GET /v1/sorafs/orderbook/events` returns a hash-chain-validated committed
  event page with an ETag. Its continuation cursor binds sequence, block
  height/hash, and event index.
- `GET /v1/sorafs/orderbook/events/stream` and `/events/ws` replay the requested
  finalized page and poll the typed committed-event query for later finalized
  records. No process-local broadcast history exists.

Still-open API/service work:

- Complete clean-consumer SDK smoke, authenticated deployment tests, runtime
  PKCS#11/HSM signer injection, and live multi-peer stream/restart evidence.

## Observability
The deleted mirror no longer emits orderbook metrics. The supervised native
forwarder builds telemetry only from one immutable finalized state view. It
consumes at most 1,024 typed committed events per scan, validates strict
sequence/block/index cursors, bounds active-order and open-channel scans by the
native V1 capacities, rejects malformed/incomplete pages and arithmetic
overflow, and cross-checks event totals, lifecycle counters, trades, receipts,
admission/trade sequences, and channel totals before publishing. Matcher lag
is measured from the last committed event that actually advanced the book
revision, so later receipt or policy events cannot hide a stale matcher.
Catch-up or any error clears
`torii_sorafs_orderbook_finalized_projection_ready`; a successful refresh
writes the ready bit last and exposes the finalized height and block timestamp.
No local book, provider-id label, or mirror-divergence gauge exists.

- `dashboards/grafana/sorafs_orderbook_observability.json` tracks committed
  event flow, authoritative tier/side depth, bounded matcher lag, settlement
  backlog/age/runway, API counter-derived error ratio, and finalized projection
  readiness, height, and freshness.
- `dashboards/alerts/sorafs_orderbook_rules.yml` defines alert thresholds for
  projection unavailability/staleness/failures/replica-height skew, matching
  lag, settlement backlog, API error ratio, and escrow runway.
  Projection-derived matcher and
  settlement alerts are gated on the ready bit so retained diagnostic values
  cannot page as if they were a fresh projection.
- `dashboards/alerts/tests/sorafs_orderbook_rules.test.yml` provides synthetic
  Prometheus rule coverage for the alert pack.
- `iroha_telemetry::metrics::Metrics` registers the
  `torii_sorafs_orderbook_*` Prometheus families consumed by those fixtures and
  exposes one fail-closed finalized projection publication helper plus bounded
  route/outcome API counters. Route labels are mapped to a closed vocabulary;
  unknown values collapse to `other`.

Live Prometheus scrape freshness, alert installation/routing, and reviewed
production evidence remain deployment obligations. No compatibility mirror is
available as a fallback source.

## Security & Compliance Requirements
- On-chain state and settlement receipts must be auditable and replayable.
- The matcher must not become the source of truth; every worker action and
  replica comparison must reconcile the same finalized ledger cursor.
- Governance must be able to pause order placement and fee/surge updates.
- Wash trading and self-crossing rules need explicit policy before beta.
- Access control should reuse SoraFS authentication/capability patterns already used by Torii-facing SoraFS APIs.

## Testing & Fixtures
Implemented:
- `crates/iroha_data_model` tests cover governed policy bounds/digests,
  channel-derived settlement-lock ids, all four ISI registry/slice round trips,
  and every typed orderbook query wire variant.
- `crates/iroha_core/src/smartcontracts/isi/sorafs_orderbook.rs` tests cover the
  native policy/order/cancellation/receipt lifecycle, status counters, typed
  lookup and cursor pages, and fail-closed asset settlement. Adversarial cases
  include malformed/non-canonical/oversized payloads, signature and owner
  mismatch, replay/nonce rollback, corrupt durable state, missing/wrong-asset/
  mis-authorized/expired/unregistered locks, missing transfer-call context,
  custody drift/overdraw, destination overflow, and unchanged balances/audit
  state on rejection.
- `crates/iroha_executor` tests require either pricing or settlement permission
  for all seven authoritative read-query visitors.
- `crates/sorafs_manifest/src/orderbook.rs` unit tests cover valid orders,
  cancellation payloads, Ed25519 signature-length checks, invalid remaining
  quantities/bytes, self-trade rejection, and balanced/imbalanced settlement
  receipts, deterministic pair matching, full-book price-time priority,
  partial fills, expired-order filtering, duplicate snapshot guards, fee
  calculation, settlement channel opening, receipt-driven channel closure,
  canonical signature digest stability, valid order/cancel/receipt signature
  verification, tampered order-signature rejection, and deterministic generated
  order-stream invariant scenarios for filled/open balance conservation and
  non-crossing book remainders, deterministic permutation-invariance scenarios
  that prove canonical sequence drives matching and expired tombstone ordering,
  plus overlapping receipt-range rejection. Retired runtime-snapshot selectors
  and CLI aliases are covered only as negative compatibility rejection.
- `crates/sorafs_manifest/src/reference.rs` unit tests cover accepted orderbook
  payloads, malformed Norito, policy failures, signature failures, and
  settlement-accounting imbalance outcomes; bundle tests cover orderbook
  payloads mixed with linked SoraFS artifacts.
- `crates/sorafs_manifest/src/bin/sorafs-validate.rs` parser tests cover the
  orderbook CLI kind path, receipt alias, duplicate alias rejection, and
  supported kind aliases.
- `crates/sorafs_manifest/src/reference_ffi.rs` tests cover accepted orderbook
  FFI validation, bundle validation with canonical order/trade/channel/receipt
  payloads, and unsupported retired-selector rejection.
- `fixtures/sorafs_manifest/orderbook/` contains deterministic `.to` and JSON
  commentary fixtures for orders, cancellations, trades, settlement channels,
  and settlement receipts; regenerate them with
  `cargo run -p sorafs_manifest --bin generate_orderbook_fixtures`.
- `crates/sorafs_manifest/tests/orderbook_fixtures.rs` round-trips the
  committed orderbook fixtures and checks JSON `norito_bytes_hex` commentary
  against the canonical bytes.
- `crates/sorafs_node` tests cover the bounded durable transaction forwarder,
  canonical signing material, retry/attempt ceilings, restart recovery,
  reconciliation retention, checkpoint corruption, and dead-letter bounds.
- `crates/iroha_config` unit and fixture tests cover
  bounded `sorafs.storage.orderbook_worker` operational policy and explicitly
  reject the retired `[storage.orderbook]` authority table.
- `crates/iroha_torii` and native smart-contract tests cover exact
  signed-transaction route binding, owner/authority/provider/policy/revision
  negatives, native price-time/escrow/channel/receipt atomicity, durable worker
  reconciliation, finalized queries/cursors, event hash-chain validation, SSE
  backlog replay, and WebSocket frame shape.

Required before rollout:
- Full focused and workspace validation of authoritative ledger/durable-worker
  parity with reference price-time semantics, channel creation, receipt
  authorization, lock funding/refund/expiry, settled balances, and finality
  reconciliation.
- External fuzz harnesses for random order streams beyond the deterministic
  generated matcher invariant and permutation-invariance scenarios.
- Integration tests spanning contract, matcher, settlement service, API, and governance evidence publication.
- Load and disaster-recovery tests for oracle outage, contract pause, queue lag, and settlement backlog.

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_orderbook_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_orderbook_rollout_evidence_test.py`

## Rollout Evidence Gate

Use the rollout gate after the deployed orderbook contract, durable matcher,
streaming-settlement receipt service, authenticated API gateway, durable
SSE/WebSocket streams, released SDK artifacts, live observability, reconciliation
jobs, and governance packet have produced reviewed, payload-free JSON evidence:

```sh
python3 scripts/check_sorafs_orderbook_rollout_evidence.py \
  @scripts/examples/sorafs_orderbook_rollout_evidence.args.example
```

For staged collections with reviewed evidence paths, prefer the planner so the
verifier command, summary path, thresholds, and current required payload-free
field contract are reproducible:

```sh
python3 scripts/run_sorafs_orderbook_rollout_evidence.py \
  @scripts/examples/sorafs_orderbook_rollout_collection.args.example \
  --dry-run
```

For reviewed canary evidence generation, build individual payload-free
artifacts first and then pass the resulting files to the gate:

```sh
python3 scripts/build_sorafs_orderbook_canary.py \
  @scripts/examples/sorafs_orderbook_contract_canary.args.example
python3 scripts/build_sorafs_orderbook_canary.py \
  @scripts/examples/sorafs_orderbook_api_canary.args.example
python3 scripts/build_sorafs_orderbook_canary.py \
  @scripts/examples/sorafs_orderbook_observability_canary.args.example
python3 scripts/build_sorafs_orderbook_canary.py \
  @scripts/examples/sorafs_orderbook_reconciliation_canary.args.example
```

The checker recognizes `sorafs.orderbook.*` SFM-2 rollout schemas for contract
surface, matcher service, settlement service, API gateway, event streams, SDK
release, observability, reconciliation, and governance approval evidence. It
reports `ready` only when every required kind is present, every recognized
artifact is valid, raw order payloads, receipt payloads, raw snapshots, raw
contract state, response bodies, signed transactions, secrets, and ledgers are
absent, route latency, stream lag, and matcher lag stay under configured
thresholds, every API route response carries a lowercase `body_blake3_hex`
digest, and `routes[].latency_ms`, matcher `matcher_lag_ms`, and stream
`lag_ms` are non-negative integer-unit evidence before those ceilings apply,
reconciliation covers at least four peers, governance is bound to
`iroha_config`, and matcher, settlement, API, stream, SDK, observability,
reconciliation, and governance approval artifacts carry a
`contract_digest_hex` that matches a valid contract-surface artifact in the
same rollout bundle. Governance approval must also carry a `policy_digest_hex`
that matches a valid contract-surface policy digest from the same rollout
bundle. Matcher-service artifacts also bind `accepted_order_count`,
`matched_order_count`, and `rejected_invalid_order_count` to the unique
canonical `accepted_orders`, `matched_orders`, and `rejected_invalid_orders`
inventories, require matched orders to be present in the accepted-order
inventory, require order IDs to use reviewed lowercase `orderbook-order-*`
labels without non-production markers, and reject duplicate order entries
before promotion can report ready. Settlement-service artifacts also bind `open_channel_count`,
`settled_receipt_count`, and `settlement_backlog_count` to the unique canonical
`open_channels`, `settled_receipts`, and `settlement_backlog_channels`
inventories, require channel and receipt IDs to use reviewed lowercase
`orderbook-channel-*` and `orderbook-receipt-*` labels without non-production
markers, and reject duplicate channel or receipt entries before promotion can
report ready. API gateway artifacts also bind
`route_count` to the unique canonical `routes[].name` inventory and reject
duplicate or unknown route entries before promotion can report ready, and require
every route response to carry a lowercase `body_blake3_hex` digest. Event-stream
artifacts also bind `stream_count` to the
unique canonical `streams[].name` inventory and reject duplicate or unknown
stream entries before promotion can report ready. SDK release artifacts also
bind `language_count` to the unique canonical `languages[].name` inventory,
reject missing, inflated, duplicate, or unknown language evidence, and bind
`artifact_count` to the unique canonical `artifacts[].id` inventory, rejecting
duplicate artifact entries before promotion can report ready. They also require
artifact IDs to start with a reviewed SDK language prefix and at least one
distinct SDK release artifact per reviewed SDK language before promotion can
report ready. Matcher evidence must affirm finalized-cursor replay, committed
state reconciliation, and absence of a local book authority. Orderbook
payload-safety artifacts must explicitly set `raw_contract_state_included`,
`raw_receipts_included`, `response_bodies_included`, `debug_artifacts`,
`critical_alerts_firing`, and `raw_ledger_included` to `false` before promotion
can report ready.
Observability
artifacts also bind `metric_count` to the unique canonical `metrics` inventory,
require the reviewed orderbook metrics set, and reject duplicate or unknown
metric labels before promotion can report ready. They require a successful
scrape no more than 120 seconds old, a ready non-zero-height finalized
projection whose finalized timestamp is not newer than the scrape or more than
120 seconds behind it, and zero projection failures during the reviewed
collection window. The summary exports the sorted
reviewed `metrics` inventory plus `metric_count_values`, and the aggregate
production-readiness gate requires those fields to match the observability
artifact fingerprint before final promotion can report ready. Aggregate
promotion also rechecks the lane-proven orderbook digest relationships:
contract-bound artifact fingerprints must match `valid_contract_digests`, and
policy-bound artifact fingerprints must match `valid_policy_digests` before
final promotion can report ready. Reconciliation artifacts also bind
`peer_count` and `source_count` to the unique canonical `peers[].name` and
`sources[].name` inventories, require peer labels to use reviewed lowercase
`orderbook-peer-*` labels without non-production markers, and reject duplicate
peer entries plus duplicate or unknown source entries before promotion can
report ready. The closed source inventory names the finalized ledger, matcher
worker, Torii finalized projection, settlement worker, and Governance DAG; no
process-local mirror is recognized. The collection planner's
dry-run JSON also includes the checker-backed `evidence_contract` map so operators can inspect
the exact required fields for each requested evidence kind before collecting or
submitting live orderbook artifacts. Use the payload-free SFM-2 orderbook
canary builder for reviewed promotion evidence after those deployment facts
exist; route-bearing API gateway canaries require explicit
`--route-body-blake3-hex` evidence. The builder does not replace the missing
runtime HSM/KMS, SDK release smoke, live scrape/alert routing, multi-peer
deployment, or reconciliation evidence.

## Rollout Status
- Done locally: canonical order/cancel/trade/channel/receipt payloads and
  validators; cross-SDK Ed25519 builders; native predecessor-linked policy with
  exact matcher/settlement identities; authoritative admission, cancellation,
  bounded price-time matching, partial/final state, atomic buyer-custody
  partitioning, trade/channel creation, expiry/refund, receipt settlement, typed
  finalized queries/events; signed-transaction Torii forwarding; finalized
  REST/SSE/WebSocket projections; durable matcher/maintenance/settlement
  generation, signing, submission, retry/dead-letter, and finality
  reconciliation; deletion of the mirror-local
  book/config/checkpoint/outbox and runtime-snapshot format; reference
  CLI/FFI/fixtures; rollout checker, planner,
  payload-free canary contracts; bounded fail-closed finalized observability,
  API counters with closed labels, dashboards, and Prometheus rule tests.
- The rollout evidence gate also publishes contract-surface policy digests as
  `valid_policy_digests` and rejects governance approval artifacts whose
  `policy_digest_hex` is not anchored to one of those valid policies.
- Remaining: validate and deploy the supervised worker with a runtime
  PKCS#11/HSM signer; complete SDK release/live smoke, live scrape and alert
  routing, four-peer retry/restart/recovery tests,
  and genuine staged/live evidence that passes the SFM-2 gate.

The runner validates the schema-closed collection-plan envelope before printing
dry-run JSON or executing the verifier. The shared runner plan guard rejects
non-canonical nested required-kind, threshold, external-evidence,
evidence-contract, and command-step shapes.
