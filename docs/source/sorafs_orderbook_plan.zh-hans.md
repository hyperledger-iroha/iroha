---
lang: zh-hans
direction: ltr
source: docs/source/sorafs_orderbook_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 965067d671c0c972601d966f5650ef9d1d55af613bbad4b713da98131c49ffa7
source_last_modified: "2026-06-25T17:28:04+00:00"
translation_last_reviewed: 2026-06-25
title: SoraFS XOR Orderbook & Streaming Settlement
summary: SFM-2 target architecture and current gap status for XOR orderbook, streaming settlement, APIs, telemetry, and rollout.
---

# SoraFS XOR Orderbook & Streaming Settlement

## Status
SFM-2 now has an initial Norito payload, reference-validator foundation, and
local runtime mirror surface. `crates/sorafs_manifest` ships the `orderbook`
module with versioned order, cancel, trade, settlement-channel,
settlement-receipt, and runtime replay snapshot payloads plus
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
settlement receipt, and runtime snapshot fixtures.

`sorafs_node` also owns an in-memory local orderbook mirror for rollout and
operator testing. It accepts canonical `OrderRequestV1`, `OrderCancelV1`, and
`SettlementReceiptV1` payloads, verifies their embedded Ed25519 payload
signatures, enforces config-backed minimum order quantity and price tick policy,
assigns deterministic local admission sequences, runs
`match_order_book_v1`, records emitted `TradeEventV1` values, opens local
`SettlementChannelV1` snapshots for fills, applies non-overlapping settlement
receipts to local channel state, publishes accepted receipts to a configured
local Governance DAG filesystem/signed-runtime-DAG sink, and updates the existing
`torii_sorafs_orderbook_*` metric handles for order flow, open depth, matcher
lag, settlement backlog/age, and contract/mirror divergence. It can also export
and restore a validated canonical Norito local runtime replay snapshot for the
open order set, emitted trades/channels, accepted receipts, and expired-order
tombstones. When storage is enabled, accepted local order/cancel/receipt
mutations atomically checkpoint that snapshot under
`<sorafs.data_dir>/orderbook/runtime-snapshot.to`, and startup reloads a
validated checkpoint if present. Torii exposes that local mirror through Norito
POST routes for order, cancel, and settlement receipt submissions, requiring
canonical `X-Iroha-*` request authentication and binding the embedded payload
signer to the verified request signer. Order and cancel submissions also bind
`owner_account` bytes to the authenticated canonical `AccountId`; known-channel
receipt submissions additionally require the authenticated signer account to
derive to the channel provider id. Torii also exposes JSON GET routes for the
book, trades, settlement channels, accepted settlement receipts, and replayable
event history, plus SSE/WebSocket streams for local orderbook events.

