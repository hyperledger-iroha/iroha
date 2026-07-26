---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-telemetria-remediação
título: خطة معالجة تيليمترية Nexus (B2)
description: Use o `docs/source/nexus_telemetry_remediation_plan.md` para obter informações sobre o produto e o produto.
---

# نظرة عامة

عنصر خارطة الطريق **B2 - ملكية فجوات القياس** يتطلب خطة منشورة تربط كل فجوة قياس متبقية Em Nexus, você pode obter informações e informações sobre o produto no Q1 2026. تعكس هذه الصفحة `docs/source/nexus_telemetry_remediation_plan.md` para que você possa usar o SDK e o SDK Use o método routed-trace e `TRACE-TELEMETRY-BRIDGE`.

# مصفوفة الفجوات

| معرف الفجوة | الاشارة وحاجز التنبيه | المالك / التصعيد | Hora (UTC) | Artigos e produtos |
|----|-------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Use o `torii_lane_admission_latency_seconds{lane_id,endpoint}` para obter o valor **`SoranetLaneAdmissionLatencyDegraded`** para usar o `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` em 5 minutos. (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (referência) + `@telemetry-ops` (referência); O rastreamento roteado de chamada on-call é baseado em Nexus. | 23/02/2026 | A solução `dashboards/alerts/tests/soranet_lane_rules.test.yml` pode ser usada para substituir `TRACE-LANE-ROUTING`. Raspe bem / raspe o Torii `/metrics` em [Nexus notas de transição] (./nexus-transition-notes). |
| `GAP-TELEM-002` | O `nexus_config_diff_total{knob,profile}` é o mesmo que o `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` é um problema (`docs/source/telemetry.md`). | `@nexus-core` (referência) -> `@telemetry-ops` (referência); Não se preocupe, não há problema em fazer isso. | 26/02/2026 | مخرجات dry-run للحوكمة محفوظة بجوار `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; Verifique se o Prometheus é um dispositivo que pode ser usado para substituir o `StateTelemetry::record_nexus_config_diff`. |
| `GAP-TELEM-003` | O `TelemetryEvent::AuditOutcome` (`nexus.audit.outcome`) pode ser usado **`NexusAuditOutcomeFailure`** O número de telefone é de 30 dias (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) é baseado em `@sec-observability`. | 27/02/2026 | Use CI `scripts/telemetry/check_nexus_audit_outcome.py` para carregar payloads NDJSON e usar o TRACE no caminho certo. O recurso de rastreamento roteado. |
| `GAP-TELEM-004` | O medidor `nexus_lane_configured_total` é o medidor `nexus_lane_configured_total != EXPECTED_LANE_COUNT` que está localizado no SRE. | `@telemetry-ops` (manômetro/exportação) O `@nexus-core` pode ser usado para medir o tamanho do produto. | 28/02/2026 | Use o código `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` para obter mais informações A configuração Prometheus + a configuração `StateTelemetry::set_nexus_catalogs` é definida como TRACE. |

# سير العمل التشغيلي

1. **فرز اسبوعي.** يبلغ المالكون عن التقدم في مكالمة جاهزية Nexus; Você pode usar o software de reparo em `status.md`.
2. **تجارب التنبيه.** تشحن كل قاعدة تنبيه مع مدخل `dashboards/alerts/tests/*.test.yml` بحيث ينفذ CI الامر `promtool test rules` Não, não.
3. **ادلة التدقيق.** خلال تمارين `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE` يلتقط المناوب نتائج استعلامات Prometheus وسجل التنبيهات ومخرجات السكربتات ذات الصلة (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` للاشارات المترابطة) ويخزنها Isso é feito com routed-trace.
4. **تصعيد.**** Nexus é uma ferramenta de segurança que pode ser usada para configurar o software e a solução de problemas. التدقيق.

مع نشر هذه المصفوفة - والاشارة اليها من `roadmap.md` e `status.md` - يفي عنصر roadmap **B2** الان بمعايير القبول "المسؤولية, الموعد النهائي, التنبيه, التحقق".