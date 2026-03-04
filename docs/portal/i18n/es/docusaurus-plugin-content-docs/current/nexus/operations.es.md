---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-nexus
título: Runbook de operaciones de Nexus
descripción: Resumen listo para el campo del flujo de trabajo del operador de Nexus, que refleja `docs/source/nexus_operations.md`.
---

Usa esta página como el hermano de referencia rápida de `docs/source/nexus_operations.md`. Resume la lista operativa, los ganchos de gestión de cambios y los requisitos de cobertura de telemetría que los operadores de Nexus deben seguir.

## Lista de ciclo de vida| Etapa | Acciones | Pruebas |
|-------|--------|----------|
| Pre-vuelo | Verifica hashes/firmas de lanzamiento, confirma `profile = "iroha3"` y prepara plantillas de configuración. | Salida de `scripts/select_release_profile.py`, registro de suma de comprobación, paquete de manifiestos firmados. |
| Alineación del catálogo | Actualiza el catálogo `[nexus]`, la política de enrutamiento y los umbrales de DA según el manifiesto emitido por el consejo, y luego captura `--trace-config`. | Salida de `irohad --sora --config ... --trace-config` almacenada con el ticket de onboarding. |
| Pruebas de humo y corte | Ejecuta `irohad --sora --config ... --trace-config`, ejecuta la prueba de humo del CLI (`FindNetworkStatus`), valida las exportaciones de telemetría y solicita admisión. | Registro de prueba de humo + confirmación de Alertmanager. |
| Estado estable | Monitorea tableros/alertas, rota claves según la cadencia de gobernanza y sincroniza configuraciones/runbooks cuando cambien los manifiestos. | Minutas de revisión trimestral, capturas de tableros, IDs de tickets de rotación. |

El onboarding detallado (reemplazo de claves, plantillas de enrutamiento, pasos del perfil de lanzamiento) permanece en `docs/source/sora_nexus_operator_onboarding.md`.

## Gestión de cambios1. **Actualizaciones de lanzamiento** - sigue anuncios en `status.md`/`roadmap.md`; adjunte la checklist de onboarding a cada PR de liberación.
2. **Cambios de manifiestos de carril** - verifica paquetes firmados del Space Directory y archivados bajo `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuración** - cada cambio en `config/config.toml` requiere un ticket que haga referencia a lane/dataspace. Guarde una copia redactada de la configuración efectiva cuando los nodos se unan o se actualicen.
4. **Simulacros de rollback** - ensaya trimestralmente procedimientos de stop/restore/smoke; registrar resultados en `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprobaciones de cumplimiento** - lanes privadas/CBDC deben asegurar el visto bueno de cumplimiento antes de modificar la política de DA o los botones de redacción de telemetría (ver `docs/source/cbdc_lane_playbook.md`).

## Telemetría y SLO- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, más vistas especificaciones de SDK (por ejemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` y reglas de transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas de un vigilante:
  - `nexus_lane_height{lane_id}` - alerta si no hay progreso durante tres slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerta por encima de umbrales por carril (por defecto 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - alerta cuando el P99 supera 900 ms (público) o 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta si la tasa de error a 5 minutos supera el 2%.
  - `telemetry_redaction_override_total` - Sever 2 inmediato; asegura que las anulaciones tengan tickets de cumplimiento.
- Ejecuta la checklist de remediacion de telemetria en el [plan de remediacion de telemetria de Nexus](./nexus-telemetry-remediation) al menos trimestralmente y adjunta el formulario completado a las notas de revisión de operaciones.

## Matriz de incidentes| Severidad | Definición | Respuesta |
|----------|------------|----------|
| Septiembre 1 | Brecha de aislamiento de data-space, paro de asentamiento >15 min o corrupción de voto de gobernanza. | Pagear a Nexus Primary + Release Engineering + Compliance, congelar admision, recolectar artefactos, publicar comunicados 30 min, rollout de manifiestos fallido. | Pagear a Nexus Primario + SRE, mitigar <=4 h, seguimiento de registrador en 2 días habiles. |
| Septiembre 3 | Deriva no bloqueante (docs, alertas). | Registrar en el tracker y planificar el arreglo dentro del sprint. |

Los tickets de incidentes deben registrar IDs de lane/data-space afectados, hashes de manifiesto, línea de tiempo, métricas/logs de soporte y tareas/owners de seguimiento.

##Archivo de evidencias

- Guarda paquetes/manifiestos/exportaciones de telemetria bajo `artifacts/nexus/<lane>/<date>/`.
- Conserva las configuraciones redactadas + salida de `--trace-config` para cada lanzamiento.
- Adjunta minutas del consejo + decisiones firmadas cuando aterricen cambios de configuración o manifiesto.
- Conserva instantáneas semanales de Prometheus relevantes para métricas de Nexus durante 12 meses.
- Registre ediciones del runbook en `docs/source/project_tracker/nexus_config_deltas/README.md` para que los auditores sepan cuando cambien las responsabilidades.

## Material relacionado- Resumen: [descripción general de Nexus](./nexus-overview)
- Especificación: [Especificación Nexus](./nexus-spec)
- Geometria de carriles: [Nexus modelo de carril](./nexus-lane-model)
- Transicion y shims de ruteo: [Nexus notas de transición](./nexus-transition-notes)
- Onboarding de operadores: [Sora Nexus onboarding de operadores](./nexus-operator-onboarding)
- Remediacion de telemetria: [Nexus plan de remediación de telemetría](./nexus-telemetry-remediation)