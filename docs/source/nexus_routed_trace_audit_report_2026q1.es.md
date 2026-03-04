<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: es
direction: ltr
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

# Reporte de auditoria Routed-Trace 2026 Q1 (B1)

El item de roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** requiere una
revision trimestral del programa routed-trace de Nexus. Este reporte documenta
la ventana de auditoria Q1 2026 (enero-marzo) para que el consejo de gobernanza
apruebe la postura de telemetria antes de los ensayos de lanzamiento Q2.

## Alcance y cronograma

| Trace ID | Ventana (UTC) | Objetivo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verificar histogramas de admision de lanes, gossip de colas y flujo de alertas antes de habilitar multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Validar replay OTLP, paridad del bot de diff e ingesta de telemetria del SDK antes de los hitos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas `iroha_config` aprobadas por gobernanza y preparacion de rollback antes del corte RC1. |

Cada ensayo se ejecuto en una topologia similar a produccion con la
instrumentacion routed-trace habilitada (telemetria `nexus.audit.outcome` +
contadores Prometheus), reglas de Alertmanager cargadas y evidencia exportada en
`docs/examples/`.

## Metodologia

1. **Coleccion de telemetria.** Todos los nodos emitieron el evento estructurado
   `nexus.audit.outcome` y las metricas asociadas (`nexus_audit_outcome_total*`). El helper
   `scripts/telemetry/check_nexus_audit_outcome.py` siguio el log JSON, valido el estado del evento
   y archivo el payload bajo `docs/examples/nexus_audit_outcomes/`
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **Validacion de alertas.** `dashboards/alerts/nexus_audit_rules.yml` y su arnes de pruebas
   aseguraron que los umbrales de ruido y el templating del payload se mantuvieran consistentes.
   CI ejecuta `dashboards/alerts/tests/nexus_audit_rules.test.yml` en cada cambio; las mismas reglas
   se ejercitaron manualmente durante cada ventana.
3. **Captura de dashboards.** Operadores exportaron los paneles routed-trace de
   `dashboards/grafana/soranet_sn16_handshake.json` (salud de handshake) y los dashboards de
   telemetria para correlacionar salud de colas con los resultados de auditoria.
4. **Notas de revision.** La secretaria de gobernanza registro las iniciales de revisores, la
   decision y cualquier ticket de mitigacion en `docs/source/nexus_transition_notes.md` y el tracker
   de config delta (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Hallazgos

| Trace ID | Resultado | Evidencia | Notas |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Capturas de alerta fire/recover (link interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados en `docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule`. | Queue-admission P95 se mantuvo en 612 ms (objetivo <=750 ms). No se requiere seguimiento. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Payload archivado `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mas hash de replay OTLP registrado en `status.md`. | Las sales de redaccion del SDK coincidieron con la baseline de Rust; el bot de diff reporto cero deltas. |
| `TRACE-CONFIG-DELTA` | Pass (mitigacion cerrada) | Entrada en el tracker de gobernanza (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest del pack de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La repeticion Q2 hizo hash del perfil TLS aprobado y confirmo cero rezagados; el manifest de telemetria registra el rango de slots 912-936 y la semilla de workload `NEXUS-REH-2026Q2`. |

Todos los traces produjeron al menos un evento `nexus.audit.outcome` dentro de
sus ventanas, satisfaciendo los guardrails de Alertmanager (`NexusAuditOutcomeFailure`
se mantuvo en verde durante el trimestre).

## Seguimientos

- Se actualizo el apendice routed-trace con el hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  (ver `nexus_transition_notes.md`); mitigacion `NEXUS-421` cerrada.
- Continuar adjuntando replays OTLP crudos y artefactos de diff de Torii al archivo para
  reforzar evidencia de paridad para revisiones AND4/AND7.
- Confirmar que los proximos ensayos `TRACE-MULTILANE-CANARY` reutilicen el mismo helper de
  telemetria para que el sign-off Q2 aproveche el flujo validado.

## Indice de artefactos

| Activo | Ubicacion |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Reglas y tests de alerta | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Payload de resultado de muestra | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma y notas routed-trace | `docs/source/nexus_transition_notes.md` |

Este reporte, los artefactos anteriores y las exportaciones de alerta/telemetria
se deben adjuntar al log de decision de gobernanza para cerrar B1 del trimestre.
