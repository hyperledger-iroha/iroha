---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-telemetry-remediation
title: خطة معالجة تيليمترية Nexus (B2)
description: نسخة مطابقة لـ `docs/source/nexus_telemetry_remediation_plan.md` توثق مصفوفة فجوات القياس ومسار العمل التشغيلي.
---

# نظرة عامة

عنصر خارطة الطريق **B2 - ملكية فجوات القياس** يتطلب خطة منشورة تربط كل فجوة قياس متبقية في Nexus باشارة وحاجز تنبيه ومالك وموعد نهائي واثر تحقق قبل بدء نوافذ تدقيق Q1 2026. تعكس هذه الصفحة `docs/source/nexus_telemetry_remediation_plan.md` لكي يتمكن فريق هندسة الاصدار وعمليات القياس ومالكو SDK من تاكيد التغطية قبل تمارين routed-trace و `TRACE-TELEMETRY-BRIDGE`.

# مصفوفة الفجوات

| معرف الفجوة | الاشارة وحاجز التنبيه | المالك / التصعيد | الاستحقاق (UTC) | الدليل والتحقق |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | مخطط تكراري `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`** يعمل عندما `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` لمدة 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (اشارة) + `@telemetry-ops` (تنبيه)؛ التصعيد عبر on-call routed-trace الخاص بـ Nexus. | 2026-02-23 | اختبارات التنبيه تحت `dashboards/alerts/tests/soranet_lane_rules.test.yml` مع لقطة تمرين `TRACE-LANE-ROUTING` التي تظهر التنبيه عند الاطلاق/التعافي وارشفة scrape لـ Torii `/metrics` في [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | عداد `nexus_config_diff_total{knob,profile}` مع حاجز `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` يمنع عمليات النشر (`docs/source/telemetry.md`). | `@nexus-core` (ادوات القياس) -> `@telemetry-ops` (تنبيه)؛ يتم تنبيه مسؤول الحوكمة المناوب عند زيادة العداد بشكل غير متوقع. | 2026-02-26 | مخرجات dry-run للحوكمة محفوظة بجوار `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ تشمل قائمة الاصدار لقطة استعلام Prometheus مع مقتطف سجل يثبت ان `StateTelemetry::record_nexus_config_diff` اصدر الفارق. |
| `GAP-TELEM-003` | حدث `TelemetryEvent::AuditOutcome` (المترية `nexus.audit.outcome`) مع تنبيه **`NexusAuditOutcomeFailure`** عندما تستمر حالات الفشل او النتائج المفقودة لاكثر من 30 دقيقة (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) مع تصعيد الى `@sec-observability`. | 2026-02-27 | بوابة CI `scripts/telemetry/check_nexus_audit_outcome.py` تؤرشف payloads NDJSON وتفشل عندما تخلو نافذة TRACE من حدث نجاح؛ لقطات التنبيه مرفقة بتقرير routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` مع حاجز `nexus_lane_configured_total != EXPECTED_LANE_COUNT` يغذي قائمة التحقق الخاصة بـ SRE المناوب. | `@telemetry-ops` (gauge/export) مع تصعيد الى `@nexus-core` عندما تبلغ العقد عن احجام كتالوج غير متناسقة. | 2026-02-28 | اختبار تيليمترية المجدول `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يثبت الاصدار؛ يرفق المشغلون فرق Prometheus + مقتطف سجل `StateTelemetry::set_nexus_catalogs` بحزمة تمرين TRACE. |

# سير العمل التشغيلي

1. **فرز اسبوعي.** يبلغ المالكون عن التقدم في مكالمة جاهزية Nexus؛ يتم تسجيل العوائق وادلة اختبار التنبيه في `status.md`.
2. **تجارب التنبيه.** تشحن كل قاعدة تنبيه مع مدخل `dashboards/alerts/tests/*.test.yml` بحيث ينفذ CI الامر `promtool test rules` عند تغير الحاجز.
3. **ادلة التدقيق.** خلال تمارين `TRACE-LANE-ROUTING` و `TRACE-TELEMETRY-BRIDGE` يلتقط المناوب نتائج استعلامات Prometheus وسجل التنبيهات ومخرجات السكربتات ذات الصلة (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` للاشارات المترابطة) ويخزنها مع اثباتات routed-trace.
4. **تصعيد.** اذا تم اطلاق اي حاجز خارج نافذة تمرين، يفتح الفريق المالك تذكرة حادث Nexus تشير الى هذه الخطة، مع تضمين لقطة المترية وخطوات التخفيف قبل استئناف عمليات التدقيق.

مع نشر هذه المصفوفة - والاشارة اليها من `roadmap.md` و `status.md` - يفي عنصر roadmap **B2** الان بمعايير القبول "المسؤولية، الموعد النهائي، التنبيه، التحقق".
