---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c نقع تراكم السعة

التاريخ: 2026-03-21

## اسكوپ

يحتوي هذا التقرير على تقرير SF-2c المتضمن استحقاق السعة والدفع SoraFS واختبارات الامتصاص الحتمية.

- **30 دن نقع متعدد الموفر:**
  `capacity_fee_ledger_30_day_soak_deterministic` ذريع
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` هو كليا جاتا.
  مقدمي خدمات تسخير پانچ بناتنا ہے، 30 مستوطنة ونڈوز کا احاطہ کرتا ہے، اور
  يعد فحص البطاقة وإجماليات دفتر الأستاذ أمرًا متعدد الاستخدامات وفقًا للإسقاط المرجعي.
  يوجد في Blake3 Digest (`capacity_soak_digest=...`) خارج اللقطة الأساسية لـ CI
  التقاط والفرق كسے۔
- **عقوبات نقص التسليم:**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (هذا هو الاسم الشائع) الذي نجح في تحقيق هذا الهدف. ٹیسٹ تصدیق كرتا يضرب العتبات، فترات التهدئة، الخطوط المائلة الجانبية
  وعدادات دفتر الأستاذ حتمية.

## التنفيذ

عمليات التحقق من صحة النقع مخففة:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

لا داعي للقلق بشأن وجود جهاز كمبيوتر محمول عام للسرعة مكتمل بالبوابات وتركيبات البيرون الضرورية.

## إمكانية الملاحظة

Torii هو لقطات ائتمانية لموفر الخدمة ودفاتر رسوم الرسوم وتسجيل لوحات المعلومات والأرصدة وضربات الجزاء على البوابة:- REST: `GET /v1/sorafs/capacity/state` `credit_ledger[*]` إدخالات وابس كرتا واختبار جو نقع للتحقق من النهاية
  حقول دفتر الأستاذ هي الاسم المستعار. رائع
  `crates/iroha_torii/src/sorafs/registry.rs`.
- استيراد Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` عدادات الضربات المصدرة، إجماليات العقوبات،
  والضمانات المستعبدة عبارة عن قطعة أرض وتسمح عند الطلب بنقع خطوط الأساس في البيئات الحية وتوازنها.

##متابعة

- تعمل بوابة الحرب CI على اختبار نقع الشيول (طبقة الدخان).
- عند تصدير القياس عن بعد للإنتاج مباشرة، يمكنك استخدام لوحة Grafana وأهداف كشط Torii.