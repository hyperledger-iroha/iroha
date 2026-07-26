---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6866c0c8d2d58023b5ba57e28f5b43d7795a20c679a7c98649941a1cb8d63c7f
source_last_modified: "2026-01-03T18:07:58+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-operations
title: Runbook de operaciones de Nexus
description: Resumen listo para el campo del flujo de trabajo del operador de Nexus, que refleja `docs/source/nexus_operations.md`.
---

Usa esta pagina como el hermano de referencia rapida de `docs/source/nexus_operations.md`. Resume la lista operativa, los ganchos de gestion de cambios y los requisitos de cobertura de telemetria que los operadores de Nexus deben seguir.

## Lista de ciclo de vida

| Etapa | Acciones | Evidencia |
|-------|--------|----------|
| Pre-vuelo | Verifica hashes/firmas de lanzamiento, confirma `profile = "iroha3"` y prepara plantillas de configuracion. | Salida de `scripts/select_release_profile.py`, registro de checksum, bundle de manifiestos firmado. |
| Alineacion del catalogo | Actualiza el catalogo `[nexus]`, la politica de enrutamiento y los umbrales de DA segun el manifiesto emitido por el consejo, y luego captura `--trace-config`. | Salida de `irohad --sora --config ... --trace-config` almacenada con el ticket de onboarding. |
| Pruebas de humo y corte | Ejecuta `irohad --sora --config ... --trace-config`, ejecuta la prueba de humo del CLI (`FindNetworkStatus`), valida las exportaciones de telemetria y solicita admision. | Log de smoke-test + confirmacion de Alertmanager. |
| Estado estable | Monitorea dashboards/alertas, rota claves segun la cadencia de gobernanza y sincroniza configs/runbooks cuando cambien los manifiestos. | Minutas de revision trimestral, capturas de dashboards, IDs de tickets de rotacion. |

El onboarding detallado (reemplazo de claves, plantillas de enrutamiento, pasos del perfil de lanzamiento) permanece en `docs/source/sora_nexus_operator_onboarding.md`.

## Gestion de cambios

1. **Actualizaciones de lanzamiento** - sigue anuncios en `status.md`/`roadmap.md`; adjunta la checklist de onboarding a cada PR de release.
2. **Cambios de manifiestos de lane** - verifica bundles firmados del Space Directory y archivados bajo `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuracion** - cada cambio en `config/config.toml` requiere un ticket que referencie la lane/dataspace. Guarda una copia redactada de la configuracion efectiva cuando los nodos se unan o se actualicen.
4. **Simulacros de rollback** - ensaya trimestralmente procedimientos de stop/restore/smoke; registra resultados en `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprobaciones de compliance** - lanes privadas/CBDC deben asegurar el visto bueno de compliance antes de modificar la politica de DA o los knobs de redaccion de telemetria (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetria y SLOs

- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, mas vistas especificas de SDK (por ejemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` y reglas de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Metricas a vigilar:
  - `nexus_lane_height{lane_id}` - alerta si no hay progreso durante tres slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerta por encima de umbrales por lane (por defecto 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta cuando el P99 supera 900 ms (public) o 1200 ms (private).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta si la tasa de error a 5 minutos supera el 2%.
  - `telemetry_redaction_override_total` - Sev 2 inmediato; asegura que las anulaciones tengan tickets de compliance.
- Ejecuta la checklist de remediacion de telemetria en el [plan de remediacion de telemetria de Nexus](./nexus-telemetry-remediation) al menos trimestralmente y adjunta el formulario completado a las notas de revision de operaciones.

## Matriz de incidentes

| Severidad | Definicion | Respuesta |
|----------|------------|----------|
| Sev 1 | Brecha de aislamiento de data-space, paro de settlement >15 min o corrupcion de voto de gobernanza. | Pagear a Nexus Primary + Release Engineering + Compliance, congelar admision, recolectar artefactos, publicar comunicados <=60 min, RCA <=5 dias habiles. |
| Sev 2 | Incumplimiento de SLA de backlog de lane, punto ciego de telemetria >30 min, rollout de manifiestos fallido. | Pagear a Nexus Primary + SRE, mitigar <=4 h, registrar follow-ups en 2 dias habiles. |
| Sev 3 | Deriva no bloqueante (docs, alertas). | Registrar en el tracker y planificar el arreglo dentro del sprint. |

Los tickets de incidentes deben registrar IDs de lane/data-space afectadas, hashes de manifiesto, linea de tiempo, metricas/logs de soporte y tareas/owners de seguimiento.

## Archivo de evidencias

- Guarda bundles/manifiestos/exportaciones de telemetria bajo `artifacts/nexus/<lane>/<date>/`.
- Conserva configs redactadas + salida de `--trace-config` para cada release.
- Adjunta minutas del consejo + decisiones firmadas cuando aterricen cambios de config o manifiesto.
- Conserva snapshots semanales de Prometheus relevantes para metricas de Nexus durante 12 meses.
- Registra ediciones del runbook en `docs/source/project_tracker/nexus_config_deltas/README.md` para que los auditores sepan cuando cambiaron las responsabilidades.

## Material relacionado

- Resumen: [Nexus overview](./nexus-overview)
- Especificacion: [Nexus spec](./nexus-spec)
- Geometria de lanes: [Nexus lane model](./nexus-lane-model)
- Transicion y shims de routing: [Nexus transition notes](./nexus-transition-notes)
- Onboarding de operadores: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- Remediacion de telemetria: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
