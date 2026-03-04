---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معالجة العلاقة بين القياس عن بعد
العنوان: Plano de remediacao de telemetria do Nexus (B2)
الوصف: مثال على `docs/source/nexus_telemetry_remediation_plan.md`، توثيق مصفوفة ثغرات القياس عن بعد والتدفق التشغيلي.
---

# فيساو جيرال

عنصر خريطة الطريق **B2 - ملكية ثغرات القياس عن بعد** يتطلب خطة منشورة تتغلب على كل ثغرة في القياس عن بعد معلقة في Nexus في خط دفاعي، حاجز تنبيه، استجابة، إجراء، وأداة للتحقق قبل البدء في سجلات الاستماع في Q1 2026. هذه الصفحة مخصصة لـ `docs/source/nexus_telemetry_remediation_plan.md` لإصدار عمليات الهندسة والقياس عن بعد وتأكيد مالكي SDK على التغطية قبل إجراء التتبع الموجه و`TRACE-TELEMETRY-BRIDGE`.

# ماتريز دي الثغرات

| معرف دا ثغرة | تنبيه سينال ودرابزين | الاستجابة / التصعيد | برازو (UTC) | الأدلة والتحقق |
|--------|----------------------------------------|-----|----------|-------------------------|
| `GAP-TELEM-001` | الرسم البياني `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`** تجاهل عندما `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` لمدة 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سينال) + `@telemetry-ops` (تنبيه)؛ قم بالتصعيد عبر التتبع الموجه عند الطلب باستخدام Nexus. | 2026-02-23 | اختبارات التنبيه في `dashboards/alerts/tests/soranet_lane_rules.test.yml` أكثر من تسجيل `TRACE-LANE-ROUTING` معظم تنبيهات الحذف/الاسترداد والحذف Torii `/metrics` محفوظة في انتقال [Nexus ملاحظات](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` com الدرابزين `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` نشر bloqueando (`docs/source/telemetry.md`). | `@nexus-core` (الأداة) -> `@telemetry-ops` (تنبيه)؛ مسؤول الإدارة والصفحة عندما يتم زيادة المبلغ المحاسب على شكل غير مكتمل. | 2026-02-26 | إرشادات التشغيل الجاف للإدارة المُخزنة على طول `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ تتضمن قائمة التحقق من الإصدار الالتقاط الذي راجعته Prometheus بالإضافة إلى سجل الإدخال الذي يصدره `StateTelemetry::record_nexus_config_diff` أو اختلافًا. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (قياس `nexus.audit.outcome`) مع تنبيه **`NexusAuditOutcomeFailure`** عند حدوث خطأ أو نتيجة مستمرة لمدة تزيد عن 30 دقيقة (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (خط الأنابيب) مع التصعيد لـ `@sec-observability`. | 2026-02-27 | تفتح بوابة CI `scripts/telemetry/check_nexus_audit_outcome.py` حمولات NDJSON وتفشل عند TRACE في حدث النجاح؛ التقاط التنبيهات المرتبطة بتتبع التوجيه. |
| `GAP-TELEM-004` | مقياس `nexus_lane_configured_total` مع الدرابزين `nexus_lane_configured_total != EXPECTED_LANE_COUNT` يوفر قائمة مرجعية عند الطلب من SRE. | `@telemetry-ops` (قياس/تصدير) مع تصعيد لـ `@nexus-core` عندما نبلغ عن حجم الكتالوج غير المتسق. | 2026-02-28 | اختبار القياس عن بعد لجدولة `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يطابق الإرسال؛ المشغلون مختلفون عن Prometheus + سجل `StateTelemetry::set_nexus_catalogs` في حزمة TRACE. |

# فلوكسو التشغيلية

1. **Triagem semanal.** يُبلغ المالكون عن التقدم المحرز في استدعاء الاستعداد Nexus; حاصرات ومصنوعات تنبيه الخصية المسجلة في `status.md`.
2. **نصائح التنبيهات.** كل تنبيه جديد وإدخاله مع مدخل `dashboards/alerts/tests/*.test.yml` حتى يقوم CI بتنفيذ `promtool test rules` عند تحطيم الدرابزين.
3. **أدلة الاستماع.** خلال المهام `TRACE-LANE-ROUTING` و`TRACE-TELEMETRY-BRIDGE` أو التقاط نتائج الاستشارة عند الطلب Prometheus، وتاريخ التنبيهات والأقوال ذات الصلة بالنصوص البرمجية (`scripts/telemetry/check_nexus_audit_outcome.py`، `scripts/telemetry/check_redaction_status.py` لـ الارتباطات المرتبطة) كما يتم تخزينها مع التتبع الموجه للقطع الأثرية.
4. **التصعيد.** إذا كان هناك شيء ما يفصل حاجز الحماية عن أحد الجوانب المدروسة، فإن فريق الاستجابة لتذكرة الحادث Nexus يشير إلى هذه الخطة، بما في ذلك لقطة القياس وخطوات التخفيف قبل إعادة الاستماع إلى المستمعين.

باستخدام هذه المصفوفة المنشورة - والمرجعية إلى `roadmap.md` و`status.md` - عنصر خريطة الطريق **B2** يجب أن يلتزم بمعايير الاستخدام "المسؤولية، والصراحة، والتنبيه، والتحقق".