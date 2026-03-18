---
lang: fr
direction: ltr
source: docs/source/sorafs_hedging_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f9e6d695d417b77cb38b625072a788232820a5926933b06e0ab26977e0197c
source_last_modified: "2026-01-03T18:07:57.026282+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS XOR Hedging & Billing
summary: Final specification for SFM-5 covering price feeds, hedging engine, billing statements, risk controls, observability, and rollout.
---

# SoraFS XOR Hedging & Billing

## Goals & Scope
- Provide a resilient hedging service that maintains a USD-pegged reference price for XOR credits used across SoraFS.
- Generate dual-quoted billing statements (XOR + USD) for providers and buyers, reflecting streaming settlement activity.
- Offer APIs, CLI tooling, and alerts so operators can monitor escrow runway, hedge positions, and reconcile invoices.
- Ensure auditability with signed Norito payloads and transparent decision logs.

This specification completes **SFM-5 — XOR hedging & billing layer**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Hedging engine (`hedgingd`) | Aggregates price feeds, executes hedge trades, derives reference XOR/USD rate, maintains risk exposure. | Runs in active/standby pair; deterministic decision logs. |
| Price feed collectors | Pull primary/secondary feeds (on-chain TWAP, Chainlink, synthetic pairs), normalise to `PriceFeedV1`. | Each feed isolated; feeds signed and validated. |
| Billing aggregator (`billingd`) | Consumes settlement events, computes usage line items, applies hedged rates, emits weekly statements. | Connects to settlement service and orderbook. |
| Statement publisher (`billing_publisher`) | Stores statements, emails notifications, exposes REST API, updates Governance DAG (optional). | Handles acknowledgement tracking. |
| Alerting service | Monitors escrow balances, hedge exposure, feed divergence, overdue statements. | Integrates with Alertmanager + Slack/email. |

## Price Feeds & Hedging Decision
- **Feeds**:
  - `primary`: on-chain XOR/USDT TWAP (10-minute window) published by governance oracle committee. Retrieved via Torii `GET /v1/oracle/price?symbol=XORUSDT`.
  - `secondary`: Chainlink XOR/USDC feed or synthetic XOR/KSM + KSM/USDT pair (via HTTPS). Transformed into `PriceFeedV1`.
  - `tertiary`: internal market data (orderbook implied price) used as sanity check.
- **Decision rules**:
  1. Validate signature, timestamp (stale if >3 min) and confidence per feed.
  2. If primary & secondary within ±1.5%, compute weighted average `price = (p1*w1 + p2*w2)/(w1+w2)` with `w = confidence`.
  3. If divergence > threshold, use primary only, set `status = degraded`, emit alert `hedging_oracle_divergence`.
  4. If primary stale >5 min, fall back to 24h moving average, pause auto-trades, escalate `critical`.
- **Trade execution**:
  - Hedge ratios configured in `hedging_config.toml` (target inventory = expected 7-day settlement).
  - Trades executed via integrated DEX/market connectors (SORA DEX, aggregated OTC).
  - Every trade recorded as `HedgingTradeV1` (Norito) with fields: `trade_id`, `side`, `amount_xor`, `price`, `venue`, `tx_hash`, `reason`.
- **Decision log**:
  - `hedging_decision.log` appended with `HedgingDecisionV1` for each price tick.
  - Logs stored in S3 + shipped to transparency pipeline; retained 12 months hot / 5 years cold.

## Billing Pipeline
- **Event sources**: streaming settlement receipts (`SettlementReceiptV1`), orderbook trades, orchestrator fees, governance penalties.
- **Aggregation**:
  - Billing runs hourly to update accruals in `billing_accruals` table.
  - Weekly (Monday 00:00 UTC) finalize `BillingStatementV1` per account (providers + buyers).
  - Ad-hoc statements on demand (`POST /v1/billing/statements/run`).
- **Statement schema** (unchanged from draft, reaffirmed):
  ```norito
  struct BillingStatementV1 {
      statement_id: Uuid,
      account_id: AccountId,
      period_start: Timestamp,
      period_end: Timestamp,
      hedging_price_source: String,
      xor_usd_rate_avg: Decimal64,
      line_items: Vec<BillingLineItemV1>,
      adjustments: Vec<HedgingAdjustmentV1>,
      total_xor: Decimal64,
      total_usd: Decimal64,
      signatures: Vec<GovSignature>,
  }
  ```
- **Line items** include category, quantity, unit prices (XOR & USD), metadata (manifest IDs, trade IDs).
- **Adjustments** cover hedging gain/loss, penalties, credits.
- **Distribution**:
  - REST `GET /v1/billing/statements/{id}` (Norito + PDF). GraphQL option for dashboards.
  - Email notifications with secure link (expires 7 days).
  - CLI `sorafs billing download --period 2026-W09`.
  - Acknowledgement required within 5 days; API `POST /v1/billing/statements/{id}/ack`.

