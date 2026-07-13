---
id: deal-engine
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: Overview of the SF-8 deal engine, Torii integration, and telemetry surfaces.
---

:::note Canonical Source
:::

# SoraFS Deal Engine

The SF-8 roadmap track introduces the SoraFS deal engine, providing
deterministic accounting for storage and retrieval agreements between
clients and providers. Agreements are described with the Norito payloads
defined in `crates/sorafs_manifest/src/deal.rs`, covering deal terms, bond
locking, probabilistic micropayments, and settlement records.

The embedded SoraFS worker (`sorafs_node::NodeHandle`) instantiates a
`DealEngine` for every node process. The engine:

- validates identifiers, inclusive epoch windows (including one-epoch deals),
  prices, capacity, metadata bounds, and exact bond requirements before
  mutation;
- accrues XOR-denominated charges when replication usage is reported;
- evaluates probabilistic micropayment windows using deterministic
  BLAKE3-based sampling;
- persists exact nano-XOR accounting, ticket replay protection, funding
  sequences, and the canonical settlement head in atomic checkpoints; and
- produces ledger snapshots and settlement payloads suitable for governance
  publishing.

Every external funding request carries the exact next one-based durable
sequence; replays, gaps, and concurrent forks fail before balance mutation and
remain rejected after restart. Active deals can be cancelled only at an exact
pre-terminal settlement boundary when the current window has no usage, credit
carry, or liability. Cancellation releases remaining collateral and emits an
irreversible, predecessor-linked `Cancelled` settlement.

Settlements emit
`DealSettlementV1` governance payloads, wiring directly into the SF-12
publishing pipeline, and update the `sorafs.node.deal_*` OpenTelemetry series
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) for Torii dashboards and SLO
enforcement.

Usage telemetry now also feeds the `sorafs.node.micropayment_*` metrics set:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, and the ticket counters
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). These totals expose the probabilistic
lottery flow so operators can correlate micropayment wins and credit carry-over
with settlement outcomes.

## Torii Integration

Torii exposes the complete authenticated deal lifecycle:

- `POST /v1/sorafs/deal/fund-provider` accepts the exact next provider funding
  sequence and requires a fresh, body-bound signature from the current Ed25519
  key in that provider's admitted advert.
- `POST /v1/sorafs/deal/fund-client` accepts the exact next client funding
  sequence and requires a configured operator signature.
- `POST /v1/sorafs/deal/open` validates and atomically activates a funded deal
  for a provider with a current admitted advert; it requires a configured
  operator signature.
- `POST /v1/sorafs/deal/usage` accepts `DealUsageReport` telemetry and returns
  deterministic accounting outcomes (`UsageOutcome`); the complete request is
  signed by the deal provider's current admitted Ed25519 key.
- `POST /v1/sorafs/deal/cancel` performs conservative boundary-only
  cancellation and returns the final canonical governance payload; it requires
  a configured operator signature.
- `POST /v1/sorafs/deal/settle` finalises the current window, streaming the
  resulting `DealSettlementRecord` alongside a base64-encoded `DealSettlementV1`
  ready for governance DAG publication; it requires a configured operator
  signature.
- Torii's `/v1/events/sse` feed now broadcasts `SorafsGatewayEvent::DealUsage`
  records summarising each usage submission (epoch, metered GiB-hours, ticket
  counters, deterministic charges), `SorafsGatewayEvent::DealSettlement`
  records that include the canonical settlement ledger snapshot plus the
  BLAKE3 digest/size/base64 of the on-disk governance artefact, and
  `SorafsGatewayEvent::ProofHealth` alerts whenever PDP/PoTR thresholds are
  exceeded (provider, window, strike/cooldown state, penalty amount). Consumers can
  filter by provider to react to new telemetry, settlements, or proof-health alerts without polling.

All deal endpoints participate in the SoraFS quota framework via the
`torii.sorafs.quota.deal_telemetry` window, allowing operators to tune the
allowed submission rate per deployment.
