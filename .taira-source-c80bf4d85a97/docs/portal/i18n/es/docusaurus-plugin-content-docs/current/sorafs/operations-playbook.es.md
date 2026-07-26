---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operaciones
título: Playbook de operaciones de SoraFS
sidebar_label: Manual de operaciones
descripción: Guías de respuesta a incidentes y procedimientos de simulacros de caos para operadores de SoraFS.
---

:::nota Fuente canónica
Esta página refleja el runbook mantenido en `docs/source/sorafs_ops_playbook.md`. Mantenga ambas copias sincronizadas hasta que el conjunto de documentación Sphinx se migre por completo.
:::

## Referencias clave

- Activos de observabilidad: consulte los paneles de control de Grafana en `dashboards/grafana/` y las reglas de alerta de Prometheus en `dashboards/alerts/`.
- Catálogo de métricas: `docs/source/sorafs_observability_plan.md`.
- Superficies de telemetría del orquestador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de escalamiento| Prioridad | Ejemplos de disparo | Director de guardia | Copia de seguridad | Notas |
|----------|---------------------|------------------|--------|-------|
| P1 | Caída global del gateway, tasa de fallos PoR > 5% (15 min), backlog de replicación duplicándose cada 10 min | Almacenamiento SRE | Observabilidad TL | Involucra al consejo de gobernanza si el impacto supera 30 min. |
| P2 | Incumplimiento de SLO de latencia regional del gateway, pico de reintentos del orquestador sin impacto de SLA | Observabilidad TL | Almacenamiento SRE | Continúa el despliegue pero bloquea nuevos manifiestos. |
| P3 | Alertas no críticas (estancamiento de manifiestos, capacidad 80–90%) | Triaje de admisión | Gremio de operaciones | Resolver en el siguiente día hábil. |

## Caída del gateway / disponibilidad degradada

**Detección**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tablero: `dashboards/grafana/sorafs_gateway_overview.json`.

**Acciones inmediatas**

1. Confirma el alcance (proveedor único vs flota) vía el panel de tasa de solicitudes.
2. Cambia el enrutamiento de Torii a proveedores sanos (si es multiproveedor) activando `sorafs_gateway_route_weights` en la configuración de operaciones (`docs/source/sorafs_gateway_self_cert.md`).
3. Si todos los proveedores están afectados, habilite el respaldo de “direct fetch” para clientes CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triaje**- Revisa la utilización del token de stream frente a `sorafs_gateway_stream_token_limit`.
- Inspecciona registros del gateway por errores de TLS o de admisión.
- Ejecuta `scripts/telemetry/run_schema_diff.sh` para asegurar que el esquema exportado por el gateway coincida con la versión esperada.

**Opciones de remediación**

- Reinicia solo el proceso de gateway afectado; Evite reciclar todo el cluster salvo que caigan varios proveedores.
- Aumente el límite de tokens de flujo en 10–15% de forma temporal si se confirma saturación.
- Reejecuta el self-cert (`scripts/sorafs_gateway_self_cert.sh`) tras la estabilización.

**Después del incidente**

- Registre un postmortem P1 usando `docs/source/sorafs/postmortem_template.md`.
- Programa un taladro de caos de seguimiento si la remediación requirió intervenciones manuales.

## Pico de fallos de pruebas (PoR / PoTR)

**Detección**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tablero: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetría: `torii_sorafs_proof_stream_events_total` y eventos `sorafs.fetch.error` con `provider_reason=corrupt_proof`.

**Acciones inmediatas**

1. Congela nuevas admisiones de manifests marcando el registro de manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifica a Governance para pausar incentivos de los proveedores afectados.

**Triaje**

- Revisa la profundidad de la cola de desafíos PoR frente a `sorafs_node_replication_backlog_total`.
- Valida el pipeline de verificación de pruebas (`crates/sorafs_node/src/potr.rs`) para despliegues recientes.
- Compara versiones de firmware de proveedores con el registro de operadores.**Opciones de remediación**

- Dispara replays de PoR usando `sorafs_cli proof stream` con el manifiesto más reciente.
- Si las pruebas fallan consistentemente, elimina el proveedor del conjunto activo actualizando el registro de gobernanza y forzando el refresco de marcadores del orquestador.

**Después del incidente**

- Ejecuta el escenario de taladro de caos PoR antes del siguiente implementar una producción.
- Captura aprendizajes en la plantilla de postmortem y actualiza la lista de verificación de calificación de proveedores.

## Retraso de replicación / crecimiento del backlog

**Detección**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. importa
  `dashboards/alerts/sorafs_capacity_rules.yml` y ejecuta
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  antes de la promoción para que Alertmanager refleje los umbrales documentados.
- Tablero: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Acciones inmediatas**

1. Verifica el alcance del backlog (proveedor único o flota) y pausa tareas de replicación no esenciales.
2. Si el backlog está aislado, reasigna temporalmente nuevos pedidos a proveedores alternos mediante el planificador de replicación.

**Triaje**

- Inspecciona la telemetría del orquestador por ráfagas de reintentos que puedan escalar el backlog.
- Confirma que los objetivos de almacenamiento tienen suficiente espacio libre (`sorafs_node_capacity_utilisation_percent`).
- Revisar cambios recientes de configuración (actualizaciones de perfiles de chunk, cadencia de pruebas).**Opciones de remediación**

- Ejecuta `sorafs_cli` con la opción `--rebalance` para redistribuir contenido.
- Escalar horizontalmente los trabajadores de replicación para el proveedor afectado.
- Dispara una actualización de manifiestos para realinear las ventanas TTL.

**Después del incidente**

- Programa un taladro de capacidad enfocado en fallos por saturación de proveedores.
- Actualiza la documentación de SLA de replicación en `docs/source/sorafs_node_client_protocol.md`.

## Cadencia de ejercicios de caos

- **Trimestral**: simulación combinada de caída de gateway + tormenta de reintentos del orquestador.
- **Semestral**: inyección de fallos PoR/PoTR en dos proveedores con recuperación.
- **Chequeo mensual**: escenario de retraso de replicación usando manifiestos de puesta en escena.
- Registra los taladros en el log compartido (`ops/drill-log.md`) vía:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Valida el log antes de los commits con:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Usa `--status scheduled` para taladros futuros, `pass`/`fail` para ejecuciones completas, y `follow-up` cuando queden acciones abiertas.
- Sobrescribe el destino con `--log` para ensayos o verificación automatizada; sin eso el script sigue actualizando `ops/drill-log.md`.

## Plantilla de autopsiaUsa `docs/source/sorafs/postmortem_template.md` para cada incidente P1/P2 y para retrospectivas de simulacros de caos. La plantilla cubre cronología, cuantificación de impacto, factores contribuyentes, acciones correctivas y tareas de verificación de seguimiento.