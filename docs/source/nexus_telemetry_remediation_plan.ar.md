---
lang: ar
direction: rtl
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_telemetry_remediation_plan.md -->

% خطة معالجة التلِمتري في Nexus (المرحلة B2)

# نظرة عامة

يتطلب بند خارطة الطريق **B2 - telemetry gap ownership** خطة منشورة تربط كل فجوة تلِمتري معلقة في
Nexus مع اشارة، وحاجز تنبيه، ومالك، وموعد نهائي، واثر تحقق قبل بدء نوافذ تدقيق الربع الاول 2026.
تجمع هذه الوثيقة تلك المصفوفة حتى يتمكن release engineering وعمليات التلِمتري ومالكو SDK من تأكيد
التغطية قبل تدريبات routed-trace و `TRACE-TELEMETRY-BRIDGE`.

# مصفوفة الفجوات

| معرف الفجوة | الاشارة وحاجز التنبيه | المالك / التصعيد | الاستحقاق (UTC) | الادلة والتحقق |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | مدرج `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`** يعمل عندما تكون `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` لمدة 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (الاشارة) + `@telemetry-ops` (التنبيه) - تصعيد عبر مناوبة Nexus routed-trace. | 2026-02-23 | اختبارات التنبيه في `dashboards/alerts/tests/soranet_lane_rules.test.yml` والتقاط تدريب `TRACE-LANE-ROUTING` الذي يظهر التنبيه عند الاطلاق/التعافي، مع حفظ سحب Torii `/metrics` في `docs/source/nexus_transition_notes.md`. |
| `GAP-TELEM-002` | عداد `nexus_config_diff_total{knob,profile}` مع حاجز `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` يمنع عمليات النشر (`docs/source/telemetry.md`). | `@nexus-core` (التحزيم) -> `@telemetry-ops` (التنبيه) - يتم تنبيه duty officer في الحوكمة عند زيادة العداد بشكل غير متوقع. | 2026-02-26 | يتم حفظ مخرجات dry-run للحوكمة بجوار `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ وتحتوي قائمة الاصدار على لقطة استعلام Prometheus مع مقتطف سجل يثبت ان `StateTelemetry::record_nexus_config_diff` اصدر الفرق. |
| `GAP-TELEM-003` | حدث `TelemetryEvent::AuditOutcome` (المقياس `nexus.audit.outcome`) مع تنبيه **`NexusAuditOutcomeFailure`** عندما تستمر حالات الفشل او النتائج المفقودة لاكثر من 30 دقيقة (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (خط المعالجة) مع تصعيد الى `@sec-observability`. | 2026-02-27 | بوابة CI `scripts/telemetry/check_nexus_audit_outcome.py` تؤرشف حمولات NDJSON وتفشل عندما تفتقد نافذة TRACE حدث نجاح؛ يتم ارفاق لقطات التنبيه في تقرير routed-trace. |
| `GAP-TELEM-004` | مقياس `nexus_lane_configured_total` يتم مراقبته بحاجز `nexus_lane_configured_total != EXPECTED_LANE_COUNT` (موثق في `docs/source/telemetry.md`) ويغذي قائمة مناوبة SRE. | `@telemetry-ops` (المقياس/التصدير) مع تصعيد الى `@nexus-core` عند قيام العقد بالابلاغ عن احجام كتالوج غير متسقة. | 2026-02-28 | اختبار تلِمتري scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يثبت الارسال؛ يرفق المشغلون فرق Prometheus مع مقتطف سجل `StateTelemetry::set_nexus_catalogs` ضمن حزمة تدريب TRACE. |

# ميزانية التصدير وحدود OTLP

- **القرار (2026-02-11):** وضع حد لمصدري OTLP عند **5 MiB/min لكل عقدة** او
  **25,000 spans/min** ايهما اقل، مع حجم دفعة 256 span ووقت تصدير قدره 10 ثوان. التصدير فوق 80%
  من الحد يطلق تنبيه `NexusOtelExporterSaturated` في `dashboards/alerts/nexus_telemetry_rules.yml`
  ويصدر حدث `telemetry_export_budget_saturation` لسجلات التدقيق.
- **التنفيذ:** تقرأ قواعد Prometheus العدادات `iroha.telemetry.export.bytes_total` و
  `iroha.telemetry.export.spans_total`؛ ويعتمد ملف OTLP collector نفس الحدود كافتراضي، ولا يجوز
  رفعها في اعدادات العقد بدون اعفاء من الحوكمة.
- **الادلة:** يتم ارشفة متجهات اختبار التنبيه والحدود المعتمدة تحت
  `docs/source/nexus_transition_notes.md` بجانب اثار تدقيق routed-trace. ويعامل قبول B2 الان
  ميزانية التصدير على انها مغلقة.

# سير العمل التشغيلي

1. **فرز اسبوعي.** يقدم المالكون تحديثات التقدم في مكالمة جاهزية Nexus؛ ويتم تسجيل العوائق واثار
   اختبار التنبيه في `status.md`.
2. **تجارب تنبيه جافة.** يتم شحن كل قاعدة تنبيه مع ادخال في
   `dashboards/alerts/tests/*.test.yml` كي تنفذ CI الامر `promtool test rules` عند تغيير الحاجز.
3. **ادلة التدقيق.** خلال تدريبات `TRACE-LANE-ROUTING` و `TRACE-TELEMETRY-BRIDGE` تلتقط المناوبة
   نتائج استعلامات Prometheus، وسجل التنبيهات، ومخرجات السكربتات ذات الصلة
   (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py`
   لاشارات مترابطة) وتخزنها مع اثار routed-trace.
4. **التصعيد.** اذا اطلق اي حاجز خارج نافذة تدريب، يفتح الفريق المالك تذكرة حادث Nexus تشير الى
   هذه الخطة، متضمنة لقطة المقاييس وخطوات المعالجة قبل استئناف التدقيق.

مع نشر هذه المصفوفة والاشارة اليها من `roadmap.md` و `status.md`، يحقق بند خارطة الطريق **B2** الان
معايير القبول "المسؤولية، الموعد النهائي، التنبيه، التحقق".

</div>
