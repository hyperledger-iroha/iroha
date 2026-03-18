---
lang: es
direction: ltr
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

% Plan de remediacion de telemetria de Nexus (Fase B2)

# Resumen

El item de roadmap **B2 - telemetry gap ownership** requiere un plan publicado que vincule cada
brecha de telemetria de Nexus pendiente con una senal, un guardrail de alerta, un responsable,
una fecha limite y un artefacto de verificacion antes de que comiencen las ventanas de auditoria
Q1 2026. Este documento centraliza esa matriz para que release engineering, operaciones de
telemetria y owners de SDK confirmen la cobertura antes de los ensayos de routed-trace y
`TRACE-TELEMETRY-BRIDGE`.

# Matriz de brechas

| ID de brecha | Senal y guardrail de alerta | Owner / Escalacion | Vence (UTC) | Evidencia y verificacion |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` con alerta **`SoranetLaneAdmissionLatencyDegraded`** que dispara cuando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` durante 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (senal) + `@telemetry-ops` (alerta) - escalar via on-call de Nexus routed-trace. | 2026-02-23 | Pruebas de alerta en `dashboards/alerts/tests/soranet_lane_rules.test.yml` y la captura del ensayo `TRACE-LANE-ROUTING` mostrando la alerta disparada/recuperada, mas el scrape de Torii `/metrics` archivado en `docs/source/nexus_transition_notes.md`. |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` con guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` que bloquea despliegues (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (alerta) - el duty officer de gobernanza es avisado cuando el contador incrementa de forma inesperada. | 2026-02-26 | Salidas de dry-run de gobernanza guardadas junto a `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; el checklist de release incluye la captura de la consulta Prometheus mas el extracto de logs que prueba que `StateTelemetry::record_nexus_config_diff` emitio el diff. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) con alerta **`NexusAuditOutcomeFailure`** cuando fallas o resultados ausentes persisten por >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) con escalacion a `@sec-observability`. | 2026-02-27 | El gate de CI `scripts/telemetry/check_nexus_audit_outcome.py` archiva payloads NDJSON y falla cuando una ventana TRACE carece de un evento de exito; capturas de alerta adjuntas al reporte de routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` monitoreado con guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` (documentado en `docs/source/telemetry.md`) que alimenta la checklist de guardia SRE. | `@telemetry-ops` (gauge/export) con escalacion a `@nexus-core` cuando los nodos reportan tamanos de catalogo inconsistentes. | 2026-02-28 | La prueba de telemetria del scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` prueba la emision; los operadores adjuntan el diff Prometheus mas el extracto de log de `StateTelemetry::set_nexus_catalogs` al paquete de ensayo TRACE. |

# Presupuesto de exportacion y limites OTLP

- **Decision (2026-02-11):** limitar exportadores OTLP a **5 MiB/min por nodo** o
  **25,000 spans/min**, lo que sea menor, con un tamano de lote de 256 spans y un timeout de
  exportacion de 10 segundos. Exportes por encima del 80% del limite disparan la alerta
  `NexusOtelExporterSaturated` en `dashboards/alerts/nexus_telemetry_rules.yml` y emiten el evento
  `telemetry_export_budget_saturation` para los logs de auditoria.
- **Enforcement:** las reglas Prometheus leen los contadores `iroha.telemetry.export.bytes_total`
  y `iroha.telemetry.export.spans_total`; el perfil del collector OTLP entrega los mismos limites
  como defaults y las configs en nodo no deben aumentarlos sin una exencion de gobernanza.
- **Evidencia:** los vectores de prueba de alertas y los limites aprobados se archivan bajo
  `docs/source/nexus_transition_notes.md` junto con los artefactos de auditoria de routed-trace.
  La aceptacion B2 ahora trata el presupuesto de exportacion como cerrado.

# Flujo operativo

1. **Triage semanal.** Los owners reportan progreso en la llamada de readiness de Nexus; bloqueos y
   artefactos de pruebas de alertas se registran en `status.md`.
2. **Dry-runs de alertas.** Cada regla de alerta se publica junto con una entrada en
   `dashboards/alerts/tests/*.test.yml` para que CI ejecute `promtool test rules` cada vez que cambia
   el guardrail.
3. **Evidencia de auditoria.** Durante los ensayos `TRACE-LANE-ROUTING` y `TRACE-TELEMETRY-BRIDGE`
   la guardia captura los resultados de consultas Prometheus, el historial de alertas y las salidas
   de scripts relevantes (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` para senales correladas) y los guarda con los
   artefactos de routed-trace.
4. **Escalacion.** Si cualquier guardrail dispara fuera de una ventana ensayada, el equipo responsable
   abre un ticket de incidente Nexus que referencia este plan, incluyendo el snapshot de metricas y
   los pasos de mitigacion antes de reanudar auditorias.

Con esta matriz publicada y referenciada desde `roadmap.md` y `status.md`, el item de roadmap
**B2** ahora cumple los criterios de aceptacion de "responsabilidad, fecha limite, alerta,
verificacion".
