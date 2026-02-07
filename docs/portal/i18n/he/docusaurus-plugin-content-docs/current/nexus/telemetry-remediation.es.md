---
lang: he
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-telemetry-remediation
כותרת: Plan de remediacion de telemetria de Nexus (B2)
תיאור: Espejo de `docs/source/nexus_telemetry_remediation_plan.md`, documenta la matriz de brechas de telemetria y el flujo operativo.
---

# רזומה כללי

El item del roadmap **B2 - Ownership de brechas de telemetria** דורש un plan publicado que vincule cada brecha de telemetria pendiente de Nexus con una senal, un guard de alerta, un responsable, una fecha limite y un artefacto de la verificacion de comience comiencen Q1 2026. Esta page refleja `docs/source/nexus_telemetry_remediation_plan.md` para que release release, telemetry operations y los responsables de SDK confirmen la cobertura antes de los ensayos routed-trace y `TRACE-TELEMETRY-BRIDGE`.

# Matriz de brechas

| ID de brecha | סנאל ומעקה בטיחות דה אלרטה | אחראי / escalamiento | Fecha (UTC) | Evidencia y Verificacion |
|--------|------------------------|------------------------|-----------|------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` עם התראה **`SoranetLaneAdmissionLatencyDegraded`** que dispara cuando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` למשך 5 דקות (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (סנאל) + `@telemetry-ops` (תראה); escalar באמצעות on-call de routed-trace de Nexus. | 23-02-2026 | Pruebas de alerta en `dashboards/alerts/tests/soranet_lane_rules.test.yml` mas la captura del ensayo `TRACE-LANE-ROUTING` mostrando alerta disparada/recuperada y el scrape de Torii `/metrics` archivado en [I transition הערות](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` עם מעקה בטיחות `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` que bloquea despliegues (`docs/source/telemetry.md`). | `@nexus-core` (מכשירים) -> `@telemetry-ops` (תראה); se page al oficial de guardia de gobernanza cuando el contador incrementa de forma inesperada. | 2026-02-26 | Salidas de dry-run de gobernanza almacenadas junto a `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; la checklist de release incluye la captura de la consulta de Prometheus mas el extracto de logs que prueba que `StateTelemetry::record_nexus_config_diff` emitio el diff. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) עם התראה **`NexusAuditOutcomeFailure`** cuando fallas o resultados faltantes persistent por >30 minuteos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (צינור) עם escalamiento a `@sec-observability`. | 2026-02-27 | La compuerta de CI `scripts/telemetry/check_nexus_audit_outcome.py` עומסי ארכיון NDJSON y falla cuando una ventana TRACE carece de un evento de exito; מעקב אחר התראה לדיווחים מנותבים. |
| `GAP-TELEM-004` | מד `nexus_lane_configured_total` עם מעקה בטיחות `nexus_lane_configured_total != EXPECTED_LANE_COUNT` que alimenta la checklist on-call de SRE. | `@telemetry-ops` (מד/ייצוא) עם escalamiento a `@nexus-core` עם דיווחים לא עקביים בקטלוג. | 2026-02-28 | La prueba de telemetria del scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` demuestra la emision; los operations adjuntan Prometheus diff + extracto de log de `StateTelemetry::set_nexus_catalogs` al paquete del ensayo TRACE. |

# Flujo operativo1. **טריאז' סמנאלי.** Los owners reportan progreso en la llamada de readiness de Nexus; חוסמי y artefactos de pruebas de alerta se registran en `status.md`.
2. **Ensayos de alertas.** Cada regla de alerta se entrega junto con una entrada en `dashboards/alerts/tests/*.test.yml` para que CI ejecute `promtool test rules` cuando cambie el מעקה בטיחות.
3. **Evidencia de auditoria.** Durante los ensayos `TRACE-LANE-ROUTING` y `TRACE-TELEMETRY-BRIDGE` el on-call captura los resultados de consultas de Prometheus, el historical de alertas y las salidas008 de 5NI08 relevantes `scripts/telemetry/check_redaction_status.py` para senales correlacionadas) y los almacena con los artefactos routed-trace.
4. **Escalamiento.** מעקה הבטיחות של אלגון הוא דיספארה דה אונה ונטנה אנסיאדה, אל equipo responsable abre un ticket de incidente Nexus que referencia este plan, incluyendo el snapshot de la metrica y los pasos de mitorianudar.

Con esta matriz publicada - y referenciada desde `roadmap.md` y `status.md` - el item de roadmap **B2** ahora cumple los criterias de acceptacion "responsabilidad, fecha limite, alerta, verificacion".