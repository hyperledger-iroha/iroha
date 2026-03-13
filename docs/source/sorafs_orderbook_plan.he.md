---
lang: he
direction: rtl
source: docs/source/sorafs_orderbook_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd8565edf5a4820a4ea209508fd1f16f986a91184642d24ad0cc86e713caadc0
source_last_modified: "2026-01-03T18:07:57.202547+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS XOR Orderbook & Streaming Settlement
summary: Final specification for SFM-2 covering matching architecture, escrow and streaming settlement, APIs, security, observability, and rollout.
---

# SoraFS XOR Orderbook & Streaming Settlement

## Goals & Scope
- Deliver a governance-controlled limit orderbook that prices SoraFS storage/egress in XOR while supporting USD visibility through oracle feeds.
- Provide streaming settlement so buyers pay per delivered byte/range while orders remain on-chain and auditable.
- Integrate economics (tier multipliers, maker/taker fees), incentives, and telemetry with existing SoraFS components (pricing, reputation, governance DAG).
- Expose REST/gRPC/WebSocket APIs and SDK hooks so providers, buyers, and orchestrators can interact deterministically.

This document completes **SFM-2 — XOR Orderbook & streaming settlement**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| On-chain orderbook contract (`OrderbookContractV1`) | Stores bids/asks, matches orders, records fills, enforces escrow requirements. | Implemented on SORA chain (Layer-1). Deterministic matching engine, maker/taker fee logic. |
| Off-chain matching engine (`orderbook-matcher`) | Maintains in-memory order book mirror, submits transactions to contract, streams depth/trade events. | Written in Rust, uses optimistic matching with contract reconciliation. |
| Escrow & settlement service (`streaming-settlementd`) | Manages XOR escrow channels, debits buyers per delivered chunk-range (micro-settlements). | Uses Norito-defined streaming receipts; interacts with orderbook contract to confirm settlement completion. |
| Pricing oracle feeder | Supplies XOR/USD price, tier multipliers, fee schedules (governance approved). | Integrates with existing hedging/oracle pipeline (see `sorafs_hedging_plan.md`). |
| API gateway (`sorafs_orderbook_api`) | REST/gRPC/WebSocket endpoints for order placement, cancellation, depth & trades, settlement streaming. | Enforces auth, rate limits, logging. |
| Analytics & dashboards | Generates pricing dashboards, fee reports, utilization stats. | Feeds Grafana and transparency pipeline. |

### Data Flow
1. Provider/buyer submits order via API → request signed, validated, forwarded to matcher.
2. Matcher updates in-memory book, constructs transaction (Norito payload) for on-chain submission.
3. Contract executes matching, persists new state, emits events (fills, cancellations).
4. Ingest service listens to events, updates caches, publishes to WebSocket subscribers.
5. For filled orders, settlement service opens or updates streaming channel. As payload delivered, provider sends chunk receipts → settlement debits buyer escrow, credits provider wallet, logs receipts.
6. All trades/settlements recorded in Governance DAG via `OrderbookFillNode` and `SettlementReceiptNode`.

## On-chain Contract Design
- **State elements:**
  - `OrderV1`: `(order_id, side, manifest_tier, price_xor_per_gib, quantity_gib, remaining_gib, owner, expiry, nonce, maker_fee_bps, taker_fee_bps)`
  - `OrderBookState`: order books per tier (`hot`, `warm`, `archive`) with price-time priority.
  - `EscrowAccount`: locked XOR for buyers; references streaming channels.
  - `FeeConfig`: maker/taker basis points, penalty multipliers, tier multipliers.
- **Norito canonical payloads** (persisted on chain & events):
  ```norito
  struct OrderRequestV1 { order_id: Uuid, side: OrderSide, tier: OrderTier, price_xor_per_gib: Decimal64, quantity_gib: Decimal64, expiry_unix: u64, owner: AccountId, signature: Signature }
  struct OrderCancelV1 { order_id: Uuid, owner: AccountId, reason: CancelReason, signature: Signature }
  struct TradeEventV1 { trade_id: Uuid, maker_order_id: Uuid, taker_order_id: Uuid, tier: OrderTier, price_xor_per_gib: Decimal64, filled_gib: Decimal64, maker_fee_xor: Decimal64, taker_fee_xor: Decimal64, timestamp_unix: u64 }
  struct SettlementReceiptV1 { receipt_id: Uuid, trade_id: Uuid, chunk_bytes: u64, xor_debited: Decimal64, provider_credit: Decimal64, chunk_hash: Digest32, range: ByteRangeV1, issued_at_unix: u64, settlement_signature: Signature }
  ```
