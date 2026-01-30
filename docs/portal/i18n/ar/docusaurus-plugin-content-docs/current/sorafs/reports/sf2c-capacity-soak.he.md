---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa555e044155ae667e7c3427bdfc744a103cec6307a3e39d222b217c871b57ab
source_last_modified: "2025-11-14T04:43:22.258261+00:00"
translation_last_reviewed: 2026-01-30
---

# تقرير اختبار soak لتراكم سعة SF-2c

التاريخ: 2026-03-21

## النطاق

يسجل هذا التقرير اختبارات soak الحتمية لتراكم سعة SoraFS والمدفوعات المطلوبة ضمن مسار خطة SF-2c.

- **اختبار soak متعدد المزوّدين لمدة 30 يوما:** يتم عبر
  `capacity_fee_ledger_30_day_soak_deterministic` في
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  يقوم harness بإنشاء خمسة مزودين، ويمتد عبر 30 نافذة تسوية، ويتحقق من أن إجماليات
  ledger تطابق إسقاطا مرجعيا محسوبا بشكل مستقل. يخرج الاختبار digest من Blake3
  (`capacity_soak_digest=...`) حتى تتمكن CI من التقاط اللقطة القياسية ومقارنتها.
- **عقوبات نقص التسليم:** تُفرض بواسطة
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (نفس الملف). يؤكد الاختبار أن عتبات strikes وcooldowns وslashing للضمان وعدّادات
  ledger تبقى حتمية.

## التنفيذ

شغّل تحقق soak محليا باستخدام:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

تكتمل الاختبارات في أقل من ثانية على حاسوب محمول قياسي ولا تتطلب fixtures خارجية.

## الرصد

يعرض Torii الآن لقطات رصيد المزوّدين جنبًا إلى جنب مع fee ledgers حتى تتمكن لوحات المتابعة
من الضبط على الأرصدة المنخفضة وpenalty strikes:

- REST: `GET /v1/sorafs/capacity/state` يعيد إدخالات `credit_ledger[*]` التي
  تعكس حقول ledger التي تم التحقق منها في اختبار soak. راجع
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana import: `dashboards/grafana/sorafs_capacity_penalties.json` يرسم
  عدادات strikes المصدّرة وإجمالي العقوبات والضمان المربوط حتى يتمكن فريق
  المناوبة من مقارنة baselines soak مع البيئات الحية.

## المتابعة

- جدولة تشغيل بوابة أسبوعي في CI لإعادة تشغيل اختبار soak (smoke-tier).
- توسيع لوحة Grafana بأهداف scrape من Torii بمجرد تفعيل صادرات telemetry الإنتاجية.
