---
lang: mn
direction: ltr
source: docs/source/sorafs/deal_engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e38b6c30d653b3c56a70a0575c7e527a29cb54111b528bd24f1c6a7f06f4fe86
source_last_modified: "2025-12-29T18:16:36.109834+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Deal Engine

The SF-8 roadmap track introduces the SoraFS deal engine, providing
deterministic accounting for storage and retrieval agreements between
clients and providers. Agreements are described with the Norito payloads
defined in `crates/sorafs_manifest/src/deal.rs`, covering deal terms, bond
locking, probabilistic micropayments, and settlement records.

The embedded SoraFS worker (`sorafs_node::NodeHandle`) now instantiates a
`DealEngine` instance for every node process. The engine:

- validates and registers deals using `DealTermsV1`;
- accrues XOR-denominated charges when replication usage is reported;
- evaluates probabilistic micropayment windows using deterministic
  Blake3-based sampling; and
- produces ledger snapshots and settlement payloads suitable for governance
  publishing.

Unit tests cover validation, micropayment selection, and settlement flows so
operators can exercise the APIs with confidence. Settlements now emit
`DealSettlementV1` governance payloads, wiring directly into the SF-12
publishing pipeline, and update the `sorafs.node.deal_*` OpenTelemetry series
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) for Torii dashboards and SLO
enforcement. Follow-up items focus on auditor-initiated slashing automation and
coordinating cancellation semantics with governance policy.

Usage telemetry now also feeds the `sorafs.node.micropayment_*` metrics set:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, and the ticket counters
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). These totals expose the probabilistic
lottery flow so operators can correlate micropayment wins and credit carry-over
with settlement outcomes.

## Torii Integration

Torii exposes dedicated endpoints so providers can report usage and drive the
deal lifecycle without bespoke wiring:

- `POST /v2/sorafs/deal/usage` accepts `DealUsageReport` telemetry and returns
  deterministic accounting outcomes (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` finalises the current window, streaming the
  resulting `DealSettlementRecord` alongside a base64-encoded `DealSettlementV1`
  ready for governance DAG publication.
- Torii's `/v2/events/sse` feed now broadcasts `SorafsGatewayEvent::DealUsage`
  records summarising each usage submission (epoch, metered GiB-hours, ticket
  counters, deterministic charges), `SorafsGatewayEvent::DealSettlement`
  records that include the canonical settlement ledger snapshot plus the
  BLAKE3 digest/size/base64 of the on-disk governance artefact, and
  `SorafsGatewayEvent::ProofHealth` alerts whenever PDP/PoTR thresholds are
  exceeded (provider, window, strike/cooldown state, penalty amount). Consumers can
  filter by provider to react to new telemetry, settlements, or proof-health alerts without polling.

Both endpoints participate in the SoraFS quota framework via the new
`torii.sorafs.quota.deal_telemetry` window, allowing operators to tune the
allowed submission rate per deployment.
