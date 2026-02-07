---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معالجة العلاقة بين القياس عن بعد
العنوان: خطة معالجة القياس عن بعد لـ Nexus (B2)
الوصف: شاهد `docs/source/nexus_telemetry_remediation_plan.md`، قم بتوثيق مصفوفة قواطع القياس عن بعد والتدفق التشغيلي.
---

# السيرة الذاتية العامة

يتطلب عنصر خريطة الطريق **B2 - ملكية قطع القياس عن بعد** خطة منشورة تتغلب على كل قطع القياس عن بعد المعلقة على Nexus بمراقبة، وحاجز حماية للتنبيه، ومسؤول، وتقرير محدود، وأداة للتحقق قبل بدء تشغيل فتحات السمع في الربع الأول من عام 2026. تشير هذه الصفحة إلى `docs/source/nexus_telemetry_remediation_plan.md` لإصدار الهندسة وعمليات القياس عن بعد وتأكيد مسؤولي SDK على التغطية السابقة للتتبع التوجيهي و`TRACE-TELEMETRY-BRIDGE`.

# ماتريز دي بريشاس

| معرف دي بريشا | تنبيه سينال ودرابزين | المسؤول / التصعيد | فيتشا (UTC) | الأدلة والتحقق |
|--------|----------------------------------------|-----|----------|-------------------------|
| `GAP-TELEM-001` | الرسم البياني `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`** الذي يتم حذفه عند `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` خلال 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سينال) + `@telemetry-ops` (تنبيه)؛ تصاعدي عبر التتبع الموجه عند الطلب Nexus. | 2026-02-23 | اختبار التنبيه في `dashboards/alerts/tests/soranet_lane_rules.test.yml` بالإضافة إلى التقاط الكتابة `TRACE-LANE-ROUTING` يعرض تنبيهات التعطيل/الاسترداد وكشط Torii `/metrics` في الأرشيف في انتقال [Nexus ملاحظات](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` مع حاجز الحماية `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` الذي يحجب (`docs/source/telemetry.md`). | `@nexus-core` (الأداة) -> `@telemetry-ops` (تنبيه)؛ تصبح الصفحة رسمية لحماية الإدارة عندما يتم زيادة حجم الحساب بشكل غير مكتمل. | 2026-02-26 | مخلفات التشغيل الجاف للمزارع بجوار `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ تتضمن قائمة الإصدار المرجعية التقاط استشارة Prometheus بالإضافة إلى استخراج السجلات التي تختبر `StateTelemetry::record_nexus_config_diff` لإصدار الفرق. |
| `GAP-TELEM-003` | الحدث `TelemetryEvent::AuditOutcome` (قياس `nexus.audit.outcome`) مع تنبيه **`NexusAuditOutcomeFailure`** عند حدوث فشل أو استمرار النتائج الخاطئة لمدة تزيد عن 30 دقيقة (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (خط الأنابيب) مع التصعيد إلى `@sec-observability`. | 2026-02-27 | يقوم جهاز الكمبيوتر CI `scripts/telemetry/check_nexus_audit_outcome.py` بأرشفة حمولات NDJSON ويسقط عند فتح نافذة TRACE لحدث الخروج؛ التقاط التنبيهات الإضافية لتقرير التتبع الموجه. |
| `GAP-TELEM-004` | مقياس `nexus_lane_configured_total` مع حاجز الحماية `nexus_lane_configured_total != EXPECTED_LANE_COUNT` الذي يزود قائمة التحقق عند الطلب من SRE. | `@telemetry-ops` (قياس/تصدير) مع تصعيد إلى `@nexus-core` عندما تبلغ العقد عن حجم الكتالوج غير المتناسق. | 2026-02-28 | اختبار القياس عن بعد لجدولة `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يعرض الانبعاثات؛ المشغلون المساعدون Prometheus مختلفون + مستخرج السجل `StateTelemetry::set_nexus_catalogs` في حزمة TRACE. |

# عملية التشغيل

1. **سلسلة الفرز.** أبلغ المالكون عن التقدم في اتصال الاستعداد Nexus؛ يتم تسجيل أدوات الحظر واختبارات التنبيه في `status.md`.
2. **رسائل التنبيه.** يتم إدخال كل نظام تنبيه جنبًا إلى جنب مع مدخل في `dashboards/alerts/tests/*.test.yml` حتى يتمكن CI من تنفيذ `promtool test rules` عند تغيير الدرابزين.
3. **أدلة الاستماع.** خلال المحاولات `TRACE-LANE-ROUTING` و`TRACE-TELEMETRY-BRIDGE`، تم التقاط نتائج الاستشارة عند الطلب Prometheus، وسجل التنبيهات، ونتائج البرامج النصية ذات الصلة (`scripts/telemetry/check_nexus_audit_outcome.py`، `scripts/telemetry/check_redaction_status.py` للمجموعات المرتبطة) والتخزين مع تتبع القطع الأثرية.
4. **التصعيد.** إذا انفصلت بعض الدرابزين عن نافذة مفتوحة، فإن الجهاز المسؤول عن تذكرة الحادث Nexus التي تشير إلى هذه الخطة، بما في ذلك لقطة المقياس وإجراءات التخفيف قبل إعادة القاعات.

مع نشر هذه المصفوفة - والمرجعية من `roadmap.md` و`status.md` - عنصر خريطة الطريق **B2** الآن يكمل معايير القبول "المسؤولية، الحد الأقصى، التنبيه، التحقق".