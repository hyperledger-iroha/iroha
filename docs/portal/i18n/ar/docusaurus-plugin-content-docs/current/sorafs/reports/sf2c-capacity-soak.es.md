---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إعلام امتصاص السعة التراكمية SF-2c

رسالة: 2026-03-21

## الكانس

قم بإبلاغنا بتسجيل اختبار محددات امتصاص السعة التراكمية SoraFS والدفعات
نطلب المساعدة من طريق SF-2c.

- **نقع متعدد لمدة 30 يومًا:** تم تشغيله
  `capacity_fee_ledger_30_day_soak_deterministic` ar
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  مقدمي خدمات El Harbour Instancia 5, Abarca 30 نوافذ التسوية ذ
  التحقق من أن إجمالي دفتر الأستاذ يتزامن مع عرض مرجعي
  حساب الشكل المستقل. التجربة تنبعث من هضم Blake3
  (`capacity_soak_digest=...`) حتى يتمكن CI من التقاط اللقطة ومقارنتها
  canónico.
- **Penalizaciones por subentrega:** Impuestas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (أرشيف ميسمو). يؤكد الاختبار أن مظلات الضربات وفترات التهدئة،
  تخفيضات الضمانات وضوابط دفتر الأستاذ تحافظ على المحددات.

## قذف

قم بتنفيذ عمليات التحقق من النقع محليًا باستخدام:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

تكتمل الاختبارات في أقل من ثانية على جهاز محمول قياسي ولا تتطلب
تركيبات خارجية.

## إمكانية الملاحظة

Torii يعرض الآن لقطات من موفري الائتمان جنبًا إلى جنب مع دفاتر رسوم الرسوم للوحات المعلومات
Puedan Gatear sobre Saldos Bajos وضربات الجزاء:- الباقي: `GET /v2/sorafs/capacity/state` devuelve entradas `credit_ledger[*]` que
  قم بإعادة النظر في مجالات دفتر الأستاذ التي تم التحقق منها في اختبار الامتصاص. الاصدار
  `crates/iroha_torii/src/sorafs/registry.rs`.
- استيراد Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` الرسوم البيانية
  صدرت مواجهات الضربات وإجمالي العقوبات والضمانات لضمان أن
  يمكن للمعدات عند الطلب مقارنة خطوط الأساس للنقع داخل الجسم الحي.

## متابعة

- برمجة عمليات تشغيل البوابة الفاصلة في CI لإعادة تشغيل اختبار النقع (طبقة الدخان).
- موسع اللوحة Grafana بأهداف كشط Torii أثناء عمليات التصدير
  القياس عن بعد للإنتاج متاح.