---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operaciones
título: Playbook de operacoes da SoraFS
sidebar_label: Manual de operaciones
descripción: Guías de respuesta a incidentes y procedimientos de simulacros de caos para operadores da SoraFS.
---

:::nota Fuente canónica
Esta página espelha o runbook mantido en `docs/source/sorafs_ops_playbook.md`. Mantenha ambas as copias sincronizadas ate que o conjunto de documentacao Sphinx seja totalmente migrado.
:::

## Referencias chave

- Activos de observabilidade: consulte los paneles Grafana en `dashboards/grafana/` y registros de alerta Prometheus en `dashboards/alerts/`.
- Catálogo de métricas: `docs/source/sorafs_observability_plan.md`.
- Superficies de telemetría del orquestador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de escalamiento

| Prioridad | Ejemplos de gatilho | Primario de guardia | Copia de seguridad | Notas |
|-----------|----------------------|------------------|--------|-------|
| P1 | Queda global do gateway, taxa de falha PoR > 5% (15 min), backlog de replicacao dobrando a cada 10 min | Almacenamiento SRE | Observabilidad TL | Acción del consejo de gobierno se o impacto ultrapasar 30 min. |
| P2 | Violacao de SLO de latencia regional do gateway, pico de reintentos do orquestador sin impacto de SLA | Observabilidad TL | Almacenamiento SRE | Continúe con el lanzamiento, se manifiestan más bloqueos nuevos. |
| P3 | Alertas nao criticos (estancamiento de manifiestos, capacidade 80-90%) | Triaje de admisión | Gremio de operaciones | Resolver no proximo dia util. |## Queda do gateway / disponibilidade degradada

**Detectado**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tablero: `dashboards/grafana/sorafs_gateway_overview.json`.

**Acoes inmediatamente**

1. Confirme o escopo (provedor único vs frota) vía Painel de taxa de requisicoes.
2. Troque o roteamento do Torii para provedores saudaveis (se multi-provedor) ajustando `sorafs_gateway_route_weights` na configuracao ops (`docs/source/sorafs_gateway_self_cert.md`).
3. Si todos los proveedores estiverem impactados, habilite o fallback de \"direct fetch\" para clientes CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triaje**

- Verifique la utilización de tokens de flujo contra `sorafs_gateway_stream_token_limit`.
- Inspeccione los registros de la puerta de enlace en busca de errores TLS o de admisión.
- Ejecute `scripts/telemetry/run_schema_diff.sh` para garantizar que el esquema exportado por la puerta de enlace corresponde a la inversa esperada.

**Opciones de remediación**

- Reinicia apenas el proceso de gateway afetado; Evite reciclar todo el cluster, a menos que varios proveedores falhem.
- Aumente temporalmente el límite de tokens de transmisión en 10-15% se a saturación para confirmación.
- Vuelva a ejecutar la autocertificación (`scripts/sorafs_gateway_self_cert.sh`) después de estabilización.

**Pos-incidente**

- Registre um postmortem P1 usando `docs/source/sorafs/postmortem_template.md`.
- Agenda para un simulacro de caos de acompañamiento se a remediacao dependiendo de las intervenciones manuales.

## Pico de falhas de prueba (PoR / PoTR)

**Detectado**- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tablero: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetría: `torii_sorafs_proof_stream_events_total` y eventos `sorafs.fetch.error` con `provider_reason=corrupt_proof`.

**Acoes inmediatamente**

1. Congele novas admissoes de manifests sinalizando o registro de manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifique a Governance para pausar incentivos aos provedores afetados.

**Triaje**

- Verifique a profundidade da fila de desafios PoR contra `sorafs_node_replication_backlog_total`.
- Valide o pipeline de verificación de provas (`crates/sorafs_node/src/potr.rs`) en implementaciones recientes.
- Compare los versículos del firmware de los proveedores con el registro de operadores.

**Opciones de remediación**

- Dispare repeticiones PoR usando `sorafs_cli proof stream` con el manifiesto más reciente.
- Se as provas falharem de forma consistente, remova o provedor do conjunto ativo atualizando o registro de gobernadora e forcando o actualizar dos marcadores do orquestrador.

**Pos-incidente**

- Ejecutar el escenario de simulacro de caos PoR antes del próximo despliegue en producción.
- Registre aprendizados no template de post mortem y atualize o checklist de qualificacao de provedores.

## Atraso de replicacao / crescimento do backlog

**Detectado**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. importar
  `dashboards/alerts/sorafs_capacity_rules.yml` y ejecutar
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  Antes de la promoción para que Alertmanager reflita los umbrales documentados.
- Tablero: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Acoes inmediatamente**1. Verifique o escopo do backlog (provedor unico ou frota) e pausa tarefas de replicacao nao essenciais.
2. Si el backlog está aislado, reatribuya temporalmente nuevos pedidos a proveedores alternativos a través del planificador de réplicas.

**Triaje**

- Inspección de telemetría del orquestador en busca de ráfagas de reintentos que puedan ampliar el trabajo pendiente.
- Confirme que os objetivos de armazenamento tem headroom suficiente (`sorafs_node_capacity_utilisation_percent`).
- Revisar mudancas recientes de configuración (atualizacoes de chunk perfil, cadencia de pruebas).

**Opciones de remediación**

- Ejecute `sorafs_cli` con opcao `--rebalance` para redistribuir contenido.
- Escalar horizontalmente los trabajadores de replicación para el proveedor impactado.
- Dispare la actualización del manifiesto para realinhar janelas TTL.

**Pos-incidente**

- Agenda para un taladro de capacidad enfocado en falta de saturación de proveedores.
- Actualizar una documentación de SLA de replicación en `docs/source/sorafs_node_client_protocol.md`.

## Cadencia de ejercicios de caos

- **Trimestral**: simulacao combinado de queda do gateway + tempestade de retries do orquestrador.
- **Semestral**: injecao de falhas PoR/PoTR em dois provedores com recuperacao.
- **Spot-check mensal**: escenario de atraso de replicacao usando manifiestos de puesta en escena.
- Registre los ejercicios sin registro compartilhado (`ops/drill-log.md`) a través de:

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

- Validar el registro antes de commits con:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- Utilice `--status scheduled` para ejercicios futuros, `pass`/`fail` para ejecuciones concluidas, e `follow-up` cuando desee realizar operaciones cerradas.
- Sustituya el destino con `--log` para ensayos en seco o verificación automática; Sin embargo, el script continúa actualizando `ops/drill-log.md`.

## Plantilla de autopsia

Use `docs/source/sorafs/postmortem_template.md` para cada incidente P1/P2 y para retrospectivas de simulacros de caos. O template cobre linha do tempo, quantificacao de impacto, fatores contribuintes, acoes corretivas e tarefas de verificacao de acompanhamento.