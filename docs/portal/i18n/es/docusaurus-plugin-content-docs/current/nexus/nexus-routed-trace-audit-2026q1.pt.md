---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
título: Relatorio de auditoria routed-trace 2026 Q1 (B1)
descripción: Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, cobrindo os resultados trimestrales das revisiones de telemetría.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Mantenha as duas copias alinhadas ate que as traducoes restantes cheguem.
:::

# Relatorio de auditoria Routed-Trace 2026 Q1 (B1)

El elemento de la hoja de ruta **B1 - Línea base de telemetría y auditorías de seguimiento de ruta** exige una revisión trimestral del programa de seguimiento de ruta de Nexus. Este relatorio documenta a janela de auditoria Q1 2026 (janeiro-marco) para que el consejo de gobierno possa aprovar a postura de telemetria antes dos ensayos de lancamento Q2.

## Escopo y cronograma

| ID de seguimiento | Janela (UTC) | Objetivo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verificar histogramas de admisión de carril, chismes de fila y flujo de alertas antes de habilitacao multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Validar la repetición de OTLP, paridad de diferencias entre bots y ingesta de telemetría de SDK antes de los marcos AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmar deltas de `iroha_config` aprovados pelagobernanza y prontidao de rollback antes de cortar RC1. |Cada ensayo rodou em topología similar a la producción con un instrumento de seguimiento enrutado habilitado (telemetría `nexus.audit.outcome` + contadores Prometheus), registros de Alertmanager cargados y evidencia exportada para `docs/examples/`.

## Metodología

1. **Coleta de telemetria.** Todos os nos emitiram o evento estruturado `nexus.audit.outcome` e as métricas asociadas (`nexus_audit_outcome_total*`). El ayudante `scripts/telemetry/check_nexus_audit_outcome.py` hace cola en el registro JSON, valida el estado del evento y archiva la carga útil en `docs/examples/nexus_audit_outcomes/`. [scripts/telemetría/check_nexus_audit_outcome.py:1]
2. **Validación de alertas.** `dashboards/alerts/nexus_audit_rules.yml` y su arnés de prueba garantiza que los limiares de ruido y la plantilla de la carga útil permanecen consistentes. O CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` a cada mudanca; as mesmas regras foram ejercitadas manualmente durante cada janela.
3. **Captura de paneles.** Los operadores exportan el rutado de seguimiento de `dashboards/grafana/soranet_sn16_handshake.json` (saude de handshake) y los paneles de control de visao general de telemetría para correlacionar a saude das filas con los resultados de auditoría.
4. **Notas de revisión.** A secretaría de gobierno registrou iniciais dos revisores, decisao e tickets de mitigacao em [Nexus notas de transición](./nexus-transition-notes) y no tracker de delta de configuracao (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

##Achados| ID de seguimiento | Resultado | Pruebas | Notas |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pase | Capturas de alerta fire/recover (enlace interno) + repetición de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetria registrados em [notas de transición Nexus](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 de admisión de la fila permaneceu em 612 ms (alvo <=750 ms). Seguimiento parcial. |
| `TRACE-TELEMETRY-BRIDGE` | Pase | Carga útil almacenada `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` más hash de reproducción OTLP registrado en `status.md`. | Las sales de redacción de la batería SDK con base Rust; El bot diff informa cero deltas. |
| `TRACE-CONFIG-DELTA` | Pasa (mitigación cerrada) | Entrada no rastreador de gobierno (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifiesto de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifiesto del paquete de telemetría (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Una repetición del segundo trimestre tiene el perfil TLS aprobado y confirma que no hay rezagados; o manifiesto de registro de telemetría o intervalo de ranuras 912-936 y o semilla de carga de trabajo `NEXUS-REH-2026Q2`. |

Todos los rastros se producen al menos un evento `nexus.audit.outcome` dentro de sus janelas, satisfazendo os guardrails do Alertmanager (`NexusAuditOutcomeFailure` permanece verde no trimestre).

## Seguimientos- El apéndice routed-trace actualizado con el hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; a mitigacao `NEXUS-421` foi encerrada nas notas de transición.
- Continuar anexando repeticiones OTLP brutos y artefactos de diferenciación de Torii al archivo para reforcar la evidencia de paridad para revisiones de Android AND4/AND7.
- Confirmar que a medida que los ensayos próximos `TRACE-MULTILANE-CANARY` reutilicen el mesmo helper de telemetria para que o sign-off de Q2 se beneficie del flujo de trabajo validado.

## Índice de artefatos

| activo | Locales |
|-------|----------|
| Validador de telemetria | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Registros y testículos de alertas | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Carga útil de resultado del ejemplo | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Rastreador delta de configuración | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Cronograma y notas routed-trace | [Notas de transición Nexus](./nexus-transition-notes) |

Este relatorio, os artefatos acima y os exports de alertas/telemetria deben ser anexados al registro de decisiones de gobierno para fechar o B1 do trimestre.