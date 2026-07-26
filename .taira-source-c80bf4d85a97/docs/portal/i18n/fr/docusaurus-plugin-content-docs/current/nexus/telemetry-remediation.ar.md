---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : lien-télémétrie-remédiation
titre : خطة معالجة تيليمترية Nexus (B2)
description: نسخة مطابقة لـ `docs/source/nexus_telemetry_remediation_plan.md` توثق مصفوفة فجوات القياس ومسار العمل التشغيلي.
---

# نظرة عامة

عنصر خارطة الطريق **B2 - ملكية فجوات القياس** يتطلب خطة منشورة تربط كل فجوة قياس متبقية في Nexus باشارة وحاجز نوافذ تدقيق Q1 2026. Le `docs/source/nexus_telemetry_remediation_plan.md` est un outil de développement durable pour le SDK et le SDK. Utilisé routed-trace et `TRACE-TELEMETRY-BRIDGE`.

# مصفوفة الفجوات| معرف الفجوة | الاشارة وحاجز التنبيه | المالك / التصعيد | الاستحقاق (UTC) | الدليل والتحقق |
|--------|-------------------------|----------|---------------|-------------------------|
| `GAP-TELEM-001` | مخطط تكراري `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`** يعمل عندما `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` لمدة 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (اشارة) + `@telemetry-ops` (تنبيه)؛ La trace acheminée sur appel est Nexus. | 2026-02-23 | اختبارات التنبيه تحت `dashboards/alerts/tests/soranet_lane_rules.test.yml` مع لقطة تمرين `TRACE-LANE-ROUTING` التي تظهر التنبيه عند الاطلاق/التعافي وارشفة gratter لـ Torii `/metrics` في [Notes de transition Nexus](./nexus-transition-notes). |
| `GAP-TELEM-002` | عداد `nexus_config_diff_total{knob,profile}` مع حاجز `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` يمنع عمليات النشر (`docs/source/telemetry.md`). | `@nexus-core` (ادوات القياس) -> `@telemetry-ops` (تنبيه)؛ يتم تنبيه مسؤول الحوكمة المناوب عند زيادة العداد بشكل غير متوقع. | 2026-02-26 | Fonctionnement à sec pour `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` تشمل قائمة الاصدار لقطة استعلام Prometheus مع مقتطف سجل يثبت ان `StateTelemetry::record_nexus_config_diff` اصدر الفارق. || `GAP-TELEM-003` | حدث `TelemetryEvent::AuditOutcome` (المترية `nexus.audit.outcome`) avec **`NexusAuditOutcomeFailure`** عندما تستمر حالات الفشل او النتائج المفقودة Pour 30 dollars (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) est également compatible avec `@sec-observability`. | 2026-02-27 | Utilisez CI `scripts/telemetry/check_nexus_audit_outcome.py` pour charger les charges utiles NDJSON et utilisez TRACE pour vos besoins. Il s'agit d'une trace acheminée. |
| `GAP-TELEM-004` | La jauge `nexus_lane_configured_total` est compatible avec la jauge `nexus_lane_configured_total != EXPECTED_LANE_COUNT`. | `@telemetry-ops` (jauge/export) est utilisé pour `@nexus-core`. | 2026-02-28 | اختبار تيليمترية المجدول `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يثبت الاصدار؛ يرفق المشغلون فرق Prometheus + مقتطف سجل `StateTelemetry::set_nexus_catalogs` pour TRACE. |

# سير العمل التشغيلي1. **فرز اسبوعي.** يبلغ المالكون عن التقدم في مكالمة جاهزية Nexus؛ يتم تسجيل العوائق وادلة اختبار التنبيه في `status.md`.
2. **تجارب التنبيه.** تشحن كل قاعدة تنبيه مع مدخل `dashboards/alerts/tests/*.test.yml` byحيث ينفذ CI الامر `promtool test rules` عند تغير الحاجز.
3. **ادلة التدقيق.** خلال تمارين `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE` يلتقط المناوب نتائج استعلامات Prometheus وسجل التنبيهات ومخرجات السكربتات ذات الصلة (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` للاشارات المترابطة) et مع اثباتات trace acheminée.
4. **تصعيد.** اذا تم اطلاق اي حاجز خارج نافذة تمرين، يفتح الفريق المالك تذكرة حادث Nexus تشير الى هذه الخطة، مع تضمين لقطة المترية وخطوات التخفيف قبل استئناف عمليات التدقيق.

مع نشر هذه المصفوفة - واشارة اليها من `roadmap.md` et `status.md` - يفي عنصر roadmap **B2** الان بمعايير القبول "المسؤولية، الموعد النهائي، التنبيه، التحقق".