---
lang: es
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: reparación-telemetría-nexus
título: خطة معالجة تيليمترية Nexus (B2)
descripción: نسخة مطابقة لـ `docs/source/nexus_telemetry_remediation_plan.md` توثق مصفوفة فجوات القياس ومسار العمل التشغيلي.
---

# نظرة عامة

عنصر خارطة الطريق **B2 - ملكية فجوات القياس** يتطلب خطة منشورة تربط كل فجوة قياس متبقية في Nexus باشارة y تنبيه ومالك وموعد نهائي واثر تحقق قبل بدء نوافذ تدقيق Q1 2026. هذه الصفحة `docs/source/nexus_telemetry_remediation_plan.md` لكي يتمكن فريق هندسة الاصدار وعمليات القياس AND مالكو SDK من تاكيد التغطية قبل تمارين seguimiento enrutado y `TRACE-TELEMETRY-BRIDGE`.

# مصفوفة الفجوات| معرف الفجوة | الاشارة وحاجز التنبيه | المالك / التصعيد | الاستحقاق (UTC) | الدليل والتحقق |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | La unidad `torii_lane_admission_latency_seconds{lane_id,endpoint}` se encuentra en **`SoranetLaneAdmissionLatencyDegraded`** y la unidad `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` dura 5 días (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (اشارة) + `@telemetry-ops` (تنبيه)؛ El rastreo enrutado de guardia se encuentra en Nexus. | 2026-02-23 | Utilice el `dashboards/alerts/tests/soranet_lane_rules.test.yml` para limpiar el `TRACE-LANE-ROUTING` y el raspador. Torii `/metrics` en [notas de transición Nexus](./nexus-transition-notes). |
| `GAP-TELEM-002` | عداد `nexus_config_diff_total{knob,profile}` مع حاجز `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` يمنع عمليات النشر (`docs/source/telemetry.md`). | `@nexus-core` (ادوات القياس) -> `@telemetry-ops` (تنبيه)؛ يتم تنبيه مسؤول الحوكمة المناوب عند زيادة العداد بشكل غير متوقع. | 2026-02-26 | Funcionamiento en seco del dispositivo de funcionamiento `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ Utilice el conector Prometheus para conectar el conector `StateTelemetry::record_nexus_config_diff`. || `GAP-TELEM-003` | Inserte `TelemetryEvent::AuditOutcome` (`nexus.audit.outcome`) en **`NexusAuditOutcomeFailure`** para obtener más información y más información. Límite de 30 días (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (tubería) مع تصعيد الى `@sec-observability`. | 2026-02-27 | بوابة CI `scripts/telemetry/check_nexus_audit_outcome.py` Cargas útiles de NDJSON y conexión de TRACE con conexión a Internet لقطات التنبيه مرفقة بتقرير ruta-traza. |
| `GAP-TELEM-004` | Calibre `nexus_lane_configured_total` مع حاجز `nexus_lane_configured_total != EXPECTED_LANE_COUNT` يغذي قائمة التحقق الخاصة بـ SRE المناوب. | `@telemetry-ops` (medidor/exportación) مع تصعيد الى `@nexus-core` عندما تبلغ العقد عن احجام كتالوج غير متناسقة. | 2026-02-28 | Adaptador de corriente `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` Utilice Prometheus + Inserte `StateTelemetry::set_nexus_catalogs` en TRACE. |

# سير العمل التشغيلي1. **فرز اسبوعي.** يبلغ المالكون عن التقدم في مكالمة جاهزية Nexus؛ Esta es la configuración de la unidad `status.md`.
2. **تجارب التنبيه.** تشحن كل قاعدة تنبيه مع مدخل `dashboards/alerts/tests/*.test.yml` بحيث ينفذ CI الامر `promtool test rules` عند تغير الحاجز.
3. **ادلة التدقيق.** خلال تمارين `TRACE-LANE-ROUTING` and `TRACE-TELEMETRY-BRIDGE` لتقط المناوب نتائج استعلامات Prometheus and Enlaces de seguimiento y seguimiento (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py`) y seguimiento enrutado.
4. **تصعيد.** اذا تم اطلاق اي حاجز خارج نافذة تمرين، يفتح الفريق المالك تذكرة حادث Nexus تشير الى هذه الخطة، مع تضمين لقطة المترية وخطوات التخفيف قبل استئناف عمليات التدقيق.

مع نشر هذه المصفوفة - والاشارة اليها من `roadmap.md` and `status.md` - يفي عنصر roadmap **B2** الان بمعايير القبول "المسؤولية، الموعد النهائي، التنبيه، التحقق".