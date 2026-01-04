<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a85de93fe3864e11479242bbe76b6e1b96f854b548dc0ef78c4a6a4504d31bba
source_last_modified: "2025-11-15T07:12:49.005624+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: deal-engine
title: SoraFS deal engine
sidebar_label: Deal engine
description: SF-8 deal engine، Torii integration اور telemetry surfaces کا جائزہ۔
---

:::note مستند ماخذ
:::

# SoraFS deal engine

SF-8 roadmap track SoraFS deal engine متعارف کراتا ہے، جو
clients اور providers کے درمیان storage اور retrieval agreements کے لیے
deterministic accounting فراہم کرتا ہے۔ Agreements کو Norito payloads کے ساتھ
بیان کیا جاتا ہے جو `crates/sorafs_manifest/src/deal.rs` میں تعریف شدہ ہیں، اور
deal terms، bond locking، probabilistic micropayments اور settlement records کو کور کرتے ہیں۔

Embedded SoraFS worker (`sorafs_node::NodeHandle`) اب ہر node process کے لیے
ایک `DealEngine` instance بناتا ہے۔ یہ engine:

- `DealTermsV1` کے ذریعے deals کو validate اور register کرتا ہے؛
- replication usage report ہونے پر XOR-denominated charges accumulate کرتا ہے؛
- deterministic BLAKE3-based sampling کے ذریعے probabilistic micropayment windows evaluate کرتا ہے؛ اور
- ledger snapshots اور governance publishing کے لیے موزوں settlement payloads بناتا ہے۔

Unit tests validation، micropayment selection اور settlement flows کو cover کرتے ہیں تاکہ
operators اعتماد کے ساتھ APIs exercise کر سکیں۔ Settlements اب `DealSettlementV1` governance
payloads emit کرتے ہیں، جو SF-12 publishing pipeline میں براہ راست wire ہوتے ہیں، اور
OpenTelemetry کی `sorafs.node.deal_*` series
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) کو Torii dashboards اور
SLO enforcement کے لیے update کرتی ہے۔ Follow-up items auditor-initiated slashing automation اور
cancellation semantics کو governance policy کے ساتھ coordinate کرنے پر مرکوز ہیں۔

Usage telemetry اب `sorafs.node.micropayment_*` metrics set کو بھی feed کرتی ہے:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, اور ticket counters
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). یہ totals probabilistic lottery flow کو ظاہر کرتے ہیں تاکہ
operators micropayment wins اور credit carry-over کو settlement outcomes کے ساتھ correlate کر سکیں۔

## Torii integration

Torii dedicated endpoints expose کرتا ہے تاکہ providers usage report کر سکیں اور
بدون bespoke wiring کے deal lifecycle drive کر سکیں:

- `POST /v1/sorafs/deal/usage` `DealUsageReport` telemetry accept کرتا ہے اور
  deterministic accounting outcomes (`UsageOutcome`) return کرتا ہے۔
- `POST /v1/sorafs/deal/settle` current window finalize کرتا ہے، اور
  نتیجے میں بننے والا `DealSettlementRecord` base64-encoded `DealSettlementV1` کے ساتھ stream کرتا ہے
  جو governance DAG publication کے لیے تیار ہوتا ہے۔
- Torii کا `/v1/events/sse` feed اب `SorafsGatewayEvent::DealUsage` records broadcast کرتا ہے
  جو ہر usage submission کا خلاصہ دیتے ہیں (epoch, metered GiB-hours, ticket counters,
  deterministic charges)، `SorafsGatewayEvent::DealSettlement` records جو canonical settlement ledger snapshot کے ساتھ
  on-disk governance artifact کا BLAKE3 digest/size/base64 شامل کرتے ہیں، اور
  `SorafsGatewayEvent::ProofHealth` alerts جب PDP/PoTR thresholds exceed ہوں (provider, window, strike/cooldown state, penalty amount)۔
  Consumers provider کے لحاظ سے filter کر کے نئی telemetry، settlements یا proof-health alerts پر
  polling کے بغیر react کر سکتے ہیں۔

دونوں endpoints SoraFS quota framework میں نئے `torii.sorafs.quota.deal_telemetry` window کے ذریعے
شامل ہیں، جس سے operators ہر deployment کے لیے allowed submission rate tune کر سکتے ہیں۔
