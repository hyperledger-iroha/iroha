---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/nexus/telemetry-remediation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3cbc87a6e698537cce738e692541e77c6814afaa87b968a00bbb4ceb588b06b2
source_last_modified: "2025-11-14T04:43:20.588432+00:00"
translation_last_reviewed: 2026-01-30
---

# Resumen general

El item del roadmap **B2 - ownership de brechas de telemetria** requiere un plan publicado que vincule cada brecha de telemetria pendiente de Nexus con una senal, un guardrail de alerta, un responsable, una fecha limite y un artefacto de verificacion antes de que comiencen las ventanas de auditoria de Q1 2026. Esta pagina refleja `docs/source/nexus_telemetry_remediation_plan.md` para que release engineering, telemetry ops y los responsables de SDK confirmen la cobertura antes de los ensayos routed-trace y `TRACE-TELEMETRY-BRIDGE`.

# Matriz de brechas

| ID de brecha | Senal y guardrail de alerta | Responsable / escalamiento | Fecha (UTC) | Evidencia y verificacion |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` con alerta **`SoranetLaneAdmissionLatencyDegraded`** que dispara cuando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` durante 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (senal) + `@telemetry-ops` (alerta); escalar via on-call de routed-trace de Nexus. | 2026-02-23 | Pruebas de alerta en `dashboards/alerts/tests/soranet_lane_rules.test.yml` mas la captura del ensayo `TRACE-LANE-ROUTING` mostrando alerta disparada/recuperada y el scrape de Torii `/metrics` archivado en [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` con guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` que bloquea despliegues (`docs/source/telemetry.md`). | `@nexus-core` (instrumentacion) -> `@telemetry-ops` (alerta); se pagina al oficial de guardia de gobernanza cuando el contador incrementa de forma inesperada. | 2026-02-26 | Salidas de dry-run de gobernanza almacenadas junto a `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; la checklist de release incluye la captura de la consulta de Prometheus mas el extracto de logs que prueba que `StateTelemetry::record_nexus_config_diff` emitio el diff. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) con alerta **`NexusAuditOutcomeFailure`** cuando fallas o resultados faltantes persisten por >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) con escalamiento a `@sec-observability`. | 2026-02-27 | La compuerta de CI `scripts/telemetry/check_nexus_audit_outcome.py` archiva payloads NDJSON y falla cuando una ventana TRACE carece de un evento de exito; capturas de alertas adjuntas al reporte routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` con guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` que alimenta la checklist on-call de SRE. | `@telemetry-ops` (gauge/export) con escalamiento a `@nexus-core` cuando los nodos reportan tamanos de catalogo inconsistentes. | 2026-02-28 | La prueba de telemetria del scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` demuestra la emision; los operadores adjuntan Prometheus diff + extracto de log de `StateTelemetry::set_nexus_catalogs` al paquete del ensayo TRACE. |

# Flujo operativo

1. **Triage semanal.** Los owners reportan progreso en la llamada de readiness de Nexus; blockers y artefactos de pruebas de alerta se registran en `status.md`.
2. **Ensayos de alertas.** Cada regla de alerta se entrega junto con una entrada en `dashboards/alerts/tests/*.test.yml` para que CI ejecute `promtool test rules` cuando cambie el guardrail.
3. **Evidencia de auditoria.** Durante los ensayos `TRACE-LANE-ROUTING` y `TRACE-TELEMETRY-BRIDGE` el on-call captura los resultados de consultas de Prometheus, el historial de alertas y las salidas relevantes de scripts (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para senales correlacionadas) y los almacena con los artefactos routed-trace.
4. **Escalamiento.** Si algun guardrail se dispara fuera de una ventana ensayada, el equipo responsable abre un ticket de incidente Nexus que referencia este plan, incluyendo el snapshot de la metrica y los pasos de mitigacion antes de reanudar las auditorias.

Con esta matriz publicada - y referenciada desde `roadmap.md` y `status.md` - el item de roadmap **B2** ahora cumple los criterios de aceptacion "responsabilidad, fecha limite, alerta, verificacion".