- **Matching rules:**
  - Price-time priority with partial fills allowed.
  - Orders expiry enforced on-chain; stale orders automatically canceled.
  - Minimum order size 1 GiB (configurable).
  - Tick size (price granularity) in micro XOR (1e-6).
- **Fee handling:**
  - Maker/taker fees accumulate in contract and distributed daily to treasury.
  - Penalty multipliers applied if provider flagged in reputation engine (e.g., +20% taker fee).
- **Security:**
  - Orders validated using Ed25519/ECDSA signatures present in admission.
  - Contract ensures buyer escrow ≥ order value.
  - Governance multi-sig can pause contract or change fee config (via `FeeConfigUpdateV1` events).

## Streaming Settlement Pipeline
- **Channels:** For each buyer-provider pair per trade, create `SettlementChannelV1` storing `channel_id`, `buyer`, `provider`, `trade_id`, `remaining_bytes`, `xor_locked`, `status`.
- **Workflow:**
  1. After trade event, settlement service (watcher) opens streaming channel using orderbook event.
  2. As orchestrator fetches data, it emits chunk receipts including `chunk_hash`, `range`, `bytes`.
  3. Provider signs `StreamingReceiptV1`; buyer (client) acknowledges; settlement service debits escrow and credits provider.
  4. Once `remaining_bytes` = 0 (or order canceled), channel closes; final settlement recorded in Governance DAG.
- **Frictionless streaming:** For every chunk-range, XOR deducted from buyer’s escrow using price established in trade; supports micro-payments without having to commit entire order quantity upfront.
- **Reconciliation:** At channel close, settlement service compares total debited XOR vs order value; any residual returned to buyer escrow.
- **Failure handling:**
  - If provider fails to deliver within SLA, settlement service can mark channel as breached and issue refund (partial or full). Event recorded for reputation/slashing.
  - Streaming service tolerates network splits by checkpointing the last acknowledged receipt ID (stored in contract for dispute resolution).

## Pricing & Economics
- **Tier multipliers:**
  - hot = 1.00, warm = 0.75, archive = 0.40 (initial; governance adjustable).
- **Maker/Taker fees:** default 5/10 bps with makedown/stimulus for targeted tiers.
- **Surge pricing:** If tier utilization > 85%, contract can auto-increase `tier_multiplier` by governance-defined factor (e.g., +0.05) with 24h notice event.
- **USD equivalent:** Oracle feed `xor_usd` (TWAP). Dashboards compute `price_usd` for transparency.
- **Penalty adjustments:** For providers under penalty (PDP/PoTR failure), contract references penalty table reducing allowed ask volume and increasing taker fees.

## APIs
### REST
- `POST /v2/orderbook/orders` — create order. Body `OrderRequestV1` (JSON). Response includes `order_id`.
- `GET /v2/orderbook/orders/{order_id}` — fetch order status (open, partial, filled, canceled), match history.
- `POST /v2/orderbook/cancel` — cancel order; requires signature and auth token.
- `GET /v2/orderbook/depth?tier=hot&levels=25` — returns `DepthSnapshotV1` (bid/ask ladders).
- `GET /v2/orderbook/trades?since=` — returns trade stream (pagination).
- `GET /v2/orderbook/bbo?tier=` — best bid/offer with timestamp.
- `POST /v2/orderbook/settlements` — submit streaming receipt (for providers).
- `GET /v2/orderbook/settlements/{channel_id}` — settlement channel status and receipts.

### gRPC / WebSocket
- `StreamDepth(DepthRequest)` — server-stream depth snapshots for tier.
- `StreamTrades(TradeStreamRequest)` — real-time trade events.
- `StreamSettlements(SettlementStreamRequest)` — push new receipts or channel updates.
- WebSocket endpoint `/ws/orderbook` replicates gRPC streams for browser clients.

