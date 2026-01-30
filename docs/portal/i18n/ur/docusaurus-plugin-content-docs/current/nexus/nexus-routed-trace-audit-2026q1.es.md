---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-routed-trace-audit-2026q1
title: Informe de auditoria de routed-trace 2026 Q1 (B1)
description: Espejo de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, que cubre los resultados de la revision trimestral de telemetria.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Fuente canonica
Esta pagina refleja `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Manten ambas copias alineadas hasta que lleguen las traducciones restantes.
:::

# Informe de auditoria de Routed-Trace 2026 Q1 (B1)

El item del roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** requiere una revision trimestral del programa routed-trace de Nexus. Este informe documenta la ventana de auditoria Q1 2026 (enero-marzo) para que el consejo de gobernanza pueda aprobar la postura de telemetria antes de los ensayos de lanzamiento Q2.

## Alcance y linea de tiempo

| Trace ID | Ventana (UTC) | Objetivo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verificar histogramas de admision de lane, gossip de colas y flujo de alertas antes de habilitar multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Validar replay OTLP, paridad del diff bot e ingestion de telemetria de SDK antes de los hitos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas de `iroha_config` aprobados por gobernanza y la preparacion de rollback antes del corte RC1. |

Cada ensayo se ejecuto en topologia tipo produccion con la instrumentacion routed-trace habilitada (telemetria `nexus.audit.outcome` + contadores Prometheus), reglas de Alertmanager cargadas y evidencia exportada en `docs/examples/`.

## Metodologia

1. **Recoleccion de telemetria.** Todos los nodos emitieron el evento estructurado `nexus.audit.outcome` y las metricas acompanantes (`nexus_audit_outcome_total*`). El helper `scripts/telemetry/check_nexus_audit_outcome.py` hizo tail del log JSON, valido el estado del evento y archivo el payload en `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Validacion de alertas.** `dashboards/alerts/nexus_audit_rules.yml` y su harness de pruebas aseguraron que los umbrales de ruido de alertas y el templating del payload se mantuvieran consistentes. CI ejecuta `dashboards/alerts/tests/nexus_audit_rules.test.yml` en cada cambio; las mismas reglas se ejercitaron manualmente durante cada ventana.
3. **Captura de dashboards.** Los operadores exportaron los paneles routed-trace de `dashboards/grafana/soranet_sn16_handshake.json` (salud de handshake) y los dashboards de telemetria general para correlacionar la salud de colas con los resultados de auditoria.
4. **Notas de revisores.** La secretaria de gobernanza registro iniciales de revisores, decision y tickets de mitigacion en [Nexus transition notes](./nexus-transition-notes) y el tracker de deltas de configuracion (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Hallazgos

| Trace ID | Resultado | Evidencia | Notas |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Capturas de alerta fire/recover (enlace interno) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados en [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admision de cola se mantuvo en 612 ms (objetivo <=750 ms). No se requiere seguimiento. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Payload archivado `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` mas el hash de replay OTLP registrado en `status.md`. | Los salts de redaccion de SDK coincidieron con la base Rust; el diff bot reporto cero deltas. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | Entrada en el tracker de gobernanza (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest de paquete de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La reejecucion Q2 hashio el perfil TLS aprobado y confirmo cero rezagados; el manifest de telemetria registra el rango de slots 912-936 y el workload seed `NEXUS-REH-2026Q2`. |

Todos los traces produjeron al menos un evento `nexus.audit.outcome` dentro de sus ventanas, satisfaciendo los guardrails de Alertmanager (`NexusAuditOutcomeFailure` se mantuvo en verde durante el trimestre).

## Follow-ups

- Se actualizo el apendice routed-trace con el hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; la mitigacion `NEXUS-421` se cerro en las transition notes.
- Continuar adjuntando replays OTLP sin procesar y artefactos de diff de Torii al archivo para reforzar la evidencia de paridad para revisiones de Android AND4/AND7.
- Confirmar que las proximas rehearsals `TRACE-MULTILANE-CANARY` reutilicen el mismo helper de telemetria para que el sign-off de Q2 se beneficie del flujo validado.

## Indice de artefactos

| Activo | Ubicacion |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Reglas y tests de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Payload de outcome de ejemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de delta de configuracion | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Schedule y notas de routed-trace | [Nexus transition notes](./nexus-transition-notes) |

Este informe, los artefactos anteriores y las exportaciones de alertas/telemetria deben adjuntarse al registro de decision de gobernanza para cerrar B1 del trimestre.
