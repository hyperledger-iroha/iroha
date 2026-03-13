---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير امتصاص السعة المتراكمة SF-2c

التاريخ: 2026-03-21

## بورتيه

تشير هذه العلاقة إلى اختبارات امتصاص التراكم ودفع السعة SoraFS المطلوبة
في المسار SF-2c.

- **Soak multiprovider لمدة 30 يوم:** Exécuté par
  `capacity_fee_ledger_30_day_soak_deterministic` في
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  مزودو خدمة تسخير المثيلات الخمس، يغطيون 30 نافذة تسوية، وما إلى ذلك
  صحة أن إجمالي دفتر الأستاذ يتوافق مع إسقاط مرجعي
  حساب الاستقلال. تم إجراء الاختبار على Blake3 (`capacity_soak_digest=...`)
  حتى يتمكن CI من التقاط اللقطة التقليدية ومقارنتها.
- **Pénalités de sous-livraison:** Appliquées par
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (مذكراتي). يؤكد الاختبار أن سلسلة الضربات وفترات التهدئة
  لا تزال شرائح الضمانات وحسابات دفتر الأستاذ محددة.

## التنفيذ

إعادة التحقق من صحة النقع المحلية مع:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

تنتهي الاختبارات في أقل من ثانية على جهاز كمبيوتر محمول قياسي، ولا يجوز ذلك
ضروري لاعبا أساسيا خارجيا.

## قابلية الملاحظة

يعرض Torii الاحتفاظ بلقطات موفري الائتمان لأسعار دفاتر الرسوم للوحات المعلومات
بوابة قوية لفشل الجنود وضربات الجزاء:- الباقي: `GET /v2/sorafs/capacity/state` renvoie des entrées `credit_ledger[*]` qui
  تعكس صفائح دفتر الأستاذ التي تم التحقق منها من خلال اختبار النقع. إستفتاء
  `crates/iroha_torii/src/sorafs/registry.rs`.
- استيراد Grafana: تتبع الملفات `dashboards/grafana/sorafs_capacity_penalties.json`
  يقوم محاسبو الضربات المصدرة، وإجمالي العقوبات والضمانات بالتعامل مع ذلك
  يمكن للمعدات عند الطلب مقارنة خطوط الأساس للنقع مع البيئات الحية.

## سويفي

- Planifier des exécutions hebdomadaires de gate en CI لتجديد اختبار النقع (طبقة الدخان).
- قم بتوسيع اللوحة Grafana باستخدام أسلاك الخدش Torii مرة واحدة لصادرات القياس عن بعد للإنتاج
  sernt en ligne.