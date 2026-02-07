---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# وصلة امتصاص السعة التراكمية SF-2c

البيانات:2026-03-21

##اسكوبو

هذا الارتباط يسجل محددات امتصاص التراكم والسعة SoraFS
الطلبات على خريطة الطريق SF-2c.

- **Soak multiprovider لمدة 30 يومًا:** يتم تنفيذه
  `capacity_fee_ledger_30_day_soak_deterministic` م
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  يا مقدمي خدمات تسخير Instancia 5, كوبر 30 جانيلا دي تسوية ه
  التحقق من أن دفتر الأستاذ بأكمله يتوافق مع مشروع مرجعي
  حساب شكلي مستقل. يا الخصية تنبعث من هضم Blake3
  (`capacity_soak_digest=...`) حتى تتمكن CI من التقاط ومقارنة اللقطة
  canonico.
- **Penalidades por subentrega:** Aplicadas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (الملف مفتوح). أكد الاختبار أنه يحد من الضربات وفترات التهدئة والخطوط المائلة
  الضمانات والضوابط المحددة لدفتر الأستاذ.

## تنفيذ

نفذ كـ validacoes de soak localmente com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

الخصيتين مكتملتان في أقل من ثانية على جهاز كمبيوتر محمول ولا يتطلبانه
تركيبات خارجية.

## قابلية الملاحظة

Torii يعرض الآن لقطات من ائتمان مقدمي الخدمات جنبًا إلى جنب مع دفاتر الرسوم للوحات المعلومات
بوابة بوسام فازر في ضربات الجزاء و ضربات الجزاء:- الباقي: `GET /v1/sorafs/capacity/state` retorna entradas `credit_ledger[*]` que
  قم بإعادة ملء الحقول التي تم التحقق منها من دفتر الأستاذ دون اختبار النقع. فيجا
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importacao Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` نظام التشغيل
  مواجهات الضربات المصدرة، إجمالي العقوبات والضمانات الإضافية التي تدفع لك
  يمكن للوقت عند الطلب مقارنة الخطوط الأساسية للنقع بالبيئة المحيطة بالإنتاج.

##متابعة

- جدول تنفيذي لعدة أشهر من بوابة CI لإعادة تنفيذ أو اختبار النقع (طبقة الدخان).
- قم بإرسال اللوحة Grafana مع إزالة Torii كما تريد تصدير أجهزة القياس عن بعد المنتجة إلى التشغيل.