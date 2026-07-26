---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معالجة العلاقة بين القياس عن بعد
العنوان: خطة تعزيز اختبار قياس المسافة Nexus (B2)
الوصف: Zercalo `docs/source/nexus_telemetry_remediation_plan.md`، مادة موثقة لاختبار القياس عن بعد وعملية التشغيل.
---

# ملاحظة

خريطة طريق منقطة **B2 - زيادة مشكلات القياس عن بعد** يجب نشر خطة عامة يتم إجراؤها عند انتهاء المشكلة أجهزة القياس عن بعد Nexus للإشارة والحماية من خلال التحقق من السرعة والسرعة والتأخير والتصنيع حتى النهاية تدقيق الربع الأول من عام 2026. هذا الجزء يعبر عن `docs/source/nexus_telemetry_remediation_plan.md`، لهندسة الإصدار وعمليات القياس عن بعد ووحدات SDK التي يمكنها التحقق من الإنشاء قبل ذلك التكرارات الموجهة-التتبع و`TRACE-TELEMETRY-BRIDGE`.

# ماتريسا بروبيلوف

| معرف الفجوة | مراقبة الإشارات والحماية | Владелец / التصعيد | Срок (UTC) | المصادقة والتحقق |
|--------|----------------------------------------|-----|----------|-------------------------|
| `GAP-TELEM-001` | تم تسجيل `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`**، ويمكن الاتصال به عند `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` في غضون 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (إشارة) + `@telemetry-ops` (تنبيه)؛ الترقية من خلال التتبع الموجه عند الطلب Nexus. | 2026-02-23 | اختبارات التنبيه في `dashboards/alerts/tests/soranet_lane_rules.test.yml` بالإضافة إلى كتابة التكرارات `TRACE-LANE-ROUTING` مع التنبيه والترقية واستخراج الأرشيف Torii `/metrics` в [Nexus ملاحظات الانتقال](./nexus-transition-notes). |
| `GAP-TELEM-002` | Счетчик `nexus_config_diff_total{knob,profile}` مع الدرابزين `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`، блокирующим деплой (`docs/source/telemetry.md`). | `@nexus-core` (الأجهزة) -> `@telemetry-ops` (تنبيه)؛ يتم تنفيذه دائمًا من خلال نظام الحوكمة الجديد. | 2026-02-26 | يتم تسجيل حوكمة التشغيل الجاف بواسطة `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ تتضمن قائمة الاختيار لقطة شاشة Prometheus وشعارًا مفتوحًا يؤكد أن `StateTelemetry::record_nexus_config_diff` يختلف. |
| `GAP-TELEM-003` | احصل على `TelemetryEvent::AuditOutcome` (المقياس `nexus.audit.outcome`) مع تنبيه **`NexusAuditOutcomeFailure`** عند تسجيل الدخول أو النتائج النهائية أكثر من 30 دقيقة (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (خط الأنابيب) مع التصعيد إلى `@sec-observability`. | 2026-02-27 | يعمل CI-get `scripts/telemetry/check_nexus_audit_outcome.py` على أرشفة حمولات NDJSON وإرسالها، عندما لا يحقق TRACE النجاح؛ يتم عرض تنبيهات الشاشة من خلال التتبع الموجه. |
| `GAP-TELEM-004` | مقياس `nexus_lane_configured_total` مع الدرابزين `nexus_lane_configured_total != EXPECTED_LANE_COUNT`، الذي يتم دفعه عند الطلب SRE. | `@telemetry-ops` (قياس/تصدير) مع التدرج في `@nexus-core`، عندما تنضم إلى كتالوج النطاقات غير المتكامل. | 2026-02-28 | اختبار التخطيط عن بعد `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يستجيب للبعثة; تتضمن المشغلات فرق Prometheus + تسجيل الدخول `StateTelemetry::set_nexus_catalogs` في تكرار الحزمة TRACE. |

# عملية تشغيلية

1. **الفرز الجيني.** يتابع الطلاب التقدم المحرز في رسالة الاستعداد Nexus؛ يتم تثبيت تنبيهات الحظر والعناصر الاصطناعية في `status.md`.
2. **تنبيهات التشغيل الجاف.** عند إرسال التنبيه الصحيح إلى العلامة `dashboards/alerts/tests/*.test.yml`، يتم كتابة CI إلى `promtool test rules` إزالة الدرابزين.
3. **المصادقة على التدقيق.** خلال التكرار `TRACE-LANE-ROUTING` و`TRACE-TELEMETRY-BRIDGE`، يتم التحقق من النتائج يوميًا Prometheus وتاريخ التنبيهات والنصوص البرمجية ذات الصلة (`scripts/telemetry/check_nexus_audit_outcome.py` و`scripts/telemetry/check_redaction_status.py` للإشارات المرتبطة) و قم بتخزينها الآن باستخدام آثار التتبع الموجهة.
4. **التصعيد.** إذا تم تركيب الدرابزين بشكل متكرر مرة أخرى، يتم فتح لوحة التحكم Nexus، ссылаясь на هذه الخطة، وتعرض مقاييس اللقطة والأشياء التي يمكن التقاطها قبل عمليات التدقيق الاختيارية.

مع المصفوفة العامة - والخيارات من `roadmap.md` و`status.md` - خريطة الطريق الدقيقة **B2** تتجه نحو معايير المعايير "الإجابة، التنبيه، التنبيه، التحقق".