## Escrow & Alerts
- **Escrow monitoring**:
  - Evaluate provider/buyer XOR escrow vs projected weekly usage (`expected_usage = 7d average + safety factor`).
  - Alert thresholds: `warning` at 50% coverage, `critical` at 20%.
  - Auto top-up option: configure wallet to top up when below threshold.
- **Balance API**: `GET /v1/billing/escrow/{account}` -> XOR balance, projected burn rate, USD equivalent.
- **Notifications**: Slack, email, and Torii events `EscrowLowEventV1`.
- **Runway dashboards**: Grafana shows `escrow_days_remaining`, `hedging_position`, `statement_status`.

## APIs & CLI
- REST endpoints (secured with mTLS + JWT scopes):
  - `GET /v1/hedging/price` – latest derived price, status, feed metadata.
  - `GET /v1/hedging/status` – inventory, hedge ratio, outstanding trades.
  - `GET /v1/billing/statements?account=&period=` – list statements.
  - `POST /v1/billing/statements/{id}/ack` – acknowledge.
  - `GET /v1/billing/accruals?account=` – interim usage totals.
  - `GET /v1/billing/config` – current fees, hedging parameters.
- CLI additions:
  - `sorafs hedging price` – prints current XOR/USD reference, feed status.
  - `sorafs hedging history --since 24h` – view decision log summary.
  - `sorafs billing statement --account <id> --period 2026-W08`.
  - `sorafs billing escrow --account <id>`.
  - `sorafs billing acknowledge --statement <id>`.
- SDK helpers expose typed models for statements, adjustments, and hedging status.

## Observability
- Metrics:
  - `sorafs_hedging_price_usd`
  - `sorafs_hedging_feed_lag_seconds{feed}`
  - `sorafs_hedging_decision_result_total{status}`
  - `sorafs_hedging_inventory_xor`
  - `sorafs_billing_statements_total{status}`
  - `sorafs_billing_statement_latency_seconds`
  - `sorafs_billing_ack_overdue_total`
  - `sorafs_billing_escrow_days_remaining{account_type}`
  - `sorafs_billing_line_item_total{category}`
- Logs: structured `hedging_decision`, `billing_statement`, `escrow_alert` entries.
- Alerts:
  - Feed divergence > 1.5%.
  - Primary feed stale > 5 min.
  - Hedging inventory outside target ±15%.
  - Statement generation failure.
  - Overdue acknowledgements.

## Security & Governance
- Hedging trades authorized via governance-approved key; manual overrides require multi-sig.
- Price feed connectors run with least privilege; TLS pinning for external feeds.
- Statement signatures require governance sign-off; statements hashed and optionally recorded in Governance DAG.
- Compliance: store financial data encrypted at rest (Postgres with Transparent Data Encryption; S3 SSE-KMS).
- Audit logs immutable (append-only) with daily hash committed to Governance DAG.
- Access control: API scopes (`hedging.read`, `billing.read`, `billing.manage`). CLI tokens derive from `sorafs auth`.

## Testing Strategy
- Unit tests for price aggregation, fallback logic, billing calculations.
- Integration tests with simulated feeds (stale, divergent, failure scenarios).
- End-to-end tests generating synthetic settlement events -> statement generation -> acknowledgement.
- Fuzz tests for statement serialization/deserialization.
- Load testing for billing aggregator (target 10k settlement receipts/min).
- Chaos drills: disable primary feed, degrade secondary, ensure alerts and fallback.
- Financial reconciliation tests comparing aggregated totals vs underlying settlement receipts.

## Rollout Plan
1. Implement price feed collectors, hedging engine prototype; deploy to staging with mock trades (no real funds).
2. Integrate billing aggregator with settlement/orderbook test streams; validate statement format with Economics + Treasury.
3. Connect to real price feeds (read-only) and run in shadow mode (decision logs only) for 2 weeks.
4. Governance approval for initial hedging config & fee schedule; publish `HedgingConfigV1`.
5. Enable automated hedging with small notional in staging, then production (start with low inventory limit).
6. Launch billing API/CLI; distribute first staging statements; perform reconciliation.
7. Production rollout:
   - Week 0: shadow statements sent (not enforced).
   - Week 1: official statements + acknowledgement requirement; escalate as needed.
   - Week 2: enable alerts + auto top-up.
8. Publish operator documentation (`docs/source/sorafs/hedging_operator.md`, `billing_operator.md`) and update dashboards.
9. Record completion in status/roadmap once two successful billing cycles with reconciled accounts completed.

## Implementation Checklist
- [x] Define feeds, aggregation, and hedging decision logic.
- [x] Document trading, inventory targets, and risk alerts.
- [x] Specify billing statement schema, line items, adjustments, and acknowledgement workflow.
- [x] Provide APIs, CLI, SDK surfaces.
- [x] Capture observability metrics, logs, and alerts.
- [x] Outline security, compliance, and audit requirements.
- [x] Detail testing plans and rollout steps.

With this specification, treasury, economics, and engineering teams can implement the hedging and billing stack with clear operational boundaries, compliance guarantees, and tooling support.
