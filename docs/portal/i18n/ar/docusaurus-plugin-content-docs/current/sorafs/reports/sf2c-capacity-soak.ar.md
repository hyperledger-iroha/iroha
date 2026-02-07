---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير اختبار نقع لتراكم المخلفات SF-2c

التاريخ: 2026-03-21

## النطاق

سجل هذا التقرير السيولة نقع الحتمية لتراكم SoraFS والمدفوعات الأساسية ضمن مسار خطة SF-2c.

- **اختبار نقع المتحكمين المتعددين لمدة 30 يومًا:** يتم عبره
  `capacity_fee_ledger_30_day_soak_deterministic` في
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  يتم تحديد عدد خمسة من القنوات، ويمتد عبر 30 اتفاقية نافذة، ويتم التحقق من أن إجماليات
  تطابق دفتر الأستاذ بشكل مستقل مرجعيا. الخروج الاختبار من Blake3
  (`capacity_soak_digest=...`) حتى مجانا CI من التقاط اللقطة القياسية و البرتغالية.
- **عقوبات مختلفة:** تُفترض بواسطة
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (نفس الملف). أثبتت التجربة أن اعتبات Strikes وcooldowns وslashing للضمان والتعددات
  دفتر الأستاذ يستمر حتمية.

##التنفيذ

شغّل تحقق من نقع محليا باستخدام:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

اكتملت المعركة في أقل من ثانية على حاسوب ثابت ولا تتطلب تركيبات خارجية.

## الرصد

يُعرض Torii الآن لقطات رصيد المتحكمين جنبًا إلى جنب مع دفاتر الرسوم حتى يبدأ التحميل
من الضبط على الأرصدة المنخفضة وضربات الجزاء:

- REST: `GET /v1/sorafs/capacity/state` يعيد الإرسالات `credit_ledger[*]` التي
  حيازة دفتر الأستاذ الذي تم التحقق منه في اختبار نقع. إعادة النظر
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana استيراد : `dashboards/grafana/sorafs_capacity_penalties.json` يرسم
  عدد الضربات المصدّرة والجمالية والضمانات المربوطة حتى يتطور الفريق
  المناوبة من مقارنة خطوط الأساس مع البيئات الجائزة.

## متابعة- جدول تشغيل بوابة أسبوعية في CI مرة أخرى اختبار نقع (طبقة الدخان).
- النهائي لوحة Grafana بأهداف سكراب من Torii بمجرد تفعيل صادرات القياس عن بعد.