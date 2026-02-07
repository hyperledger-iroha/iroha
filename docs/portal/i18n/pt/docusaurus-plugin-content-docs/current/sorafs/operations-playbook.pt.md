---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operações
título: Manual de operações da SoraFS
sidebar_label: Manual de operações
description: Guias de resposta a incidentes e procedimentos de exercícios de caos para operadores da SoraFS.
---

:::nota Fonte canônica
Esta página reflete o runbook mantido em `docs/source/sorafs_ops_playbook.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx esteja totalmente migrado.
:::

## Referências chave

- Ativos de observabilidade: consultar dashboards Grafana em `dashboards/grafana/` e regras de alerta Prometheus em `dashboards/alerts/`.
- Catálogo de métricas: `docs/source/sorafs_observability_plan.md`.
- Superfícies de telemetria do orquestrador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de escalada

| Prioridade | Exemplos de gatilho | Primário de plantão | Backup | Notas |
|-----------|-----------|------------------|--------|-------|
| P1 | Queda global do gateway, taxa de falha PoR > 5% (15 min), backlog de replicação dobrando a cada 10 min | Armazenamento SRE | Observabilidade TL | Ação ou conselho de governança se o impacto ultrapassar 30 min. |
| P2 | Violação de SLO de latência regional do gateway, pico de tentativas do orquestrador sem impacto de SLA | Observabilidade TL | Armazenamento SRE | Continue o lançamento, mas bloqueie novos manifestos. |
| P3 | Alertas não críticos (estagnação de manifestos, capacidade 80-90%) | Triagem de admissão | Guilda de operações | Resolver não próximo ao utilitário. |

## Queda do gateway / disponibilidade degradada

**Detecção**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Painel: `dashboards/grafana/sorafs_gateway_overview.json`.

**Ações imediatas**

1. Confirme o escopo (provedor único vs frota) via painel de taxas de requisições.
2. Troque o roteamento do Torii para provedores saudáveis ​​(se multi-provedor) ajustando `sorafs_gateway_route_weights` na configuração ops (`docs/source/sorafs_gateway_self_cert.md`).
3. Se todos os provedores forem impactados, habilite o fallback de \"direct fetch\" para clientes CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triagem**

- Verifique a utilização de stream tokens contra `sorafs_gateway_stream_token_limit`.
- Inspeção de logs do gateway em busca de erros TLS ou de admissão.
- Execute `scripts/telemetry/run_schema_diff.sh` para garantir que o esquema exportado pelo gateway corresponda à versão esperada.

**Opções de remediação**

- Reinicie apenas o processo de gateway afetado; evite reciclar todo o cluster, a menos que vários fornecedores falhem.
- Aumente temporariamente o limite de stream tokens em 10-15% se a saturação for confirmada.
- Reexecutar o self-cert (`scripts/sorafs_gateway_self_cert.sh`) após estabilização.

**Pós-incidente**

- Registre um postmortem P1 usando `docs/source/sorafs/postmortem_template.md`.
- Agende um exercício de caos de envio se a remediação depender de intervenções manuais.

## Pico de falhas de prova (PoR / PoTR)

**Detecção**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Painel: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetria: `torii_sorafs_proof_stream_events_total` e eventos `sorafs.fetch.error` com `provider_reason=corrupt_proof`.

**Ações imediatas**1. Congele novas admissões de manifestos sinalizando o registro de manifestos (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifique a Governança para pausar incentivos aos fornecedores afetados.

**Triagem**

- Verifique a profundidade da fila de desafios PoR contra `sorafs_node_replication_backlog_total`.
- Valide o pipeline de verificação de provas (`crates/sorafs_node/src/potr.rs`) em implantações recentes.
- Compare versões de firmware dos provedores com o registro de operadoras.

**Opções de remediação**

- Dispare replays PoR usando `sorafs_cli proof stream` com o manifesto mais recente.
- Se as provas falharem de forma consistente, remova o provedor do conjunto ativo atualizando o registro de governança e forçando a atualização dos placares do orquestrador.

**Pós-incidente**

- Execute o cenário de perfuração de caos PoR antes da próxima implantação em produção.
- Cadastre aprendizados no template de postmortem e atualize o checklist de qualificação de provedores.

## Atraso de replicação / crescimento do backlog

**Detecção**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importar
  `dashboards/alerts/sorafs_capacity_rules.yml` e executar
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  antes da promoção para que o Alertmanager reflita os limites documentados.
- Painel: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Ações imediatas**

1. Verifique o escopo do backlog (provedor único ou frota) e pause tarefas de replicação não essenciais.
2. Se o backlog for isolado, reatribua temporariamente novos pedidos a provedores alternativos via o agendador de replicação.

**Triagem**

- Inspecione a telemetria do orquestrador em busca de rajadas de novas tentativas que possam ampliar o backlog.
- Confirme que os alvos de armazenamento têm espaço suficiente (`sorafs_node_capacity_utilisation_percent`).
- Revisar mudanças recentes de configuração (atualizações de perfil de chunk, cadência de provas).

**Opções de remediação**

- Execute `sorafs_cli` com a opção `--rebalance` para redistribuir conteúdo.
- Escale horizontalmente os trabalhadores de replicação para o provedor impactado.
- Dispare atualização de manifesto para realinhar janelas TTL.

**Pós-incidente**

- Agende um exercício de capacidade focado em falha de saturação de provedores.
- Atualizar uma documentação de SLA de replicação em `docs/source/sorafs_node_client_protocol.md`.

## Cadência de exercícios de caos

- **Trimestral**: simulação combinada de queda do gateway + tempestade de tentativas do orquestrador.
- **Semestral**: injeção de falhas PoR/PoTR em dois provedores com recuperação.
- **Spot-check mensal**: cenário de atraso de replicação usando manifestos de staging.
- Cadastre os treinos no log compartilhado (`ops/drill-log.md`) via:

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

- Utilize `--status scheduled` para perfurações futuras, `pass`/`fail` para execuções concluídas, e `follow-up` quando houver ações abertas.
- Substitua o destino com `--log` para simulação ou verificação automatizada; sem isso o script continua atualizando `ops/drill-log.md`.

## Modelo de postmortemUse `docs/source/sorafs/postmortem_template.md` para cada incidente P1/P2 e para retrospectivas de exercícios de caos. O modelo cobre linha do tempo, quantificação de impacto, fatores contribuintes, ações corretivas e tarefas de verificação de acompanhamento.