### Authentication & Authorization
- mTLS between gateways/orchestrators and API; Ed25519/EdDSA bearer tokens for external clients (issued by `sorafs auth`).
- Bearer token claims: `sub` (account id), `capabilities` (`order:submit`, `order:cancel`, `settlement:stream`), `tier_limits`.
- Orders carry Ed25519 signatures hashed over canonical Norito; API verifies before forwarding.
- Rate limiting: default 60 order submissions/min per account, 30 cancels/min; burst token bucket 20. Overruns produce HTTP 429 with `Retry-After`.
- Audit logs: every API call stored in `orderbook_audit` table with request ID, identity, and outcome.

## Observability
- Metrics:
  - `sorafs_orderbook_orders_total{side,tier,result}`
  - `sorafs_orderbook_depth_latency_seconds_bucket`
  - `sorafs_settlement_channel_open_total`, `..._close_total`, `..._bytes_streamed_total`
  - `sorafs_orderbook_maker_fees_total`, `..._taker_fees_total`
  - `sorafs_orderbook_surge_factor{tier}`
  - `sorafs_orderbook_api_errors_total{endpoint,code}`
  - `sorafs_orderbook_matching_lag_seconds`
  - `sorafs_orderbook_escrow_balance{status}`
- Logs: structured JSON with fields `rid`, `order_id`, `trade_id`, `tier`, `price`, `quantity`, `status`, `latency_ms`.
- Dashboards: order flow heatmaps, depth snapshots, settlement throughput, fee accumulation, XOR/USD chart.
- Alerts:
  - Matching lag > 5 seconds.
  - Settlement backlog > 10 channels waiting.
  - Contract-event discrepancy (non-deterministic matching).
  - API error rate > 2% over 5 minutes.
  - Escrow depletion or price oracle stale > 10 minutes.

## Security & Compliance
- All orders stored on-chain; tampering infeasible w/out consensus.
- Settlement service enforces double-entry accounting (buyer debit = provider credit + fees). Daily reconciliation with contract state.
- Governance can pause order placement via `PauseOrderbookV1` event.
- KYC compliance handled by `sorafs auth`; tokens issued only to verified accounts.
- Anti-abuse: watchers detect wash trading (same account both sides) and flag for governance review.
- GDPR/Privacy: trade data aggregated for dashboards; individual order details accessible to account owners or governance only.

## Testing & Fixtures
- Unit tests for order validation, fee calculations, settlement channel transitions.
- Simulation tests (Rust `proptest`) for random order streams ensuring invariants (no negative balances).
- Integration tests using localchain (testnet) verifying contract + matcher + settlement interactions.
- Fixture directory `fixtures/sorafs_orderbook/` with sample orders, trades, and settlement receipts (Norito + JSON).
- Load tests: target 5k orders/min sustained, 1k receipts/min; ensure API and contract keep up.
- Disaster recovery tests: simulate oracle outage, contract pause, or queue lag; verify safe fallback.

## Rollout Plan
1. Implement on-chain contract + Rust crates (`crates/sorafs_orderbook_contract`, `crates/sorafs_orderbook_client`).
2. Build matcher, settlement service, and API gateway; integrate with governance DAG for logging.
3. Staging deployment with synthetic workload; run soak tests and settlement reconciliation.
4. Run economics calibration (fee tables, tier multipliers) with Economics WG; publish config in governance pipeline.
5. Enable limited beta (selected providers/buyers) with reduced limits; monitor metrics, adjust.
6. Full production launch after two governance epochs of stable operation; enable dashboards and reporting.
7. Document runbooks (`docs/source/sorafs/orderbook_operator.md`) and update status/roadmap.

## Implementation Checklist
- [x] Document architecture (contract, matcher, settlement, API).
- [x] Define Norito payloads for orders, trades, settlements, and events.
- [x] Specify streaming settlement logic and escrow reconciliation.
- [x] Capture pricing economics, fee calculations, and surge policy.
- [x] Detail REST/gRPC/WebSocket APIs, auth, and rate limiting.
- [x] Outline observability metrics, logs, alerts.
- [x] Provide testing strategy, fixtures, and rollout sequence.
- [x] Note documentation and operator requirements.

With this specification, teams can implement the SoraFS orderbook, settlement service, and integrations with confidence, ensuring pricing remains transparent, auditable, and responsive to network demand.