The repository still does not ship an on-chain SoraFS orderbook contract, a
durable off-chain matcher service, an on-chain/daemonized streaming-settlement
receipt service, contract-backed authenticated orderbook streams, published SDK
release artifacts for the orderbook validator/submitter helpers, or live
rollout evidence. The Rust
reference API, C ABI facade, CLI, committed fixture payloads, deterministic
helper surface, local runtime mirror, local Torii routes, local receipt
application path, local receipt Governance DAG publication, local runtime replay
snapshots, local event streams, local payload signature verification, local
request-authenticated POST envelope binding, local known-channel receipt
provider-role checks, local config-backed order admission policy, runtime metric
emission, JavaScript, Python, Kotlin/JVM, Java Android, and Swift orderbook
validator bindings, Rust/JavaScript/Python/Kotlin/JVM/Java Android/Swift
encoded Ed25519 signing helpers for order/cancel/receipt payloads,
Rust/JavaScript/Python/Kotlin/JVM/Java Android/Swift field-level signed
order/cancel/receipt payload builders, JavaScript and Python local orderbook
read helpers,
JavaScript and `iroha_python` local orderbook SSE/WebSocket stream helpers,
JavaScript, `iroha_python`, and standalone `iroha_torii_client` local
order/cancel/receipt submit helpers for already signed Norito payload bytes,
target observability fixtures, Prometheus metric handles/helper methods, and
bundle-validation selectors exist locally.
`scripts/check_sorafs_orderbook_rollout_evidence.py` now provides the
fail-closed SFM-2 rollout evidence gate for deployed orderbook and
streaming-settlement promotion packets, and
`scripts/run_sorafs_orderbook_rollout_evidence.py` provides the matching
reviewed evidence collection planner/runner. Matcher, settlement, API gateway,
event stream, SDK release, observability, and reconciliation artifacts must
carry a `contract_digest_hex` that matches a valid contract-surface artifact in
the same bundle. Contract-digest mismatches are recorded on the offending
artifact in the JSON summary before required-kind validity is reported.

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
| On-chain orderbook contract | Store bids/asks, match orders, record fills, and enforce escrow requirements. | Not shipped. |
| Off-chain matching engine | Maintain an order-book mirror, submit transactions, and stream depth/trade events. | Local mirror is shipped in `sorafs_node` for deterministic matching and Torii/API testing, with canonical Norito replay snapshots and storage-data-dir checkpoint reload; daemonized matcher service and contract submission are not shipped. |
| Escrow and streaming-settlement service | Manage escrow channels and debit buyers per delivered chunk range. | Local settlement-channel snapshots are opened for fills, canonical receipts can update local channel state, and accepted local receipts publish Governance DAG evidence; durable receipt daemon and escrow custody mutation are not shipped. |
| Pricing oracle feeder | Supply XOR/USD price, tier multipliers, and fee schedules. | Generic oracle/pricing foundations exist, but no SoraFS orderbook feeder is wired. |
| API gateway | Expose order placement, cancellation, depth, trades, settlement receipt submission, and event streams. | Local Torii order/cancel/receipt/book/trade/channel/event routes plus SSE/WebSocket local event streams are shipped; embedded payload signatures are verified by the local runtime; local POST routes require canonical request authentication, bind owner/signing keys, require known-channel receipts to be signed by the channel provider account, and enforce provider-advert `torii_gateway` capability for ask placement/known-channel receipts when capability enforcement is enabled; JavaScript and Python Torii clients expose typed local read helpers and field-level signed order/cancel/receipt payload builders, Kotlin/JVM, Java Android, and Swift expose field-level signed orderbook payload builders through `connect_norito_bridge`, JavaScript and `iroha_python` expose local stream helpers, and JavaScript, `iroha_python`, and standalone `iroha_torii_client` expose local submit helpers for already signed Norito order/cancel/receipt bytes; contract forwarding and durable streams are not shipped. |
| Analytics and dashboards | Publish pricing, fee, utilization, depth, and settlement reports. | Local runtime mirror emits the existing order flow/depth/lag/backlog/divergence metric families; live dashboard wiring and rollout evidence are not shipped. |

### Target Data Flow
1. Provider or buyer submits a signed order through the API.
2. The matcher validates the order, updates its mirror, and submits the canonical transaction.
3. The contract executes deterministic price-time matching and emits fill/cancel events.
4. Ingest services update caches and publish depth/trade streams.
5. Filled orders open settlement channels. Delivered ranges generate signed receipts that debit buyer escrow and credit providers.
6. Trades and settlement receipts are written to governance evidence for audit and reconciliation.

## Implemented Data Model Foundation
- `OrderRequestV1`: order id, side, tier, price, quantity, remaining quantity, owner account bytes, expiry, nonce, maker/taker fee basis points, and signature material.
- `OrderCancelV1`: order id, owner account bytes, reason, nonce, and signature material.
- `TradeEventV1`: trade id, maker/taker order ids, tier, price, filled quantity, maker/taker fees, and timestamp.
- `SettlementChannelV1`: channel id, trade id, buyer account bytes, provider id, total/remaining bytes, locked XOR, status, and timestamps.
- `SettlementReceiptV1`: receipt id, channel id, trade id, byte range, chunk hash, bytes delivered, XOR debited, provider credit, fee amount, issued timestamp, and signature material.
- `OrderbookRuntimeSnapshotV1`: local replay checkpoint carrying next admission
  sequence, generated timestamp, open orders, emitted trades, settlement
  channels, accepted receipts, and expired-order tombstones.
