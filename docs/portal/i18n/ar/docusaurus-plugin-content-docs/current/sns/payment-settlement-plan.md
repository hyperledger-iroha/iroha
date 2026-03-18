---
id: payment-settlement-plan
lang: ar
direction: rtl
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# خطة الدفع والتسوية لـ SNS

> المصدر القياسي: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

تقدم مهمة خارطة الطريق **SN-5 -- Payment & Settlement Service** طبقة دفع
حتمية لخدمة اسماء سورا. يجب ان يصدر كل تسجيل او تجديد او استرداد payload
Norito منظم حتى تتمكن الخزينة وstewards والحوكمة من اعادة تشغيل التدفقات
المالية دون جداول بيانات. تلخص هذه الصفحة المواصفات لجمهور البوابة.

## نموذج الايرادات

- الرسم الاساسي (`gross_fee`) مشتق من مصفوفة تسعير المسجل.
- تتلقى الخزينة `gross_fee x 0.70`، ويتلقى stewards الباقي بعد خصم مكافاة
  referral (حد 10 %).
- تسمح holdbacks الاختيارية للحوكمة بايقاف مدفوعات stewards اثناء النزاعات.
- تكشف حزم settlement عن كتلة `ledger_projection` مع ISIs `Transfer` الفعلية
  بحيث يمكن للاتمتة ترحيل حركات XOR مباشرة الى Torii.

## الخدمات والاتمتة

| المكون | الغرض | الدليل |
|--------|-------|--------|
| `sns_settlementd` | يطبق السياسة، يوقع الحزم، ويعرض `/v1/sns/settlements`. | حزمة JSON + hash. |
| Settlement queue & writer | طابور idempotent + مرسل للدفتر يقوده `iroha_cli app sns settlement ledger`. | بيان bundle hash <-> tx hash. |
| Reconciliation job | فرق يومي + بيان شهري تحت `docs/source/sns/reports/`. | Markdown + JSON digest. |
| Refund desk | استردادات معتمدة حوكما عبر `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

تعكس مساعدات CI هذه التدفقات:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## الرصد والتقارير

- لوحات المتابعة: `dashboards/grafana/sns_payment_settlement.json` لاجمالي
  الخزينة مقابل stewards، مدفوعات referral، عمق الطابور، وزمن استجابة
  الاسترداد.
- التنبيهات: `dashboards/alerts/sns_payment_settlement_rules.yml` تراقب عمر
  الانتظار، فشل المطابقة، وانحراف الدفتر.
- البيانات: digests اليومية (`settlement_YYYYMMDD.{json,md}`) تتراكم في تقارير
  شهرية (`settlement_YYYYMM.md`) ترفع الى Git ومخزن كائنات الحوكمة
  (`s3://sora-governance/sns/settlements/<period>/`).
- حزم الحوكمة تجمع لوحات المتابعة وسجلات CLI والموافقات قبل اعتماد المجلس.

## قائمة فحص الاطلاق

1. نمذج مساعدات quote + ledger والتقط حزمة staging.
2. اطلق `sns_settlementd` مع queue + writer، اربط لوحات المتابعة، وشغل اختبارات
   التنبيه (`promtool test rules ...`).
3. سلم مساعد الاسترداد وقالب البيان الشهري؛ وانسخ الاثار الى
   `docs/portal/docs/sns/reports/`.
4. شغل بروفة شريك (شهر كامل من settlements) والتقط تصويت الحوكمة الذي يعلن
   اكتمال SN-5.

ارجع الى المستند المصدر للتعريفات الدقيقة للمخططات، الاسئلة المفتوحة،
والتعديلات المستقبلية.
