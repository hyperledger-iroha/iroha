---
lang: es
direction: ltr
source: docs/source/nexus_operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 413953b2ca41875bb960be56995aa03dbaa58af4a30f927c24d1e8815c7da472
source_last_modified: "2025-11-08T16:26:57.335679+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook de operaciones de Nexus (NX-14)

**Enlace del roadmap:** NX-14 - documentacion de Nexus y runbooks de operadores
**Estado:** Borrador 2026-03-24 - se alinea con `docs/source/nexus_overview.md` y
el flujo de onboarding en `docs/source/sora_nexus_operator_onboarding.md`.
**Audiencia:** Operadores de red, ingenieros SRE/on-call, coordinadores de gobernanza.

Este runbook resume el ciclo de vida operativo de nodos Sora Nexus (Iroha 3).
No reemplaza la especificacion profunda (`docs/source/nexus.md`) ni las guias por lane
(p. ej., `docs/source/cbdc_lane_playbook.md`), pero recopila los checklists concretos,
los hooks de telemetria y los requisitos de evidencia que deben cumplirse antes de
admitir o actualizar un nodo.

## 1. Ciclo de vida operativo

| Etapa | Checklist | Evidencia |
|-------|-----------|----------|
| **Pre-flight** | Validar hashes/firmas de artefactos, confirmar `profile = "iroha3"`, y preparar plantillas de config. | Salida de `scripts/select_release_profile.py`, log de checksum, bundle de manifest firmado. |
| **Alineacion de catalogo** | Actualizar catalogo de lane + dataspace en `[nexus]`, politica de ruteo, y umbrales de DA para que coincidan con el manifest emitido por el consejo. | Salida de `irohad --sora --config ... --trace-config` almacenada con el ticket. |
| **Smoke y cutover** | Ejecutar `irohad --sora --config ... --trace-config`, correr smoke test de CLI (p. ej., `FindNetworkStatus`), verificar endpoints de telemetria, luego solicitar admision. | Log de smoke-test + confirmacion de silencio en Alertmanager. |
| **Estado estable** | Monitorear dashboards/alertas, rotar llaves segun cadencia de gobernanza, y mantener configs + runbooks en sync con revisiones de manifest. | Actas de revision trimestral, capturas de dashboards enlazadas, y IDs de tickets de rotacion. |

Las instrucciones detalladas de onboarding (incluyendo reemplazo de llaves, ejemplos
de politica de ruteo y validacion de release profile) viven en
`docs/source/sora_nexus_operator_onboarding.md`. Referencia ese documento cuando
cambien los formatos de artefactos o los scripts.

## 2. Gestion de cambios y hooks de gobernanza

1. **Actualizaciones de release**
   - Seguir anuncios en `status.md` y `roadmap.md`.
   - Cada PR de release debe adjuntar el checklist completado de
     `docs/source/sora_nexus_operator_onboarding.md`.