- `ByteRangeV1`, `OrderSideV1`, `OrderTierV1`, `OrderCancelReasonV1`, `SettlementChannelStatusV1`, and `OrderbookSignatureV1`.

The payloads use Norito, deterministic `XorAmount` micro-XOR values, and
explicit schema-version constants. The validators reject zero identifiers,
empty accounts, zero prices/quantities/escrow/debits, invalid remaining
quantities/bytes, invalid byte ranges, self-trades, fee basis points above 100%,
bad Ed25519 key/signature lengths, and settlement receipts where provider credit
plus fee does not equal the buyer debit. The separate signature helpers derive
BLAKE3 digests over domain-separated canonical Norito payloads with only the
mutable signature bytes cleared, then verify Ed25519 signatures against the
embedded public key.

Durable order-state mutation, request-envelope authentication, capability
authorization, contract submission, escrow custody mutation, and governance
publication remain outside this payload/helper layer. The local Torii POST
surface adds request-envelope authentication and signer/account binding before
calling the mirror; the mirror itself verifies embedded payload signatures and
performs process-local sequencing only, so its state is not a contract source
of truth.

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
- `OrderbookRuntimeSnapshotV1::validate` checks canonical local replay
  snapshots for version/timestamp validity, unique open-order ids/sequences,
  sequence-window consistency, unique emitted trade/channel/receipt ids, channel
  trade references, receipt channel references, non-overlapping accepted receipt
  ranges, and expired-order tombstone consistency.

These helpers are deterministic local mechanics only. They verify the embedded
payload signer but do not by themselves bind that signer to a
request/capability, submit transactions, or persist runtime escrow/order state.
`sorafs_node` adds local checkpoint persistence for its mirror on top of these
pure helpers when storage is enabled.

## Implemented Reference Validation Surface
- `validate_orderbook_payload_bytes` accepts `OrderRequestV1`, `OrderCancelV1`,
  `TradeEventV1`, `SettlementChannelV1`, `SettlementReceiptV1`, and
  `OrderbookRuntimeSnapshotV1` bytes through `OrderbookValidationPayloadKindV1`.
- The validator emits stable `ValidationOutcomeV1` records and maps orderbook
  structural, settlement-accounting, policy, signature, and Norito decode
  failures into the reference SDK error catalogue.
- The `sorafs-validate orderbook` CLI supports `--kind <payload-kind> --input
  <path>` plus payload aliases: `--order`, `--cancel`, `--trade`, `--channel`,
  `--receipt`, and `--snapshot`.
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
  with stable `SORAFS_REFERENCE_ORDERBOOK_KIND_*` selectors for SDK bindings,
  including `SORAFS_REFERENCE_ORDERBOOK_KIND_RUNTIME_SNAPSHOT`.
- Fixture-bundle validation accepts orderbook order, cancel, trade, settlement
  channel, settlement receipt, and runtime snapshot payloads alongside the
  existing manifest-linked SoraFS artifacts.

## Target Matching And Settlement Rules
- Runtime and on-chain matching must preserve the `match_order_book_v1`
  price-time priority and partial-fill semantics.
- Deterministic expiry handling and stale order cancellation.
- The local mirror enforces minimum order size and price tick from
  `sorafs.storage.orderbook`; durable on-chain/governance policy must preserve
  the same environment-free control surface.
- Maker/taker fee accounting with deterministic treasury accrual.
- Escrow checks before order acceptance; settlement debits must preserve double-entry accounting.
- Provider failures must close or breach settlement channels in a way that feeds reputation, repair/slashing, and governance evidence.

