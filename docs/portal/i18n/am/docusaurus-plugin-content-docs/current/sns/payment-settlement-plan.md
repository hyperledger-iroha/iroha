---
id: payment-settlement-plan
lang: am
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
---

> Canonical source: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

Roadmap task **SN-5 — Payment & Settlement Service** introduces a deterministic
payment layer for the Sora Name Service. Every registration, renewal, or refund
must emit a structured Norito payload so treasury, stewards, and governance can
replay the financial flows without spreadsheets. This page distills the spec
for portal audiences.

## Revenue model

- Base fee (`gross_fee`) derives from the registrar pricing matrix.  
- Treasury receives `gross_fee × 0.70`, stewards receive the remainder minus
  referral bonuses (capped at 10 %).  
- Optional holdbacks allow governance to pause steward payouts during disputes.  
- Settlement bundles expose a `ledger_projection` block with the concrete
  `Transfer` ISIs so automation can post XOR movements straight into Torii.

## Services & automation

| Component | Purpose | Evidence |
|-----------|---------|----------|
| `sns_settlementd` | Applies policy, signs bundles, surfaces `/v1/sns/settlements`. | JSON bundle + hash. |
| Settlement queue & writer | Idempotent queue + ledger submitter driven by `iroha_cli app sns settlement ledger`. | Bundle hash ↔ tx hash manifest. |
| Reconciliation job | Daily diff + monthly statement under `docs/source/sns/reports/`. | Markdown + JSON digest. |
| Refund desk | Governance-approved refunds via `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

CI helpers mirror these flows:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Observability & reporting

- Dashboards: `dashboards/grafana/sns_payment_settlement.json` for treasury vs
  steward totals, referral payouts, queue depth, and refund latency.
- Alerts: `dashboards/alerts/sns_payment_settlement_rules.yml` monitors pending
  age, reconciliation failures, and ledger drift.
- Statements: daily digests (`settlement_YYYYMMDD.{json,md}`) roll into monthly
  reports (`settlement_YYYYMM.md`) which are uploaded both to Git and the
  governance object store (`s3://sora-governance/sns/settlements/<period>/`).
- Governance packets bundle dashboards, CLI logs, and approvals before council
  sign-off.

## Rollout checklist

1. Prototype quote + ledger helpers and capture a staging bundle.
2. Launch `sns_settlementd` with queue + writer, wire dashboards, and exercise
   alert tests (`promtool test rules ...`).
3. Deliver refund helper plus monthly statement template; mirror artefacts into
   `docs/portal/docs/sns/reports/`.
4. Run a partner rehearsal (full month of settlements) and capture the
   governance vote marking SN-5 as complete.

Refer back to the source document for the exact schema definitions, open
questions, and future amendments.
