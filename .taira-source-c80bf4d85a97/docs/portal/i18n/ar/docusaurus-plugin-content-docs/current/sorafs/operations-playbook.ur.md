---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/operations-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قواعد اللعبة التي تمارسها العمليات
العنوان: SoraFS تم النشر
Sidebar_label: سجل الآن
الوصف: SoraFS حواجز للاستجابة للحوادث وحفر الفوضى من خلال العمل.
---

:::ملاحظة مستند ماخذ
هذه الصفحة `docs/source/sorafs_ops_playbook.md` ستوضح لك دليل التشغيل الخاص بك. عندما لم يكتمل انتقال أبو الهول إلى عالم جديد، نقول ما هو أفضل.
:::

## كليدی حوالہ جات

- أصول إمكانية المراقبة: `dashboards/grafana/` تحتوي على Grafana لوحات المعلومات و `dashboards/alerts/` تحتوي على Prometheus قواعد التنبيه.
- الكتالوج المتري: `docs/source/sorafs_observability_plan.md`.
- أسطح القياس عن بعد للمنسق: `docs/source/sorafs_orchestrator_plan.md`.

## مصفوفة التصعيد

| ترجیح | ٹرگر مثال | بنیادی عند الطلب | بيك اپ | أخبار |
|--------|---------------------|--------|------|--------|--|
| ص1 | انقطاع البوابة العالمية، معدل فشل PoR > 5% (15 نسبه مئويه)، تراكم النسخ المتماثل أكثر من 10 مرات | تخزين SRE | إمكانية الملاحظة TL | إذا حدث 30 شهرًا، فسيقوم مجلس الإدارة بالمشاركة. |
| ص2 | علاقی بوابة الكمون SLO خرق، منسق إعادة المحاولة ارتفاع بغیر SLA حدث کے | إمكانية الملاحظة TL | تخزين SRE | يُظهر الطرح الجديد للبوابة. |
| ص3 | غير اہم التنبيهات (الجاذبية الواضحة، السعة 80–90%) | فرز المدخول | نقابة العمليات | كل شيء على ما يرام. |

## انقطاع البوابة / تدهور التوافر

**الكشف**

- التنبيهات: `SoraFSGatewayAvailabilityDrop`، `SoraFSGatewayLatencySlo`.
- لوحة القيادة: `dashboards/grafana/sorafs_gateway_overview.json`.

