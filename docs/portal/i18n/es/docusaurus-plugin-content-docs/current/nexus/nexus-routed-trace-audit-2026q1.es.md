---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
título: Informe de auditoría de routed-trace 2026 Q1 (B1)
descripción: Espejo de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, que cubre los resultados de la revisión trimestral de telemetría.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Mantenga ambas copias alineadas hasta que lleguen las traducciones restantes.
:::

# Informe de auditoria de Routed-Trace 2026 Q1 (B1)

El elemento del roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** requiere una revisión trimestral del programa routed-trace de Nexus. Este informe documenta la ventana de auditoria Q1 2026 (enero-marzo) para que el consejo de gobernanza pueda aprobar la postura de telemetría antes de los ensayos de lanzamiento Q2.

## Alcance y linea de tiempo

| ID de seguimiento | Ventana (UTC) | Objetivo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verificar histogramas de admisión de carril, chismes de colas y flujo de alertas antes de habilitar multicarril. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Validar replay OTLP, paridad del diff bot e ingestión de telemetría de SDK antes de los hitos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas de `iroha_config` aprobados por gobernanza y la preparación de rollback antes del corte RC1. |Cada ensayo se ejecuta en topología tipo producción con la instrumentación routed-trace habilitada (telemetria `nexus.audit.outcome` + contadores Prometheus), reglas de Alertmanager cargadas y evidencia exportada en `docs/examples/`.

## Metodología

1. **Recoleccion de telemetria.** Todos los nodos emitieron el evento estructurado `nexus.audit.outcome` y las métricas acompañantes (`nexus_audit_outcome_total*`). El helper `scripts/telemetry/check_nexus_audit_outcome.py` hizo cola del log JSON, validó el estado del evento y archivo el payload en `docs/examples/nexus_audit_outcomes/`. [scripts/telemetría/check_nexus_audit_outcome.py:1]
2. **Validacion de alertas.** `dashboards/alerts/nexus_audit_rules.yml` y su arnés de pruebas aseguraron que los umbrales de ruido de alertas y el templating del payload se mantuvieran consistentes. CI ejecuta `dashboards/alerts/tests/nexus_audit_rules.test.yml` en cada cambio; las mismas reglas se ejercitaron manualmente durante cada ventana.
3. **Captura de paneles.** Los operadores exportaron los paneles routed-trace de `dashboards/grafana/soranet_sn16_handshake.json` (salud de handshake) y los paneles de telemetría general para correlacionar la salud de colas con los resultados de auditoría.
4. **Notas de revisores.** La secretaría de gobernanza registro iniciales de revisores, decision y tickets de mitigacion en [Nexus Transition Notes](./nexus-transition-notes) y el tracker de deltas de configuración (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##hallazgos| ID de seguimiento | Resultado | Pruebas | Notas |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pase | Capturas de alerta fire/recover (enlace interno) + repetición de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados en [Nexus notas de transición](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admisión de cola se mantuvo en 612 ms (objetivo <=750 ms). No se requiere seguimiento. |
| `TRACE-TELEMETRY-BRIDGE` | Pase | Carga útil archivada `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` más el hash de repetición OTLP registrado en `status.md`. | Los sales de redacción de SDK coincidieron con la base Rust; el diff bot reporto cero deltas. |
| `TRACE-CONFIG-DELTA` | Pasa (mitigación cerrada) | Entrada en el tracker de gobernanza (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifiesto de paquete de telemetria (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La reejecucion Q2 tiene el perfil TLS aprobado y confirmo cero rezagados; el manifest de telemetria registra el rango de slots 912-936 y el workload seed `NEXUS-REH-2026Q2`. |

Todos los rastros producen al menos un evento `nexus.audit.outcome` dentro de sus ventanas, satisfaciendo los guardrails de Alertmanager (`NexusAuditOutcomeFailure` se mantuvo en verde durante el trimestre).

## Seguimientos- Se actualizó el apéndice routed-trace con el hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; la mitigacion `NEXUS-421` se cerro en las notas de transición.
- Continuar adjuntando repeticiones OTLP sin procesar y artefactos de diferencia de Torii al archivo para reforzar la evidencia de paridad para revisión de Android AND4/AND7.
- Confirmar que los próximos ensayos `TRACE-MULTILANE-CANARY` reutilizan el mismo helper de telemetria para que el sign-off de Q2 se beneficie del flujo validado.

## Índice de artefactos

| Activo | Ubicación |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Reglas y pruebas de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Carga útil de resultado de ejemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Rastreador delta de configuración | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Horario y notas de routed-trace | [Notas de transición Nexus](./nexus-transition-notes) |

Este informe, los artefactos anteriores y las exportaciones de alertas/telemetría deben adjuntarse al registro de decisión de gobernanza para cerrar B1 del trimestre.