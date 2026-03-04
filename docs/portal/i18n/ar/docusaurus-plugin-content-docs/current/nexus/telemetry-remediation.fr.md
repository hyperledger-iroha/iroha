---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معالجة العلاقة بين القياس عن بعد
العنوان: خطة معالجة القياس عن بعد Nexus (B2)
الوصف: Miroir de `docs/source/nexus_telemetry_remediation_plan.md`، توثيق مصفوفة خرائط القياس عن بعد وعمليات التدفق.
---

# عرض المجموعة

عنصر خريطة الطريق **B2 - ملكية أدوات القياس عن بعد** يتطلب خطة نشر تعتمد على كل بطاقة قياس عن بعد تبقى من Nexus إشارة واحدة وحاجز حماية وتنبيه ومالك وتاريخ محدد وأداة للتحقق قبل ظهور نوافذ التدقيق في الربع الأول من عام 2026. هذه الصفحة تعكس `docs/source/nexus_telemetry_remediation_plan.md` يسمح بإصدار عمليات الهندسة والقياس عن بعد ومالكي SDK بشكل قوي لتأكيد الغطاء قبل التكرارات الموجهة للتتبع و`TRACE-TELEMETRY-BRIDGE`.

# ماتريس دي كارت

| معرف البطاقة | تنبيه الإشارة والدرابزين | بروبريتير / إسكاليد | الصدى (التوقيت العالمي المنسق) | Preuves والتحقق |
|--------|----------------------------------------|-----|----------|-------------------------|
| `GAP-TELEM-001` | الرسم البياني `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع التنبيه **`SoranetLaneAdmissionLatencyDegraded`** فاصل عندما `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` لمدة 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (إشارة) + `@telemetry-ops` (تنبيه)؛ تصعيد عبر l'on-call توجيه التتبع Nexus. | 2026-02-23 | اختبارات التنبيه Sous `dashboards/alerts/tests/soranet_lane_rules.test.yml` بالإضافة إلى التقاط التكرار `TRACE-LANE-ROUTING` لضبط التنبيه/إعادة الضبط والكشط Torii `/metrics` في الأرشيف في انتقال [Nexus ملاحظات](./nexus-transition-notes). |
| `GAP-TELEM-002` | الكمبيوتر `nexus_config_diff_total{knob,profile}` مع الدرابزين `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` يمنع عمليات النشر (`docs/source/telemetry.md`). | `@nexus-core` (الأجهزة) -> `@telemetry-ops` (تنبيه)؛ يتم تعيين ضابط الحوكمة عندما يقوم بحساب زيادة الطريقة غير المراقبة. | 2026-02-26 | طلعات مخزونات الإدارة الجافة في `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ تتضمن قائمة التحقق من التحرير التقاط الطلب Prometheus بالإضافة إلى مخرج السجلات الذي يثبت `StateTelemetry::record_nexus_config_diff` مما يؤدي إلى ظهور الفرق. |
| `GAP-TELEM-003` | الحدث `TelemetryEvent::AuditOutcome` (المقياس `nexus.audit.outcome`) مع التنبيه **`NexusAuditOutcomeFailure`** عند إجراء عمليات فحص أو نتائج متواصلة لمدة تزيد عن 30 دقيقة (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (خط الأنابيب) مع إسكاليد مقابل `@sec-observability`. | 2026-02-27 | بوابة CI `scripts/telemetry/check_nexus_audit_outcome.py` أرشيف الحمولات NDJSON والصدى عند نافذة TRACE لا تحتوي على حدث ناجح؛ يلتقط التنبيهات المشتركة في التتبع الموجه. |
| `GAP-TELEM-004` | يتم تشغيل المقياس `nexus_lane_configured_total` مع الدرابزين `nexus_lane_configured_total != EXPECTED_LANE_COUNT` بواسطة قائمة التحقق SRE عند الطلب. | `@telemetry-ops` (قياس/تصدير) مع التصعيد إلى `@nexus-core` عند الإشارة إلى تفاصيل تفاصيل الكتالوج غير المتماسكة. | 2026-02-28 | اختبار القياس عن بعد للجدولة `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يثبت الانبعاث؛ ينضم المشغلون إلى فرق Prometheus + إضافة السجل `StateTelemetry::set_nexus_catalogs` إلى حزمة التكرار TRACE. |

# عملية التدفق

1. **فرز البيانات.** أصحابها مرتبطون بالتقدم عند طلب الاستعداد Nexus; يتم إرسال كتل وعناصر اختبارات التنبيه إلى `status.md`.
2. **اختبارات التنبيه.** يتم فتح كل نظام تنبيه مع إدخال `dashboards/alerts/tests/*.test.yml` حتى يقوم CI بتنفيذ `promtool test rules` أثناء تطور حاجز الحماية.
3. **إجراءات التدقيق.** تعليق التكرارات `TRACE-LANE-ROUTING` و`TRACE-TELEMETRY-BRIDGE`، والتقاط نتائج الطلبات عند الاتصال Prometheus، وتاريخ التنبيهات وعمليات نصوص البرامج ذات الصلة (`scripts/telemetry/check_nexus_audit_outcome.py`، `scripts/telemetry/check_redaction_status.py` للإشارات المرتبطة) والمخزون مع التتبع الأثري.
4. **Escalade.** إذا كان الدرابزين ينزل عن نافذة متكررة، فإن المعدات المالكة تفتح تذكرة حادث Nexus بالرجوع إلى هذه الخطة، بما في ذلك لقطة المقياس وخطوات التخفيف قبل إعادة إجراء عمليات التدقيق.

مع هذه المصفوفة المنشورة - والمرجع من `roadmap.md` و`status.md` - عنصر خريطة الطريق **B2** يفي بمعايير القبول "المسؤولية والتحقق والتنبيه والتحقق".