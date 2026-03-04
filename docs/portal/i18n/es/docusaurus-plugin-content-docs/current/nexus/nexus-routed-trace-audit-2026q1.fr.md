---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
título: Informe de auditoría de seguimiento enrutado 2026 Q1 (B1)
descripción: Miroir de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, que cubre los resultados trimestrales de las repeticiones de telemetría.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Gardez les deux copys alignees jusqu'a ce que les traductions restantes llegan.
:::

# Informe de auditoría Routed-Trace 2026 Q1 (B1)

El elemento de la hoja de ruta **B1 - Línea base de telemetría y auditorías de seguimiento de ruta** requiere una revisión trimestral del programa de seguimiento de ruta Nexus. Este informe documenta la ventana de auditoría del primer trimestre de 2026 (enero-marzo) después de que el consejo de gobierno pueda validar la postura de la telemetría antes de las repeticiones de lanzamiento del segundo trimestre.

## Portée y calendrier

| ID de seguimiento | Fenêtre (UTC) | Objetivo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verifique los histogramas de admisión de carriles, los chismes de archivos y el flujo de alertas antes de la activación de varios carriles. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valide la reproducción OTLP, la partición del robot de diferenciación y la ingesta de telemetría SDK antes de los botones AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirme los deltas `iroha_config` aprobando la gestión y la preparación au rollback antes del corte RC1. |Cada repetición de un torneo sobre una topología cercana a la producción con la instrumentación de seguimiento enrutado activo (telemetría `nexus.audit.outcome` + computadoras Prometheus), las reglas Alertmanager cargadas y las mejores exportaciones en `docs/examples/`.

## Metodología

1. **Collecte de telemetrie.** Tous les noeuds ont emis l'evenement estructura `nexus.audit.outcome` et les metriques associees (`nexus_audit_outcome_total*`). El asistente `scripts/telemetry/check_nexus_audit_outcome.py` agrega el siguiente registro JSON, valida el estado del evento y archiva la carga útil en `docs/examples/nexus_audit_outcomes/`. [scripts/telemetría/check_nexus_audit_outcome.py:1]
2. **Validación de alertas.** `dashboards/alerts/nexus_audit_rules.yml` y su arnés de prueba garantizan que las señales de ruido y la plantilla de cargas útiles sean coherentes. CI ejecuta `dashboards/alerts/tests/nexus_audit_rules.test.yml` una modificación cada vez; les memes regles ont ete exercees manuellement colgante cada fenetre.
3. **Captura de paneles.** Los operadores exportan los paneles de seguimiento enrutado desde `dashboards/grafana/soranet_sn16_handshake.json` (sante handshake) y los paneles de telemetría global para correlacionar la seguridad de los archivos con los resultados de auditoría.
4. **Notes des relecteurs.** El secretaire de gouvernance a consigna les iniciales, la decision et les tickets de mitigation dans [Nexus notas de transición](./nexus-transition-notes) et dans le tracker de deltas de config (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Constataciones| ID de seguimiento | Resultado | Preuves | Notas |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pase | Captura alerta de incendio/recuperación (lien interna) + repetición `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diferencias de registros de telemetría en [notas de transición Nexus] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | El P95 de admisión de archivos dura 612 ms (puede <= 750 ms). Aucun suivi requis. |
| `TRACE-TELEMETRY-BRIDGE` | Pase | Archivo de carga útil `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` más hash de reproducción OTLP registrado en `status.md`. | Los propios SDK de redacción corresponden a la base Rust; la diferencia entre una señal delta cero. |
| `TRACE-CONFIG-DELTA` | Pasa (mitigación cerrada) | Entrada del rastreador de gobierno (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + perfil de manifiesto TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + telemetría del paquete de manifiesto (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La repetición del segundo trimestre ha hecho que el perfil TLS apruebe y confirme cero retrasos; La telemetría del manifiesto registra la ubicación de las ranuras 912-936 y la carga de trabajo `NEXUS-REH-2026Q2`. |

Todos los rastros del producto al menos un evento `nexus.audit.outcome` en sus ventanas, satisfacen las barandillas Alertmanager (`NexusAuditOutcomeFailure` están remanentes en el trimestre).

## Suivis- El apéndice routed-trace a ete mis a jour avec le hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; La mitigación `NEXUS-421` está cerrada en las notas de transición.
- Continúe uniendo las repeticiones OTLP brutas y los artefactos de diferencia Torii al archivo para reforzar la presión de paridad para las revisiones Android AND4/AND7.
- Confirmer que les prochaines rehearsals `TRACE-MULTILANE-CANARY` reutilisent le meme helper de telemetrie pour que la validation Q2 beneficie du workflow valide.

## Índice de artefactos

| Activo | Emplazamiento |
|-------|----------|
| Validador de telemetría | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Reglas y pruebas de alerta | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Ejemplo de resultado de carga útil | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Rastreador de deltas de configuración | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Planificación y notas de seguimiento enrutado | [Notas de transición Nexus](./nexus-transition-notes) |

Ce rapport, les artefactos ci-dessus et les exports d'alertes/telemetrie doivent etre se adjuntan au journal de decision de gouvernance pour cloturer B1 du trimestre.