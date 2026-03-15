---
id: payment-settlement-plan
lang: ur
direction: rtl
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SNS ادائیگی اور سیٹلمنٹ پلان

> کینونیکل سورس: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

روڈمیپ ٹاسک **SN-5 -- Payment & Settlement Service** Sora Name Service کے لئے
deterministic payment layer متعارف کراتا ہے۔ ہر registration، renewal یا refund
کو ایک structured Norito payload emit کرنا ہوگا تاکہ treasury، stewards اور
governance اسپریڈشیٹس کے بغیر مالی بہاؤ replay کر سکیں۔ یہ صفحہ پورٹل کے
سامعین کے لئے اسپیک کا خلاصہ کرتا ہے۔

## ریونیو ماڈل

- Base fee (`gross_fee`) registrar کی pricing matrix سے اخذ ہوتا ہے۔
- Treasury `gross_fee x 0.70` وصول کرتی ہے، stewards باقی حصہ referral bonuses
  (10 % تک محدود) کے بعد حاصل کرتے ہیں۔
- Optional holdbacks governance کو disputes کے دوران steward payouts روکنے دیتے ہیں۔
- Settlement bundles ایک `ledger_projection` بلاک ظاہر کرتے ہیں جس میں `Transfer`
  ISIs شامل ہوں تاکہ automation XOR movements کو براہ راست Torii میں پوسٹ کر سکے۔

## سروسز اور automation

| Component | Purpose | Evidence |
|-----------|---------|----------|
| `sns_settlementd` | Policy apply کرتا ہے، bundles sign کرتا ہے، `/v1/sns/settlements` expose کرتا ہے۔ | JSON bundle + hash. |
| Settlement queue & writer | Idempotent queue + ledger submitter جو `iroha_cli app sns settlement ledger` سے چلتا ہے۔ | bundle hash <-> tx hash manifest. |
| Reconciliation job | Daily diff + monthly statement `docs/source/sns/reports/` کے تحت۔ | Markdown + JSON digest. |
| Refund desk | Governance-approved refunds via `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

CI helpers ان flows کو mirror کرتے ہیں:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Observability اور reporting

- Dashboards: `dashboards/grafana/sns_payment_settlement.json` treasury vs
  steward totals، referral payouts، queue depth، اور refund latency کے لئے۔
- Alerts: `dashboards/alerts/sns_payment_settlement_rules.yml` pending age،
  reconciliation failures، اور ledger drift کو monitor کرتا ہے۔
- Statements: daily digests (`settlement_YYYYMMDD.{json,md}`) monthly reports
  (`settlement_YYYYMM.md`) میں roll ہوتے ہیں، جو Git اور governance object store
  (`s3://sora-governance/sns/settlements/<period>/`) دونوں پر upload ہوتے ہیں۔
- Governance packets dashboards، CLI logs اور approvals کو council sign-off سے
  پہلے bundle کرتے ہیں۔

## رول آؤٹ چیک لسٹ

1. quote + ledger helpers کو prototype کریں اور staging bundle capture کریں۔
2. `sns_settlementd` کو queue + writer کے ساتھ لانچ کریں، dashboards wire کریں،
   اور alert tests چلائیں (`promtool test rules ...`).
3. refund helper اور monthly statement template فراہم کریں؛ artefacts کو
   `docs/portal/docs/sns/reports/` میں mirror کریں۔
4. partner rehearsal چلائیں (پورا مہینہ settlements) اور governance vote capture
   کریں جو SN-5 کو complete نشان زد کرے۔

درست schema definitions، open questions اور مستقبل کی amendments کے لئے سورس
دستاویز کی طرف رجوع کریں۔
