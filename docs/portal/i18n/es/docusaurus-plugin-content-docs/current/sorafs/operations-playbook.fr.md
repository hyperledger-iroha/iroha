---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operaciones
título: Manual de estrategias de explotación SoraFS
sidebar_label: Guía de explotación
descripción: Guías de respuesta a incidentes y procedimientos de simulacros de caos para los operadores SoraFS.
---

:::nota Fuente canónica
Esta página refleja el mantenimiento del runbook en `docs/source/sorafs_ops_playbook.md`. Guarde las dos copias sincronizadas justo como la documentación Sphinx está totalmente migrada.
:::

## Referencias claves

- Funciones de observación: consulte los paneles de control Grafana en `dashboards/grafana/` y las reglas de alerta Prometheus en `dashboards/alerts/`.
- Catálogo de medidas: `docs/source/sorafs_observability_plan.md`.
- Superficies de télémétrie del orquestador: `docs/source/sorafs_orchestrator_plan.md`.

## Matrice d'escalade| Prioridad | Ejemplos de desaceleración | Director de guardia | Copia de seguridad | Notas |
|----------|---------------------|------------------|--------|-------|
| P1 | Panne globale gateway, taux d’échec PoR > 5% (15 min), backlog de replication doublant toutes les 10 min | Almacenamiento SRE | Observabilidad TL | Involucrar al consejo de gobierno si el impacto finaliza 30 min. |
| P2 | Violación del SLO de la puerta de enlace de latencia regional, imagen de reintentos orquestadores sin impacto SLA | Observabilidad TL | Almacenamiento SRE | Continúe con el lanzamiento pero bloquee los nuevos manifiestos. |
| P3 | Alertes non critiques (estancamiento de los manifiestos, capacidad 80–90%) | Triaje de admisión | Gremio de operaciones | À traidor en el día prochain ouvré. |

## Panne gateway / disponibilidad degradada

**Detección**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tablero: `dashboards/grafana/sorafs_gateway_overview.json`.

**Acciones inmediatas**

1. Confirme la puerta (fournisseur Unique vs flotte) a través del panel de taux de requêtes.
2. Basculez le routage Torii versus des fournisseurs sains (si multi-fournisseur) y basculant `sorafs_gateway_route_weights` dans la config ops (`docs/source/sorafs_gateway_self_cert.md`).
3. Si todos los proveedores están afectados, active la “búsqueda directa” alternativa para los clientes CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triaje**- Verifique la utilización de tokens de flujo según la relación `sorafs_gateway_stream_token_limit`.
- Inspeccione los registros de entrada para detectar errores TLS o de admisión.
- Ejecute `scripts/telemetry/run_schema_diff.sh` para verificar que el esquema exportado por el gateway corresponde a la versión presente.

**Opciones de reparación**

- Redémarrez Uniquement le Processus Gateway Affecté; Evite el reciclador en todo el grupo salvo si los proveedores más grandes lo hacen.
- Aumente temporalmente el límite de tokens de flujo de 10 a 15% si se confirma una saturación.
- Relancez le self-cert (`scripts/sorafs_gateway_self_cert.sh`) después de la estabilización.

**Después del incidente**

- Rédigez un post mortem P1 con `docs/source/sorafs/postmortem_template.md`.
- Planifique un simulacro de caos de seguimiento si la solución es necesaria mediante intervenciones manuales.

## Pic d'échecs de preuve (PoR / PoTR)

**Detección**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tablero: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemétrie: `torii_sorafs_proof_stream_events_total` y eventos `sorafs.fetch.error` con `provider_reason=corrupt_proof`.

**Acciones inmediatas**

1. Figer les nouvelles admisiones de manifiestos en marquant le registre de manifiestos (`docs/source/sorafs/manifest_pipeline.md`).
2. Notificar la gobernanza para suspender las incitaciones de los proveedores impactados.

**Triaje**- Verifique el profundidad del archivo de desafíos PoR face à `sorafs_node_replication_backlog_total`.
- Validez le pipeline de verification des preuves (`crates/sorafs_node/src/potr.rs`) pour les déploiements récents.
- Compare las versiones de firmware de los proveedores con el registro de los operadores.

**Opciones de reparación**

- Déclenchez des replays PoR via `sorafs_cli proof stream` avec le dernier manifest.
- Si les preuves échouent de manière cohérente, retirez le fournisseur de l’ensemble actif en mettant à jour le registre de gouvernance et forçant una update des scoreboards de l’orchestrateur.

**Después del incidente**

- Lanza el escenario de perforación del caos PoR antes del despliegue de la cadena en producción.
- Consignez les enseignements dans le template de post mortem et mettez à jour la checklist de rating des fournisseurs.

## Retardado de réplica / croissance du backlog

**Detección**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importar
  `dashboards/alerts/sorafs_capacity_rules.yml` y ejecutar
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  Promoción anticipada para que Alertmanager reflète les seuils documentés.
- Tablero: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Acciones inmediatas**

1. Verifique la puerta del backlog (proveedor único o flotte) y coloque en pausa las tareas de réplica no esenciales.
2. Si el atraso está aislado, afectará temporalmente los nuevos comandos a los proveedores alternativos a través del programador de réplica.**Triaje**

- Inspeccione la télémétrie Orchestrateur pour des rafales de reintries qui pourraient faire exploser le backlog.
- Confirme que les cibles de stockage ont suffisamment de headroom (`sorafs_node_capacity_utilisation_percent`).
- Revue los cambios recientes de configuración (puesta al día del perfil del fragmento, cadencia de pruebas).

**Opciones de reparación**

- Ejecute `sorafs_cli` con la opción `--rebalance` para redistribuir el contenido.
- Escala horizontalmente los trabajadores de réplica para el proveedor impactado.
- Déclenchez una actualización de los manifiestos para realinear las ventanas TTL.

**Después del incidente**

- Planifique un taladro de capacidad activando los controles de saturación proporcionados.
- Mettez à jour la documentación SLA de réplica en `docs/source/sorafs_node_client_protocol.md`.

## Cadencia de ejercicios de caos

- **Trimestriel**: simulación combinada de panel gateway + tempête de reintentos orquestador.
- **Semestriel**: inyección de cheques PoR/PoTR en dos proveedores con recuperación.
- **Spot-check mensuel**: escenario de retardo de réplica con manifiestos de puesta en escena.
- Siga los ejercicios en el registro de runbook compartido (`ops/drill-log.md`) a través de:

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

- Validez le log avant les commits avec :

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- Utilice `--status scheduled` para los ejercicios futuros, `pass`/`fail` para las ejecuciones finales y `follow-up` cuando las acciones restantes estén abiertas.
- Reemplace el destino con `--log` para los ensayos en seco o la verificación automática; Sin eso, el script continúa con el ritmo del día `ops/drill-log.md`.

## Plantilla de autopsia

Utilice `docs/source/sorafs/postmortem_template.md` para cada incidente P1/P2 y para las retrospectivas de ejercicios de caos. La plantilla cubre la cronología, la cuantificación del impacto, los factores contribuyentes, las acciones correctivas y las tareas de verificación de seguimiento.