## API Surface
Implemented local Torii routes:

- `POST /v1/sorafs/orderbook/orders` accepts canonical Norito
  `OrderRequestV1` bytes with canonical `X-Iroha-*` request authentication,
  validates the payload, verifies the embedded Ed25519 signature, requires the
  `owner_account` bytes to decode to the authenticated canonical `AccountId`,
  requires the embedded payload signer key to match a verified request signer,
  enforces the configured minimum order quantity and price tick, inserts the
  order into the local mirror, runs deterministic matching, and returns
  accepted order, fills, opened settlement channels, expired order ids, and
  open-order count.
- `POST /v1/sorafs/orderbook/cancel` accepts canonical Norito `OrderCancelV1`
  bytes with canonical `X-Iroha-*` request authentication, verifies the
  embedded Ed25519 signature, requires the `owner_account` bytes to decode to
  the authenticated canonical `AccountId`, requires the embedded payload signer
  key to match a verified request signer, verifies the open order owner match,
  removes the order from the local mirror, and returns cancellation status.
- `POST /v1/sorafs/orderbook/receipts` accepts canonical Norito
  `SettlementReceiptV1` bytes with canonical `X-Iroha-*` request
  authentication, verifies the embedded Ed25519 signature, requires the
  embedded payload signer key to match a verified request signer, requires
  known local channels to be signed by the channel provider account, verifies
  channel existence, rejects duplicate receipt ids and overlapping byte ranges,
  applies the receipt to the local channel snapshot, and returns the accepted
  receipt plus updated channel.
- `GET /v1/sorafs/orderbook/book` returns the local mirror summary, depth by
  tier/side, open orders, emitted trades, settlement channels, settlement
  receipts, and expired ids. The full count/depth fields remain computed over
  the full local mirror while the returned `open_orders`, `trades`,
  `settlement_channels`, and `settlement_receipts` arrays are bounded by
  `limit` (default 50, max 500).
- `GET /v1/sorafs/orderbook/trades` returns local trade events, keeping the
  full `count` visible while bounding the returned `trades` array with `limit`
  (default 50, max 500).
- `GET /v1/sorafs/orderbook/channels` returns local settlement-channel
  snapshots opened by fills, keeping the full `count` visible while bounding
  the returned `channels` array with `limit` (default 50, max 500).
- `GET /v1/sorafs/orderbook/receipts` returns local settlement receipts
  accepted for settlement channels, keeping the full `count` visible while
  bounding the returned `receipts` array with `limit` (default 50, max 500).
- `GET /v1/sorafs/orderbook/events` returns replayable local orderbook events
  with `since` and `limit` cursors.
- `GET /v1/sorafs/orderbook/events/stream` emits server-sent local orderbook
  events, replaying the requested backlog before live events.
- `GET /v1/sorafs/orderbook/events/ws` emits the same local orderbook event
  frames over WebSocket with lag notifications.

Still-target API work:

- Contract-backed capability policy authorization and contract forwarding.
- Durable receipt daemon, governance publication, and escrow custody mutation.
- Durable contract/matcher-backed SSE/WebSocket streams for depth, trades, and
  settlement updates.
- REST/gRPC parity, signed downstream SDK submitter clients, and durable
  contract/matcher stream clients.

The local runtime now verifies embedded signatures over canonical Norito
payloads, and the local Torii POST surface authenticates the canonical request
envelope before binding the embedded signer to the verified request signer and
claimed canonical account where one exists. For known local settlement
channels, receipt POSTs must also be authenticated by the provider account that
derives the channel provider id. Production forwarding must still authorize the
caller's orderbook capability policy and forward the verified payload to the
durable matcher or contract.

## Observability
Local orderbook-specific runtime metric emission is wired for the local mirror.
The repository also ships target observability assets and matching Prometheus
metric handles for the future durable runtime:

