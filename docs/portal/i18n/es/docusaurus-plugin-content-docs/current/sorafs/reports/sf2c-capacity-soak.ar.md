---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير اختبار remojo لتراكم سعة SF-2c

التاريخ: 2026-03-21

## النطاق

Para ello, utilice el dispositivo de remojo SoraFS y el dispositivo SF-2c.

- **اختبار remojo متعدد المزوّدين لمدة 30 يوما:** يتم عبر
  `capacity_fee_ledger_30_day_soak_deterministic` aquí
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  يقوم arnés بإنشاء خمسة مزودين، ويمتد عبر 30 نافذة تسوية، ويتحقق من أن إجماليات
  libro mayor تطابق إسقاطا مرجعيا محسوبا بشكل مستقل. يخرج الاختبار resumen de Blake3
  (`capacity_soak_digest=...`) Haga clic en CI para conectar y desconectar.
- **عقوبات نقص التسليم:** تُفرض بواسطة
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (نفس الملف). يؤكد الاختبار أن عتبات huelgas وcooldowns للضمان وعدّادات
  libro mayor تبقى حتمية.

## التنفيذ

شغّل تحقق remojo محليا باستخدام:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

تكتمل الاختبارات في أقل من ثانية على حاسوب محمول قياسي ولا تتطلب accesorios خارجية.

## الرصد

يعرض Torii الآن لقطات رصيد المزوّدين جنبًا إلى جنب مع libros de tarifas حتى تتمكن لوحات المتابعة
من الضبط على الأرصدة المنخفضة و penas:

- RESTO: `GET /v1/sorafs/capacity/state` يعيد إدخالات `credit_ledger[*]` التي
  تعكس حقول التي تم التحقق منها في اختبار remojo. راجع
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importación Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` يرسم
  عدادات huelgas المصدّرة وإجمالي العقوبات والضمان المربوط حتى يتمكن فريق
  المناوبة من مقارنة remojo de líneas base مع البيئات الحية.

## المتابعة- جدولة تشغيل بوابة أسبوعي في CI لإعادة تشغيل اختبار remojo (nivel de humo).
- Utilice el Grafana para raspar y el Torii para realizar tareas de telemetría.