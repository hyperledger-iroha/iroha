---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/operations-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قواعد اللعبة التي تمارسها العمليات
العنوان: دليل الاستغلال SoraFS
Sidebar_label: دليل الاستغلال
الوصف: أدلة الاستجابة للحوادث وإجراءات تدريبات الفوضى للمشغلين SoraFS.
---

:::ملاحظة المصدر الكنسي
تعكس هذه الصفحة دليل التشغيل بشكل مستمر في `docs/source/sorafs_ops_playbook.md`. قم بمزامنة النسختين حتى يتم هجرة وثائق أبو الهول تمامًا.
:::

## المراجع الأساسية

- أنشطة المراقبة : راجع لوحات المعلومات Grafana sous `dashboards/grafana/` وقواعد التنبيه Prometheus في `dashboards/alerts/`.
- كتالوج المقاييس : `docs/source/sorafs_observability_plan.md`.
- أسطح القياس عن بعد للمنسق : `docs/source/sorafs_orchestrator_plan.md`.

## ماتريس ديسكالاد| الأولوية | أمثلة على التخفيض | مدير تحت الطلب | النسخ الاحتياطي | ملاحظات |
|----------|--------------------------|------------------|--------|-------|
| ص1 | بوابة عمومية، معدل قوة الأداء الكامل > 5% (15 دقيقة)، تراكم النسخ المتماثل كل 10 دقائق | تخزين SRE | إمكانية الملاحظة TL | قم بإشراك مجلس الإدارة في حالة مرور التأثير لمدة 30 دقيقة. |
| ص2 | انتهاك SLO لبوابة الكمون الإقليمية، صورة لإعادة محاولة التنسيق بدون تأثير SLA | إمكانية الملاحظة TL | تخزين SRE | مواصلة الطرح لمنع ظهور البيانات الجديدة. |
| ص3 | تنبيهات غير نقدية (جامدة البيانات، السعة 80-90%) | فرز المدخول | نقابة العمليات | À leaver dans le prochain jour ouvré. |

## بوابة Panne / Disponibilité Dégradée

**الكشف**

- التنبيهات: `SoraFSGatewayAvailabilityDrop`، `SoraFSGatewayLatencySlo`.
- لوحة القيادة: `dashboards/grafana/sorafs_gateway_overview.json`.

**الإجراءات فورية**

1. قم بتأكيد المنفذ (المزود الفريد مقابل الأسطول) عبر لوحة الطلبات.
2. قم بتغيير المسار Torii إلى المزودين (إذا كان المزود متعدد الاستخدامات) بناءً على `sorafs_gateway_route_weights` في تهيئة العمليات (`docs/source/sorafs_gateway_self_cert.md`).
3. إذا كان جميع الموردين متأثرين، فقم بتنشيط "الجلب المباشر" الاحتياطي لعملاء CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**- تحقق من استخدام الرموز المميزة للبث وفقًا لـ `sorafs_gateway_stream_token_limit`.
- افحص بوابة السجلات بحثًا عن أخطاء TLS أو القبول.
- قم بتنفيذ `scripts/telemetry/run_schema_diff.sh` للتحقق من أن المخطط المُصدَّر عبر البوابة يتوافق مع الإصدار الحالي.

** خيارات العلاج **

- إعادة تشغيل بوابة العملية المؤثرة بشكل فريد ; تجنب إعادة التدوير لجميع المجموعة إذا كان هناك العديد من الموردين.
- زيادة مؤقتة في حد رموز البث بنسبة 10-15% في حالة تأكيد التشبع.
- إعادة الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) بعد التثبيت.

**ما بعد الحادث**

- قم بإرجاع P1 بعد الوفاة مع `docs/source/sorafs/postmortem_template.md`.
- خطط لفوضى المتابعة إذا كان العلاج ضروريًا من خلال التدخلات اليدوية.

## Pic d’échecs de preuve (PoR / PoTR)

**الكشف**

- التنبيهات: `SoraFSProofFailureSpike`، `SoraFSPoTRDeadlineMiss`.
- لوحة القيادة: `dashboards/grafana/sorafs_proof_integrity.json`.
- القياس عن بعد : `torii_sorafs_proof_stream_events_total` والأحداث `sorafs.fetch.error` مع `provider_reason=corrupt_proof`.

**الإجراءات فورية**