2. **Cambios en lane manifest**
   - La gobernanza publica bundles de manifest firmados via Space Directory.
   - Los operadores verifican firmas, actualizan entradas de catalogo y archivan
     los manifests en `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuracion**
   - Todos los cambios en `config/config.toml` requieren un ticket que referencie
     el lane ID y el alias de dataspace.
   - Mantener una copia redactada del config efectivo en el ticket cuando el nodo
     se une o actualiza.
4. **Ensayos de rollback**
   - Realizar ensayos trimestrales de rollback (detener nodo, restaurar bundle previo,
     re-aplicar config, re-ejecutar smoke). Registrar resultados en
     `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Aprobaciones de compliance**
   - Lanes privadas/CBDC deben obtener sign-off de compliance antes de cambiar la
     politica de DA o knobs de redaccion de telemetria. Referencia
     `docs/source/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. Cobertura de telemetria y SLO

Dashboards y reglas de alertas estan versionados en `dashboards/` y documentados en
`docs/source/nexus_telemetry_remediation_plan.md`. Los operadores DEBEN:

- Suscribir objetivos de PagerDuty/on-call a `dashboards/alerts/nexus_audit_rules.yml`
  y las reglas de salud de lane en `dashboards/alerts/torii_norito_rpc_rules.yml`
  (cubre transporte Torii/Norito).
- Publicar los siguientes tableros Grafana en el portal de operaciones:
  - `nexus_lanes.json` (altura de lane, backlog, paridad DA).
  - `nexus_settlement.json` (latencia de settlement, deltas de tesoreria).
  - `android_operator_console.json` / dashboards de SDK cuando la lane depende de
    telemetria mobile.
- Mantener exportadores OTEL alineados con `docs/source/torii/norito_rpc_telemetry.md`
  cuando el transporte binario de Torii este habilitado.
- Ejecutar el checklist de remediation de telemetria al menos trimestralmente
  (Seccion 5 en `docs/source/nexus_telemetry_remediation_plan.md`) y adjuntar el
  formulario completado a las actas de revision de ops.

### Metricas clave

| Metrica | Descripcion | Umbral de alerta |
|--------|-------------|-----------------|
| `nexus_lane_height{lane_id}` | Altura de cabeza por lane; detecta validadores estancados. | Alertar si no aumenta por 3 slots consecutivos. |
| `nexus_da_backlog_chunks{lane_id}` | Chunks de DA sin procesar por lane. | Alertar por encima del limite configurado (default: 64 para public, 8 para private). |
| `nexus_settlement_latency_seconds{lane_id}` | Tiempo entre commit de lane y settlement global. | Alertar >900 ms P99 (public) o >1200 ms (private). |
| `torii_request_failures_total{scheme="norito_rpc"}` | Conteo de errores Norito RPC. | Alertar si la razon de errores de 5 minutos >2%. |
| `telemetry_redaction_override_total` | Overrides emitidos para redaccion de telemetria. | Alertar inmediatamente (Sev 2) y requerir ticket de compliance. |

## 4. Respuesta a incidentes

| Severidad | Definicion | Acciones requeridas |
|----------|------------|--------------------|
| **Sev 1** | Brecha de aislamiento de data space, paro de settlement >15 min, o corrupcion de votos de gobernanza. | Pagear Nexus Primary + Release Engineering + Compliance. Congelar admision de lane, recolectar metricas/logs, publicar comunicacion del incidente en 60 min, registrar RCA en <=5 dias habiles. |
| **Sev 2** | Backlog de lane excediendo SLA, punto ciego de telemetria >30 min, rollout de manifest fallido. | Pagear Nexus Primary + SRE, mitigar en 4 h, registrar issues de seguimiento en 2 dias habiles. |
| **Sev 3** | Regresiones no bloqueantes (drift de docs, alerta mal disparada). | Registrar en tracker, programar correccion dentro del sprint. |

Los tickets de incidentes deben incluir:

1. IDs de lane/data-space afectados y hashes de manifest.
2. Timeline (UTC) con deteccion, mitigacion, recuperacion y comunicaciones.
3. Metricas/capturas que respalden la deteccion.
4. Tareas de seguimiento (con responsables/fechas) y si se requieren
   actualizaciones de automatizacion/runbooks.

## 5. Evidencia y trazabilidad de auditoria

- **Archivo de artefactos:** Guardar bundles, manifests y exportes de telemetria en
  `artifacts/nexus/<lane>/<date>/`.
- **Snapshots de config:** `config.toml` redactado mas salida `trace-config` para
  cada release.
- **Enlace de gobernanza:** Actas de consejo y decisiones firmadas referenciadas
  en el ticket de onboarding o incidente.
- **Exportes de telemetria:** Snapshots semanales de chunks TSDB de Prometheus
  relacionados con la lane, adjuntos al share de auditoria por minimo 12 meses.
- **Versionado de runbook:** Cada cambio significativo a este archivo debe incluir
  una entrada de changelog en `docs/source/project_tracker/nexus_config_deltas/README.md`
  para que los auditores puedan rastrear cuando cambiaron los requisitos.

## 6. Recursos relacionados

- `docs/source/nexus_overview.md` - arquitectura/resumen de alto nivel.
- `docs/source/nexus.md` - especificacion tecnica completa.
- `docs/source/nexus_lanes.md` - geometria de lanes.
- `docs/source/nexus_transition_notes.md` - roadmap de migracion.
- `docs/source/cbdc_lane_playbook.md` - politicas especificas de CBDC.
- `docs/source/sora_nexus_operator_onboarding.md` - flujo de release/onboarding.
- `docs/source/nexus_telemetry_remediation_plan.md` - guardrails de telemetria.

Mantener estas referencias actualizadas cuando avance el item NX-14 o cuando se
introduzcan nuevas clases de lane, reglas de telemetria u hooks de gobernanza.
