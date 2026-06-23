---
lang: ja
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
summary: SFM-5 target architecture and current gap status for XOR price feeds, hedging, billing statements, risk controls, and rollout.
---

# SoraFS XOR Hedging & Billing

## Status
SFM-5 is not implemented as a SoraFS hedging and billing stack in this
workspace. There is no shipped `hedgingd`, price-feed collector service,
`billingd`, statement publisher, SoraFS hedging/billing REST API, SoraFS
hedging/billing CLI, statement fixture suite, or hedging-specific dashboard and
alert pack.

Implemented adjacent foundations include SoraFS reserve quote/ledger tooling,
DA rent/bonus telemetry, reserve ledger digest dashboards, generic subscription
billing code, and generic oracle/feed tests. Those pieces do not yet constitute
the SoraFS hedging or statement pipeline described here. The sections below are
the target design that still needs implementation.

## Goals & Scope
- Maintain a resilient XOR/USD reference price for SoraFS billing and economic reporting.
- Generate dual-quoted billing statements in XOR and USD for providers and buyers.
- Provide APIs, CLI tooling, and alerts for escrow runway, exposure, invoice reconciliation, and statement acknowledgement.
- Keep decisions auditable through signed Norito payloads, deterministic logs, and governance evidence.

## Target Architecture
| Component | Responsibility | Current workspace status |
|-----------|----------------|--------------------------|
| Hedging engine | Aggregate price feeds, derive the reference XOR/USD rate, track exposure, and optionally execute hedges. | Not shipped. |
| Price feed collectors | Fetch primary/secondary/tertiary feeds and normalize them into signed price payloads. | Not shipped for SoraFS hedging. |
| Billing aggregator | Consume settlement, rent, egress, fee, and penalty events and produce account accruals. | Not shipped for SoraFS statements. |
| Statement publisher | Store, sign, publish, notify, and track acknowledgements for statements. | Not shipped. |
| Alerting service | Monitor feed divergence, escrow runway, exposure limits, and statement failures. | Not shipped for hedging/billing. |

## Target Price Feeds And Decisions
- Primary feed: governance-approved on-chain XOR/USD or XOR/stablecoin TWAP.
- Secondary feed: independent market feed or synthetic pair.
- Tertiary feed: internal market/orderbook-implied sanity check once SFM-2 exists.
- Reject stale or unsigned feeds; mark the decision degraded on feed divergence.
- Use deterministic fixed-point math for all aggregation and fallback logic.
- Record every decision as a versioned Norito payload with feed inputs, weights, status, and signature context.

Automated hedge execution must remain off until governance approves venues,
keys, limits, failover rules, and reconciliation evidence.

## Target Billing Pipeline
- Event sources: orderbook settlement receipts, reserve/rent ledgers, egress accounting, orchestrator fees, provider incentives, and governance penalties.
- Hourly accruals update account-level usage and projected escrow runway.
- Weekly statements finalize line items, adjustments, XOR totals, USD equivalents, signatures, and acknowledgement deadlines.
- Statement payloads must be Norito-first; PDF/email delivery is a presentation layer over the signed payload.
- Statement hashes should be publishable into governance evidence once the governance DAG pipeline is available.

## Target APIs And CLI
No hedging or billing routes are currently shipped. The intended API surface is:

- Latest XOR/USD reference price and feed status.
- Hedging status, inventory, and exposure.
- Billing statement list, fetch, acknowledgement, and accrual queries.
- Escrow balance/runway queries.
- Billing and hedging configuration inspection.

Target CLI helpers should mirror those routes for price/status inspection,
statement download, escrow inspection, and acknowledgement. They should reuse
the existing SoraFS CLI conventions and signed client configuration.

## Target Observability
No hedging/billing-specific metrics are shipped in this checkout. Required
metrics include:

- XOR/USD reference price and feed lag.
- Feed divergence and decision result counters.
- Hedge inventory/exposure.
- Statement generation count, latency, failures, and overdue acknowledgements.
- Escrow runway by account type.
- Billing line items by category.

Required alerts include feed divergence, primary feed staleness, exposure drift,
statement generation failure, acknowledgement backlog, and escrow runway below
warning/critical thresholds.

## Security & Governance Requirements
- Price-feed and statement payloads must be signed and replayable.
- Hedge trades require governance-approved keys, venues, inventory limits, and manual override policy.
- Financial data storage needs encryption, retention, and audit-log policy before production use.
- API scopes should distinguish read-only hedging, billing read, billing management, and treasury operations.
- Production behavior must be configured through `iroha_config`, not environment variables.

## Testing Strategy
Required before rollout:
- Unit tests for feed validation, fixed-point aggregation, fallback decisions, billing line items, and statement totals.
- Integration tests with stale/divergent feed simulations and synthetic settlement/rent inputs.
- End-to-end tests from usage events through statement finalization and acknowledgement.
- Serialization and fixture tests for all statement, feed, decision, and adjustment payloads.
- Reconciliation tests comparing generated statements with underlying ledger and settlement sources.

## Rollout Status
- Done: target requirements are documented; adjacent reserve, DA rent telemetry, and generic billing/oracle foundations exist.
- Remaining: implement hedging/feed payloads, collector service, deterministic pricing engine, billing aggregator, statement publisher, signed APIs, CLI helpers, fixtures, dashboards, alerts, reconciliation tests, governance approval flow, and at least two successful staged billing cycles.