1. قم بإظهار عمليات قبول البيانات الجديدة في سجل البيانات (`docs/source/sorafs/manifest_pipeline.md`).
2. إخطار الإدارة بتعليق تحريضات الموردين المتأثرة.

**الفرز**- تحقق من عمق ملف التحديات PoRface `sorafs_node_replication_backlog_total`.
- التحقق من خط أنابيب التحقق المسبق (`crates/sorafs_node/src/potr.rs`) لعمليات النشر الأخيرة.
- قارن إصدارات البرامج الثابتة الخاصة بالموردين مع مسجلي المشغلين.

** خيارات العلاج **

- قم بإلغاء قفل عمليات إعادة التشغيل PoR عبر `sorafs_cli proof stream` مع البيان الأخير.
- إذا كانت الإجراءات المتخذة تتبع أسلوبًا متماسكًا، فقم بإيقاف مورد المجموعة النشط في الوقت الحالي عن سجل الحوكمة واضطر إلى تحديث لوحات النتائج الخاصة بالمدير.

**ما بعد الحادث**

- قم بتنفيذ سيناريو الحفر الفوضوي قبل نشر السلسلة في الإنتاج.
- قم بتسليم التعليمات في قالب ما بعد الوفاة واستكمل قائمة التحقق من تأهيل الموردين يوميًا.

## تأخير النسخ / تراكم الأعمال المتراكمة

**الكشف**

- التنبيهات: `SoraFSReplicationBacklogGrowing`، `SoraFSCapacityPressure`. استيراد
  `dashboards/alerts/sorafs_capacity_rules.yml` ثم قم بالتنفيذ
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  الترويج المسبق لـ qu’Alertmanager يعكس المستندات التالية.
- لوحة القيادة: `dashboards/grafana/sorafs_capacity_health.json`.
- المقاييس : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**الإجراءات فورية**

1. تحقق من باب السجل (المزود الفريد أو الأسطول) وقم بإيقاف مفاتيح النسخ غير الأساسية مؤقتًا.
2. إذا كانت الأعمال المتراكمة معزولة، فقم بإعادة توجيه الأوامر الجديدة إلى الموردين البديلين مؤقتًا عبر جدولة النسخ.**الفرز**

- افحص منسق أجهزة القياس عن بعد للطائرات من أجل إعادة المحاولات التي قد تؤدي إلى تفجير التراكم.
- تأكد من أن أسلاك التخزين بها مساحة كافية للرأس (`sorafs_node_capacity_utilisation_percent`).
- قم بمراجعة التغييرات الأخيرة في التكوين (استمر في متابعة ملف تعريف القطعة وإيقاع البروفات).

** خيارات العلاج **

- قم بتنفيذ `sorafs_cli` مع الخيار `--rebalance` لإعادة توزيع المحتوى.
- قم بتوسيع أدوات النسخ أفقيًا لتأثير المورد.
- قم بإلغاء تنشيط البيانات لإعادة ضبط نوافذ TTL.

**ما بعد الحادث**

- قم بتخطيط مثقاب كهربائي لتأثيرات التشبع المصاحبة.
- قم بتحديث وثائق SLA للنسخ المتماثل في `docs/source/sorafs_node_client_protocol.md`.

## وتيرة تدريبات الفوضى

- **Trimestriel**: محاكاة مجمعة لبوابة اللوحة + معدل إعادة محاولة التنسيق.
- **Semestriel** : حقن الشوائب PoR/PoTR على اثنين من المزودين مع الاسترداد.
- **الفحص الفوري للدورة الشهرية** : سيناريو تأخير النسخ مع بيانات التدريج.
- Suivez les Drills dans le runbook log Partagé (`ops/drill-log.md`) عبر :

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

- التحقق من السجل قبل الالتزامات:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- استخدم `--status scheduled` للمثاقب القادمة و`pass`/`fail` للتشغيلات المنتهية و`follow-up` عند استمرار الإجراءات المفتوحة.
- استبدل الوجهة بـ `--log` لعمليات التشغيل الجافة أو التحقق التلقائي؛ بدون هذا، يستمر النص في العمل `ops/drill-log.md`.

## قالب ما بعد الوفاة

استخدم `docs/source/sorafs/postmortem_template.md` لكل حادث P1/P2 وللاسترجاع من تدريبات الفوضى. يغطي القالب التسلسل الزمني، وكمية التأثير، والعوامل المساهمة، والإجراءات التصحيحية، وعناصر التحقق من المتابعة.