- `dashboards/grafana/sorafs_orderbook_observability.json` tracks target order
  flow, depth, matching lag, settlement backlog, API error ratio, escrow runway,
  and contract/mirror divergence metrics.
- `dashboards/alerts/sorafs_orderbook_rules.yml` defines alert thresholds for
  matching lag, settlement backlog, contract/matcher divergence, API error
  ratio, and escrow runway.
- `dashboards/alerts/tests/sorafs_orderbook_rules.test.yml` provides synthetic
  Prometheus rule coverage for the alert pack.
- `iroha_telemetry::metrics::Metrics` registers the
  `torii_sorafs_orderbook_*` Prometheus families consumed by those fixtures and
  exposes helpers for order flow, depth, matcher lag, settlement backlog,
  mirror divergence, API error ratio, and escrow runway.

Runtime services must call those helpers with live matcher, API, contract, and
settlement data before the dashboard and alerts can be considered rollout
evidence. The local mirror currently records order flow, depth, zero matcher
lag for in-process matching, settlement backlog/oldest age, provider escrow
runway from observed receipt debit rates, cumulative Torii orderbook API error
ratios per route, and contract/mirror divergence as false because no contract
mirror exists. Receipt submission updates the local settlement backlog and
escrow runway gauges as channels close or remain open.
Additional metrics still needed at runtime include settlement channel
opens/closes beyond the local mirror, bytes streamed, maker/taker fees, surge
factor, and stale price oracle state.

## Security & Compliance Requirements
- On-chain state and settlement receipts must be auditable and replayable.
- The matcher must not become the source of truth; contract reconciliation must detect divergence.
- Governance must be able to pause order placement and fee/surge updates.
- Wash trading and self-crossing rules need explicit policy before beta.
- Access control should reuse SoraFS authentication/capability patterns already used by Torii-facing SoraFS APIs.

## Testing & Fixtures
Implemented:
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
  plus runtime replay snapshot round-trips and overlapping receipt-range
  rejection.
- `crates/sorafs_manifest/src/reference.rs` unit tests cover accepted orderbook
  payloads, malformed Norito, policy failures, signature failures, and
  settlement-accounting imbalance outcomes; bundle tests cover orderbook
  payloads mixed with linked SoraFS artifacts.
- `crates/sorafs_manifest/src/bin/sorafs-validate.rs` parser tests cover the
  orderbook CLI kind path, receipt alias, duplicate alias rejection, and
  supported kind aliases.
- `crates/sorafs_manifest/src/reference_ffi.rs` tests cover accepted orderbook
  FFI validation, bundle validation with orderbook payloads including runtime
  snapshots, and unsupported selector rejection.
- `fixtures/sorafs_manifest/orderbook/` contains deterministic `.to` and JSON
  commentary fixtures for orders, cancellations, trades, settlement channels,
  settlement receipts, and runtime replay snapshots; regenerate them with
  `cargo run -p sorafs_manifest --bin generate_orderbook_fixtures`.
- `crates/sorafs_manifest/tests/orderbook_fixtures.rs` round-trips the
  committed orderbook fixtures and checks JSON `norito_bytes_hex` commentary
  against the canonical bytes.
- `crates/sorafs_node` unit tests cover local runtime matching, settlement
  channel opening, owner-checked cancellation, receipt application,
  overlapping-range rejection, tampered signed-order rejection, `NodeHandle`
  snapshot state, local Governance DAG receipt publication, and orderbook
  metric updates, plus canonical runtime snapshot export/restore and
  storage-data-dir checkpoint reload, and config-backed minimum order
  quantity/price tick rejection.
- `crates/iroha_config` unit and fixture tests cover
  `sorafs.storage.orderbook` parsing, nonzero clamping, and default snapshot
  visibility for the local orderbook admission policy.
