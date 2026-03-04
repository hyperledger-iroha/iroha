---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: daed3ecc3f448cb1049d43ec9ce58bf617fd07333d8d62556728792c617caef9
source_last_modified: "2025-11-14T19:54:50.041958+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: operations-playbook
title: Playbook de operacoes da SoraFS
sidebar_label: Playbook de operacoes
description: Guias de resposta a incidentes e procedimentos de drills de caos para operadores da SoraFS.
---

:::note Fonte canonica
Esta pagina espelha o runbook mantido em `docs/source/sorafs_ops_playbook.md`. Mantenha ambas as copias sincronizadas ate que o conjunto de documentacao Sphinx seja totalmente migrado.
:::

## Referencias chave

- Ativos de observabilidade: consulte dashboards Grafana em `dashboards/grafana/` e regras de alerta Prometheus em `dashboards/alerts/`.
- Catalogo de metricas: `docs/source/sorafs_observability_plan.md`.
- Superficies de telemetria do orquestrador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de escalonamento

| Prioridade | Exemplos de gatilho | On-call primario | Backup | Notas |
|-----------|----------------------|------------------|--------|-------|
| P1 | Queda global do gateway, taxa de falha PoR > 5% (15 min), backlog de replicacao dobrando a cada 10 min | Storage SRE | Observability TL | Acione o conselho de governanca se o impacto ultrapassar 30 min. |
| P2 | Violacao de SLO de latencia regional do gateway, pico de retries do orquestrador sem impacto de SLA | Observability TL | Storage SRE | Continue o rollout, mas bloqueie novos manifests. |
| P3 | Alertas nao criticos (staleness de manifests, capacidade 80-90%) | Intake triage | Ops guild | Resolver no proximo dia util. |

## Queda do gateway / disponibilidade degradada

**Deteccao**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Dashboard: `dashboards/grafana/sorafs_gateway_overview.json`.

**Acoes imediatas**

1. Confirme o escopo (provedor unico vs frota) via painel de taxa de requisicoes.
2. Troque o roteamento do Torii para provedores saudaveis (se multi-provedor) ajustando `sorafs_gateway_route_weights` na configuracao ops (`docs/source/sorafs_gateway_self_cert.md`).
3. Se todos os provedores estiverem impactados, habilite o fallback de \"direct fetch\" para clientes CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- Verifique a utilizacao de stream tokens contra `sorafs_gateway_stream_token_limit`.
- Inspecione logs do gateway em busca de erros TLS ou de admissao.
- Execute `scripts/telemetry/run_schema_diff.sh` para garantir que o schema exportado pelo gateway corresponde a versao esperada.

**Opcoes de remediacao**

- Reinicie apenas o processo de gateway afetado; evite reciclar todo o cluster, a menos que varios provedores falhem.
- Aumente temporariamente o limite de stream tokens em 10-15% se a saturacao for confirmada.
- Reexecute o self-cert (`scripts/sorafs_gateway_self_cert.sh`) apos estabilizacao.

**Pos-incidente**

- Registre um postmortem P1 usando `docs/source/sorafs/postmortem_template.md`.
- Agende um drill de caos de acompanhamento se a remediacao depender de intervencoes manuais.

## Pico de falhas de prova (PoR / PoTR)

**Deteccao**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Dashboard: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetria: `torii_sorafs_proof_stream_events_total` e eventos `sorafs.fetch.error` com `provider_reason=corrupt_proof`.

**Acoes imediatas**

1. Congele novas admissoes de manifests sinalizando o registro de manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifique a Governance para pausar incentivos aos provedores afetados.

**Triage**

- Verifique a profundidade da fila de desafios PoR contra `sorafs_node_replication_backlog_total`.
- Valide o pipeline de verificacao de provas (`crates/sorafs_node/src/potr.rs`) em deployments recentes.
- Compare versoes de firmware dos provedores com o registro de operadores.

**Opcoes de remediacao**

- Dispare replays PoR usando `sorafs_cli proof stream` com o manifest mais recente.
- Se as provas falharem de forma consistente, remova o provedor do conjunto ativo atualizando o registro de governanca e forcando o refresh dos scoreboards do orquestrador.

**Pos-incidente**

- Execute o cenario de drill de caos PoR antes do proximo deploy em producao.
- Registre aprendizados no template de postmortem e atualize o checklist de qualificacao de provedores.

## Atraso de replicacao / crescimento do backlog

**Deteccao**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importe
  `dashboards/alerts/sorafs_capacity_rules.yml` e execute
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  antes da promocao para que o Alertmanager reflita os thresholds documentados.
- Dashboard: `dashboards/grafana/sorafs_capacity_health.json`.
- Metricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Acoes imediatas**

1. Verifique o escopo do backlog (provedor unico ou frota) e pause tarefas de replicacao nao essenciais.
2. Se o backlog for isolado, reatribua temporariamente novos pedidos a provedores alternativos via o scheduler de replicacao.

**Triage**

- Inspecione a telemetria do orquestrador em busca de bursts de retries que possam ampliar o backlog.
- Confirme que os targets de armazenamento tem headroom suficiente (`sorafs_node_capacity_utilisation_percent`).
- Revise mudancas recentes de configuracao (atualizacoes de chunk profile, cadencia de proofs).

**Opcoes de remediacao**

- Execute `sorafs_cli` com a opcao `--rebalance` para redistribuir conteudo.
- Escale horizontalmente os workers de replicacao para o provedor impactado.
- Dispare manifest refresh para realinhar janelas TTL.

**Pos-incidente**

- Agende um drill de capacidade focado em falha de saturacao de provedores.
- Atualize a documentacao de SLA de replicacao em `docs/source/sorafs_node_client_protocol.md`.

## Cadencia de drills de caos

- **Trimestral**: simulacao combinada de queda do gateway + tempestade de retries do orquestrador.
- **Semestral**: injecao de falhas PoR/PoTR em dois provedores com recuperacao.
- **Spot-check mensal**: cenario de atraso de replicacao usando manifests de staging.
- Registre os drills no log compartilhado (`ops/drill-log.md`) via:

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

- Valide o log antes de commits com:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Use `--status scheduled` para drills futuros, `pass`/`fail` para execucoes concluidas, e `follow-up` quando houver acoes abertas.
- Substitua o destino com `--log` para dry-runs ou verificacao automatizada; sem isso o script continua atualizando `ops/drill-log.md`.

## Template de postmortem

Use `docs/source/sorafs/postmortem_template.md` para cada incidente P1/P2 e para retrospectivas de drills de caos. O template cobre linha do tempo, quantificacao de impacto, fatores contribuintes, acoes corretivas e tarefas de verificacao de acompanhamento.
