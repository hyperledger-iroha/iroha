---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operaciones
título: دليل تشغيل عمليات SoraFS
sidebar_label: دليل التشغيل
descripción: إرشادات الاستجابة للحوادث وإجراءات تدريبات الفوضى لمشغلي SoraFS.
---

:::nota المصدر المعتمد
تعكس هذه الصفحة الدليل التشغيلي الموجود ضمن `docs/source/sorafs_ops_playbook.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم ترحيل مجموعة توثيق Sphinx بالكامل.
:::

## المراجع الرئيسية

- Nombre del producto: راجع لوحات Grafana, `dashboards/grafana/` y تنبيهات Prometheus في `dashboards/alerts/`.
- Número de modelo: `docs/source/sorafs_observability_plan.md`.
- أسطح تليمترية المُنسِّق: `docs/source/sorafs_orchestrator_plan.md`.

## مصفوفة التصعيد

| الأولوية | أمثلة على المحفزات | المناوب الأساسي | الاحتياطي | ملاحظات |
|----------|--------------------|-----------------|----------|---------|
| P1 | انقطاع عالمي للبوابة, معدل فشل PoR أكبر من 5% (15 دقيقة), تراكم التكرار يتضاعف كل 10 دقائق | Almacenamiento SRE | Observabilidad TL | La limpieza del aparato dura 30 días. |
| P2 | خرق SLO لزمن تأخر البوابة الإقليمي، قفزة إعادة المحاولة في المُنسِّق دون تأثير SLA | Observabilidad TL | Almacenamiento SRE | واصل الإطلاق لكن أوقف المانيفستات الجديدة. |
| P3 | تنبيهات غير حرجة (تقادم المانيفست، السعة 80–90%) | فرز الاستقبال | Gremio de operaciones | عالج خلال يوم العمل التالي. |

## انقطاع البوابة / تدهور التوفر

**الكشف**

- Contenidos: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Nombre del producto: `dashboards/grafana/sorafs_gateway_overview.json`.

**إجراءات فورية**1. أكّد النطاق (مزوّد واحد مقابل الأسطول) عبر لوحة معدل الطلبات.
2. Presione Torii y presione el botón `sorafs_gateway_route_weights`. إعدادات العمليات (`docs/source/sorafs_gateway_self_cert.md`).
3. Haga clic en “búsqueda directa” en CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**

- تحقّق من استهلاك رموز stream مقابل `sorafs_gateway_stream_token_limit`.
- افحص سجلات البوابة لأخطاء TLS أو أخطاء القبول.
- Asegúrese de que `scripts/telemetry/run_schema_diff.sh` esté conectado a una fuente de alimentación.

**خيارات المعالجة**

- أعد تشغيل عملية البوابة المتأثرة فقط؛ تجنّب إعادة تدوير كامل العنقود إلا إذا فشل عدة مزوّدين.
- ارفع حد رموز stream بنسبة 10–15% مؤقتًا إذا تم تأكيد التشبع.
- أعد تشغيل autocertificación (`scripts/sorafs_gateway_self_cert.sh`) بعد الاستقرار.

**ما بعد الحادث**

- سجّل postmortem من فئة P1 باستخدام `docs/source/sorafs/postmortem_template.md`.
- جدولة تدريب فوضى لاحق إذا اعتمدت المعالجة على تدخلات يدوية.

## ارتفاع مفاجئ لفشل الإثبات (PoR / PoTR)

**الكشف**

- Contenidos: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Nombre del producto: `dashboards/grafana/sorafs_proof_integrity.json`.
- Contenido: `torii_sorafs_proof_stream_events_total` y `sorafs.fetch.error` de `provider_reason=corrupt_proof`.

**إجراءات فورية**

1. جمّد قبولات المانيفست الجديدة عبر تعليم سجل المانيفست (`docs/source/sorafs/manifest_pipeline.md`).
2. أخطر الحوكمة لإيقاف الحوافز للمزوّدين المتأثرين.

**الفرز**

- راجع عمق قائمة تحديات PoR مقابل `sorafs_node_replication_backlog_total`.
- تحقّق من مسار التحقق من الإثبات (`crates/sorafs_node/src/potr.rs`) للعمليات المنشورة مؤخرًا.
- قارن إصدارات firmware للمزوّدين مع سجل المشغلين.**خيارات المعالجة**

- Utilice el cable PoR `sorafs_cli proof stream` para conectarlo.
- إذا استمرت الإخفاقات، أزِل المزوّد من المجموعة النشطة عبر تحديث سجل الحوكمة وإجبار المُنسِّق على تحديث marcadores.

**ما بعد الحادث**

- نفّذ سيناريو تدريب فوضى PoR قبل نشر الإنتاج التالي.
- دوّن الدروس في قالب postmortem وحدّث قائمة تحقق تأهيل المزوّدين.

## تأخر التكرار / نمو التراكم

**الكشف**

- Contenidos: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. استورد
  `dashboards/alerts/sorafs_capacity_rules.yml` Código
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  قبل الترقية كي يعكس Alertmanager العتبات الموثقة.
- Nombre del producto: `dashboards/grafana/sorafs_capacity_health.json`.
- Idioma: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**إجراءات فورية**

1. تحقّق من نطاق التراكم (مزوّد منفرد أو الأسطول) وأوقف مهام التكرار غير الأساسية.
2. إذا كان التراكم معزولًا، أعد إسناد الطلبات الجديدة مؤقتًا إلى مزوّدين بدلاء عبر مجدول التكرار.

**الفرز**

- افحص تليمترية المُنسِّق بحثًا عن دفعات إعادة المحاولة التي قد تؤدي إلى تضخم التراكم.
- تأكد من أن أهداف التخزين لديها هامش كافٍ (`sorafs_node_capacity_utilisation_percent`).
- راجع تغييرات الإعداد الأخيرة (تحديثات ملفات تعريف القطع، وتيرة الإثباتات).

**خيارات المعالجة**

- Haga clic en `sorafs_cli` para conectar `--rebalance`.
- قم بتوسيع عمال التكرار أفقيًا للمزوّد المتأثر.
- فعّل تحديث المانيفست لإعادة محاذاة نوافذ TTL.

**ما بعد الحادث**

- جدولة تدريب سعة يركز على فشل تشبع المزوّدين.
- حدّث وثائق SLA الخاصة بالتكرار في `docs/source/sorafs_node_client_protocol.md`.

## وتيرة تدريبات الفوضى- **ربع سنوي**: محاكاة مجمعة لانقطاع البوابة + عاصفة إعادة محاولات المُنسِّق.
- **نصف سنوي**: حقن فشل PoR/PoTR عبر مزوّدين اثنين مع التعافي.
- **تحقق شهري سريع**: سيناريو تأخر التكرار باستخدام مانيفستات puesta en escena.
- تتبع التدريبات في سجل التشغيل المشترك (`ops/drill-log.md`) عبر:

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

- تحقّق من السجل قبل الالتزامات عبر:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Adaptador de corriente `--status scheduled`, adaptador de corriente, `pass`/`fail`, adaptador de corriente y adaptador de corriente `follow-up` تبقى بنود مفتوحة.
- غيّر الوجهة باستخدام `--log` للتشغيلات التجريبية أو التحقق الآلي؛ بدونه يستمر السكربت في تحديث `ops/drill-log.md`.

## قالب ما بعد الحادث

Utilice `docs/source/sorafs/postmortem_template.md` para conectar P1/P2 y conectar dispositivos. Haga clic aquí para obtener más información.