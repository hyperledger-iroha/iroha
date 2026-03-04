---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تخلص من نقع الفطريات SF-2c

التاريخ: 2026-03-21

## Область

هذا هو الحل الأمثل لتحديد اختبارات نقع الإفرازات SoraFS وتفعيلها،
تم إغلاقه على البطاقة المتأخرة SF-2c.

- ** نقع متعدد الموفر لمدة 30 يومًا:** انتهى
  `capacity_fee_ledger_30_day_soak_deterministic` ف
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  يشكل Harness مقدمي خدمات الحيوانات الأليفة، ويصل إلى 30 دقيقة من التسوية و
  تأكد من أن هذا دفتر الأستاذ قد تمت الموافقة عليه بشكل لا لبس فيه
  مشروع. تم اختبار Blake3 Digest (`capacity_soak_digest=...`)، وهو CI
  يمكنك التقاط لقطة قانونية والتعرف عليها.
- **نصائح لعدم السداد:** الالتزام
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (هذا هو الملف). يؤكد الاختبار أن الطرق الموضحة هي الضربات وفترات التهدئة والخطوط المائلة
  يتضمن دفتر الأستاذ للضمانات والأوراق المالية تحديدًا.

## الاختيار

نقع المثبتات محليًا:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

تنتهي الاختبارات لمدة ثانية على الكمبيوتر المحمول القياسي ولا تحتاج إلى ذلك
تركيبات داخلية.

## Наблюдаемость

Torii يعرض لقطات سريعة لموفري الائتمان من خلال دفاتر الرسوم، لذلك
يمكن أن تكون لوحات العدادات بوابة متوازنة وتوازن ضربات الجزاء:-الراحة: `GET /v1/sorafs/capacity/state` تم تسجيلها `credit_ledger[*]`,
  التي يتم تسليمها إلى دفتر الأستاذ، والتي تم اختبارها في نقع الخصية. سم.
  `crates/iroha_torii/src/sorafs/registry.rs`.
- استيراد Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` stroit
  إضرابات التصدير، مبالغ كبيرة من المال والضمانات، لذلك
  يمكن أن يتم تنفيذ الأمر الأخير من خلال نقع خط الأساس مع الكائنات الحية.

## Дальнейсие саги

- قم بتخطيط بوابات البوابة الإلكترونية في CI لتجديد نقع تيستا (طبقة الدخان).
- قم بإعادة كتابة اللوحة Grafana celemscrape Torii بعد بدء تصدير القياس عن بعد في السوق.