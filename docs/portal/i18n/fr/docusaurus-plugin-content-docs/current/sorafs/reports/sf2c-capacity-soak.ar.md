---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير اختبار trempage لتراكم سعة SF-2c

Date de sortie : 2026-03-21

## النطاق

يسجل هذا التقرير اختبارات Trempage الحتمية لتراكم سعة SoraFS والمدفوعات المطلوبة ضمن مسار خطة SF-2c.

- **اختبار trempage متعدد المزوّدين لمدة 30 يوما:** يتم عبر
  `capacity_fee_ledger_30_day_soak_deterministic` dans
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  يقوم harnais بإنشاء خمسة مزودين، ويمتد عبر 30 نافذة تسوية، ويتحقق من أن إجماليات
  ledger تطابق إسقاطا مرجعيا محسوبا بشكل مستقل. يخرج الاختبار digest par Blake3
  (`capacity_soak_digest=...`) حتى تتمكن CI من التقاط اللقطة القياسية ومقارنتها.
- **عقوبات نقص التسليم:** تُفرض بواسطة
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (نفس الملف). يؤكد الاختبار أن عتبات strikes وcooldowns وslashing للضمان وعدّادات
  grand livre تبقى حتمية.

## التنفيذ

شغّل تحقق trempage محليا باستخدام:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

تكتمل الاختبارات في أقل من ثانية على حاسوب محمول قياسي ولا تتطلب luminaires خارجية.

## الرصد

يعرض Torii الآن لقطات رصيد المزوّدين جنبًا إلى جنب مع fee ledgers حتى تتمكن لوحات المتابعة
من الضبط على الأرصدة المنخفضة وpenalty strikes :

- REST : `GET /v2/sorafs/capacity/state` pour `credit_ledger[*]` pour `credit_ledger[*]`
  Vous devez utiliser le grand livre pour tremper. راجع
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importation Grafana : `dashboards/grafana/sorafs_capacity_penalties.json`
  عدادات strikes المصدّرة وإجمالي العقوبات والضمان المربوط حتى يتمكن فريق
  Les lignes de base sont trempées dans les lignes de base.

## المتابعة

- جدولة تشغيل بوابة أسبوعي في CI لإعادة تشغيل اختبار trempage (niveau de fumée).
- Utilisez Grafana pour gratter et Torii pour utiliser la télémétrie.