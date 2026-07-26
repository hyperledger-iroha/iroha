---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معالجة العلاقة بين القياس عن بعد
العنوان: خطة المعالجة التليمترية Nexus (B2)
الوصف: نسخة المقارنة لـ `docs/source/nexus_telemetry_remediation_plan.md` ت توثيق مصفوفة فجوات القياس ومسار العمل التشغيلي.
---

# نظرة عامة

تصميم خارطة الطريق **B2 - ملكية فجوات القياس** تتطلب خطة منشورة متميزة لكل ما هو متميز في القياس المتبقي في Nexus باشارة وحاجز تنبيه ومالك وموعد نهائي واثر تحقق قبل بدء نافذة تدقيق Q1 2026. تفضل هذه الصفحة `docs/source/nexus_telemetry_remediation_plan.md` لكي يتأهل فريق هندسة الاصدار للقياس ومالكو SDK من تاكيد يتبرع قبل خطوات التتبع الموجه و `TRACE-TELEMETRY-BRIDGE`.

#مصفوفة الفجوات

| معرف الاستمرارية | الاشارة وحاجز التنبيه | المالك / التصعيد | الاستحقاق (UTC) | الدليل والتحقق |
|--------|----------------------------------------|-----|----------|-------------------------|
| `GAP-TELEM-001` | مخططي `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`** يعمل عندما `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` لمدة 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (اشارة) + `@telemetry-ops` (تنبيه)؛ التصعيد عبر on-call router-trace الخاص بـ Nexus. | 2026-02-23 | التنبيه تحت `dashboards/alerts/tests/soranet_lane_rules.test.yml` مع لقطة تمرين `TRACE-LANE-ROUTING` التي تنبه التنبيه عند الاطلاق/التعافي وارشفة سكراب لـ Torii `/metrics` في [Nexus Transition Notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | عداد `nexus_config_diff_total{knob,profile}` مع حاجز `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` يمنع عمليات النشر (`docs/source/telemetry.md`). | `@nexus-core` (ادوات القياس) -> `@telemetry-ops` (تنبيه)؛ يتم التنبيه المسؤول عن غير المناوب عند زيادة العداد غير المتوقع. | 2026-02-26 | مخرجات التشغيل الجاف للإنترنت المجاورة `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ تشمل قائمة الاصدار استعلام Prometheus مع مقتطف سجل تثبيت ان اللقطة `StateTelemetry::record_nexus_config_diff` صدر الفارق. |
| `GAP-TELEM-003` | حدث `TelemetryEvent::AuditOutcome` (المترية `nexus.audit.outcome`) مع تنبيه **`NexusAuditOutcomeFailure`** عندما تستمر حالات العمل او البحث عن طلب لاكثر من 30 دقيقة (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (خط الأنابيب) مع تصعيد الى `@sec-observability`. | 2026-02-27 | بوابة CI `scripts/telemetry/check_nexus_audit_outcome.py` ترشف الحمولات NDJSON وفشلت عندما تخلت عن نافذة TRACE من حدث نجاح؛ لقطات التنبيه المرفقة بتقرير التتبع الموجه. |
| `GAP-TELEM-004` | مقياس `nexus_lane_configured_total` مع IP `nexus_lane_configured_total != EXPECTED_LANE_COUNT` يغذي بطاقة المصادقة الخاصة بـ SRE المناوب. | `@telemetry-ops` (قياس/تصدير) مع تصعيد إلى `@nexus-core` عندما أعقد عن احجام كتالوج غير متماسكة. | 2026-02-28 | اختبار تيليمترية المجدول `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` إثبات الاصدار؛ يرفق تشغيلون فرق Prometheus + مقتطف سجل `StateTelemetry::set_nexus_catalogs` تمرين TRACE. |

#سير العمل التشغيلي

1. ** فرز اسبوعي.** القسمون عن التقدم في الفعالية الجاهزية Nexus؛ يتم تسجيل العوائق واختبار الإشعارات البديلة في `status.md`.
2. **تجارب التنبيه.** شحن كل قاعدة تنبيه مع مدخل `dashboards/alerts/tests/*.test.yml` بحيث ينفذ CI الأمر `promtool test rules` عند تغير GPS.
3. **معادلة التدقيق.** خلال عمليتين `TRACE-LANE-ROUTING` و `TRACE-TELEMETRY-BRIDGE` يلتقط المناوب نتائج علامات Prometheus وتظهر التنبيهات ومخرجات السكربتات ذات الصلة (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` للاشارات المترابطة) ويخزنها مع اثباتات router-trace.
4. ** تصعيد.** اذا تم اي كونترول خارج نافذة التمرين، الفريق يغلق باستثناء تذكرة حادث Nexus تشير الى هذه البناء، مع تضمين لقطة المترية و خطوات غير قبل قبل وعمليات الحظر.

مع نشر هذه المصفوفة - والاشارة إلى `roadmap.md` و `status.md` - يفي عنصر خريطة الطريق **B2** الان بمعايير قبول "المسؤولية، النتيجة النهائية، التنبيه، التحقق".