**إجراءات فورية**1. لوحة معدل الطلب نطاق النطاق (مزود واحد مقابل الأسطول).
2. إذا قمت بموفر متعدد وتكوين ops (`docs/source/sorafs_gateway_self_cert.md`) قم بالتحويل إلى `sorafs_gateway_route_weights` إلى توجيه Torii من خلال موفري الخدمة بشكل صحيح.
3. إذا كان جميع موفري الخدمة يتواصلون مع عملاء CLI/SDK للتنشيط الاحتياطي "الجلب المباشر" (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**

- `sorafs_gateway_stream_token_limit` مقابل استخدام رمز الدفق المميز.
- فحص أخطاء TLS أو القبول في سجلات البوابة.
- `scripts/telemetry/run_schema_diff.sh` قم بتشغيل بوابة المخطط المصدرة والإصدار المتوقع الذي يتطابق مع الثابت.

** خيارات العلاج **

- يتم إنفاق عملية بوابة متاثر بشكل إبداعي؛ لم يتم إنشاء مجموعة إعادة التدوير بعد مع وجود موفري خدمات متعددين.
- إذا انتهى التشبع وقم ببث الحد الأقصى للرمز المميز الذي يصل إلى 10-15% تقريبًا.
- الاستحصال بعد إعادة الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`).

**ما بعد الحادث**

- `docs/source/sorafs/postmortem_template.md` استخدام کرتے ہوئے P1 بعد الوفاة کریں.
- إذا كان العلاج يتضمن تدخلات يدوية، فيجب عليك متابعة تدريبات الفوضى.

## ارتفاع دليل الفشل (PoR / PoTR)

**الكشف**

- التنبيهات: `SoraFSProofFailureSpike`، `SoraFSPoTRDeadlineMiss`.
- لوحة القيادة: `dashboards/grafana/sorafs_proof_integrity.json`.
- القياس عن بعد: أحداث `torii_sorafs_proof_stream_events_total` و `sorafs.fetch.error`.

**إجراءات فورية**

1. سجل البيان الذي يشير إلى تجميد قبول البيان الجديد (`docs/source/sorafs/manifest_pipeline.md`).
2. تقوم الحوكمة بإخطار مقدمي خدمات الائتمان بإيقاف الحوافز مؤقتًا.**الفرز**

- عمق قائمة انتظار التحدي PoR الذي `sorafs_node_replication_backlog_total` يقابل شيك كريں.
- حالة عمليات النشر لخط أنابيب التحقق من الإثبات (`crates/sorafs_node/src/potr.rs`) للتحقق من صحة البيانات.
- مقارنة إصدارات البرامج الثابتة للموفر مع سجل المشغل.

** خيارات العلاج **

- أحدث بيان تم تشغيله باستخدام `sorafs_cli proof stream` في عمليات إعادة تشغيل PoR.
- في حالة فشل البراهين في المسلسل، يمكنك تسجيل الحوكمة من خلال موفر المجموعة النشطة ولوحات تسجيل المنسق لتحديث السجل.

**ما بعد الحادث**

- سيناريو نشر الفوضى PoR سيناريو حفر الفوضى.
- قالب ما بعد الوفاة يلتقط الدروس وقائمة مراجعة تأهيل الموفر.

## تأخر النسخ المتماثل / نمو الأعمال المتراكمة

**الكشف**

- التنبيهات: `SoraFSReplicationBacklogGrowing`، `SoraFSCapacityPressure`. `dashboards/alerts/sorafs_capacity_rules.yml` شحن و
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  الترويج له عتبات موثقة من قبل Alertmanager والتي تعكس الائتمان.
- لوحة القيادة: `dashboards/grafana/sorafs_capacity_health.json`.
- المقاييس: `sorafs_node_replication_backlog_total`، `sorafs_node_manifest_refresh_age_seconds`.

**إجراءات فورية**

1. تراكم الأعمال المتراكمة هو نطاق صغير (مزود واحد أو أسطول) ومهام النسخ المتماثل الضرورية الأخرى.
2. إذا تم عزل الأعمال المتراكمة، فستؤدي إلى ظهور أوامر جديدة لموفري الخدمة البديلين الذين يقومون بإعادة تعيين جدولة النسخ المتماثل.

**الفرز**- يمكن للقياس عن بعد للمنسق إعادة محاولة الدفقات مما يؤدي إلى تراكم الأعمال المتراكمة.
- أهداف التخزين هي الإرتفاع (`sorafs_node_capacity_utilisation_percent`).
- تغييرات حالة التكوين (تحديثات الملف الشخصي، إيقاع الإثبات)

** خيارات العلاج **

- `sorafs_cli` و `--rebalance` يقوم بإعادة توزيع المحتوى.
- مزود خدمة النسخ المتماثل للعاملين على نطاق أفقي.
- محاذاة نوافذ TTL لمشغل تحديث البيان.

**ما بعد الحادث**

- فشل تشبع الموفر في حفر سعة المركز شيڈول کریں۔
- وثائق النسخ المتماثل لاتفاقية مستوى الخدمة (SLA) `docs/source/sorafs_node_client_protocol.md`.

## إيقاع حفر الفوضى

- **ربع سنوي**: انقطاع البوابة المدمج + إعادة محاولة المنسق لمحاكاة العاصفة.
- **نصف سنوي**: يتم توفير حقن فشل PoR/PoTR من قبل موفري الاسترداد.
- **الفحص الفوري الشهري**: يظهر التدريج سيناريو تأخر النسخ المتماثل.
- التدريبات على سجل دليل التشغيل المشترك (`ops/drill-log.md`) هي الخطة التالية:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- يلتزم بتسجيل الدخول للتحقق من صحة السجل:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- يتم استخدام هذه التدريبات لـ `--status scheduled`، ويتم تشغيلها بالكامل لـ `pass`/`fail`، ويتم استخدام عناصر العمل لـ `follow-up`.
- عمليات التشغيل الجافة أو التحقق الآلي للوجهة `--log` ستتجاوز البطاقة؛ لقد تم إنشاء النص `ops/drill-log.md`.

## قالب ما بعد الوفاةيتم استخدام حادثة P1/P2 وحفر الفوضى بأثر رجعي لـ `docs/source/sorafs/postmortem_template.md`. إنه جدول زمني للقالب، وتقدير التأثير، والعوامل المساهمة، والإجراءات التصحيحية، ومهام التحقق من المتابعة.