- `crates/iroha_torii` app-API tests cover the local order/cancel/book/trade/
  channel/receipt handler round trip, wrong-owner cancellation rejection, and
  overlapping receipt rejection, missing canonical request-auth rejection,
  embedded payload-signer/request-signer mismatch rejection, known-channel
  receipt provider-role rejection, local provider-advert capability
  enforcement for asks and known-channel receipts, plus local event history,
  SSE backlog replay, and WebSocket frame-shape coverage.

Required before rollout:
- Runtime/service tests proving contract and durable matcher parity with the
  reference full-book price-time semantics, receipt authorization, durable
  order/escrow mutation, and contract reconciliation once those layers exist.
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
verifier command and summary path are reproducible:

```sh
python3 scripts/run_sorafs_orderbook_rollout_evidence.py \
  @scripts/examples/sorafs_orderbook_rollout_collection.args.example \
  --dry-run
```

The checker recognizes `sorafs.orderbook.*` SFM-2 rollout schemas for contract
surface, matcher service, settlement service, API gateway, event streams, SDK
release, observability, reconciliation, and governance approval evidence. It
reports `ready` only when every required kind is present, every recognized
artifact is valid, raw order payloads, receipt payloads, raw snapshots, raw
contract state, response bodies, signed transactions, secrets, and ledgers are
absent, route latency, stream lag, and matcher lag stay under configured
thresholds, reconciliation covers at least four peers, governance is bound to
`iroha_config`, and matcher, settlement, API, stream, SDK, observability, and
reconciliation artifacts carry a `contract_digest_hex` that matches a valid
contract-surface artifact in the same rollout bundle.

## Rollout Status
- Done: target architecture and requirements are documented; initial SoraFS orderbook Norito payloads, structural/accounting validators, canonical embedded Ed25519 payload signature digests/verification, Rust/JavaScript/Python/Kotlin/JVM/Java Android/Swift encoded Ed25519 signing helpers for order/cancel/receipt payloads, Rust/JavaScript/Python/Kotlin/JVM/Java Android/Swift field-level signed order/cancel/receipt payload builders, deterministic pair and full-book snapshot matching/fee/settlement helpers, deterministic generated matcher invariant coverage, canonical Norito local runtime replay snapshots with storage-data-dir checkpoint reload, Rust reference validator, reference FFI selector surface, CLI parser surface, committed fixtures, bundle-validator coverage, JavaScript, Python, Kotlin/JVM, Java Android, and Swift orderbook validator bindings, JavaScript and Python Torii read helpers for local book/trades/channels/receipts/events, JavaScript and `iroha_python` local orderbook SSE/WebSocket stream helpers, JavaScript, `iroha_python`, and standalone `iroha_torii_client` local submit helpers for already signed Norito order/cancel/receipt bytes, target dashboard/alert fixtures, orderbook Prometheus metric handles/helper methods, local runtime mirror, local config-backed order admission policy, local settlement receipt application, local orderbook settlement receipt Governance DAG publication, local Torii order/cancel/receipt/book/trade/channel/event API with `limit`-bounded book/trades/channels/receipts readbacks and full total counts, local request-authenticated orderbook POST envelope/account/signer binding, local known-channel receipt provider-role authorization, local provider-advert capability authorization for asks and known-channel receipts, local SSE/WebSocket event streams with frame-shape coverage, local runtime metric emission including provider escrow runway and API error ratios, fail-closed rollout evidence gate with cross-artifact contract-digest binding, collection planner, operator argfile templates, and focused unit tests are implemented; adjacent settlement, pricing, reserve, validation, and governance foundations exist.
- Remaining: implement on-chain contract surface, durable matcher service, daemonized settlement receipt service with escrow custody mutation, on-chain/governance-backed admission policy, contract-backed capability policy authorization, contract forwarding, durable contract/matcher-backed WebSocket/SSE streams, SDK release artifacts/live smoke evidence, live dashboard wiring and alert routing, contract/mirror reconciliation tests, and staged/live rollout evidence that passes the SFM-2 gate.
