---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operações
título: Manual de operações de SoraFS
sidebar_label: Manual de operações
description: Guias de resposta a incidentes e procedimentos de exercícios de caos para operadores de SoraFS.
---

:::nota Fonte canônica
Esta página reflete o runbook mantido em `docs/source/sorafs_ops_playbook.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx migre por completo.
:::

## Referências clave

- Ativos de observação: consultar os dashboards de Grafana em `dashboards/grafana/` e as regras de alerta de Prometheus em `dashboards/alerts/`.
- Catálogo de estatísticas: `docs/source/sorafs_observability_plan.md`.
- Superfícies de telemetria do orquestrador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de escalação

| Prioridade | Exemplos de disparo | Diretor de plantão | Backup | Notas |
|----------|---------------------|------------------|--------|-------|
| P1 | Caída global do gateway, taxa de falhas PoR > 5% (15 min), backlog de replicação duplicada a cada 10 min | Armazenamento SRE | Observabilidade TL | Involucra al consejo de gobernanza se o impacto for superior a 30 min. |
| P2 | Cumprimento de SLO de latência regional do gateway, pico de reintenções do orquestrador sem impacto de SLA | Observabilidade TL | Armazenamento SRE | Continue o lançamento, mas bloqueie novos manifestos. |
| P3 | Alertas no críticas (estado de manifestos, capacidade 80–90%) | Triagem de ingestão | Guilda de operações | Resolver no próximo dia hábil. |

## Caída do gateway / disponibilidade degradada

**Detecção**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Painel: `dashboards/grafana/sorafs_gateway_overview.json`.

**Ações imediatas**

1. Confirme o escopo (provedor único vs flota) por meio do painel de solicitação de solicitação.
2. Altere a inicialização de Torii para fornecedores sanos (se for multiprovedor) ativando `sorafs_gateway_route_weights` na configuração de operações (`docs/source/sorafs_gateway_self_cert.md`).
3. Se todos os provedores estiverem afetados, habilite o substituto de “busca direta” para clientes CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triagem**

- Revise a utilização do token de stream antes de `sorafs_gateway_stream_token_limit`.
- Inspecione logs do gateway em busca de erros de TLS ou de admissão.
- Execute `scripts/telemetry/run_schema_diff.sh` para garantir que o esquema exportado pelo gateway coincide com a versão esperada.

**Opções de remediação**

- Reiniciar apenas o processo de gateway afectado; evita todo o conjunto salvo que recicle vários fornecedores.
- Aumenta o limite de tokens de stream em 10–15% de forma temporal se for confirmada a saturação.
- Execute a autocertificação (`scripts/sorafs_gateway_self_cert.sh`) após a estabilização.

**Pós-incidente**

- Registre um postmortem P1 usando `docs/source/sorafs/postmortem_template.md`.
- Programar um exercício de monitoramento de caos se a remediação exigir intervenções manuais.

## Pico de falhas de testes (PoR / PoTR)

**Detecção**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Painel: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetria: `torii_sorafs_proof_stream_events_total` e eventos `sorafs.fetch.error` com `provider_reason=corrupt_proof`.

**Ações imediatas**1. Congela novas admissões de manifestos marcando o registro de manifestos (`docs/source/sorafs/manifest_pipeline.md`).
2. Notificar a Governança para suspender incentivos dos fornecedores afetados.

**Triagem**

- Revisa a profundidade da cola de desafíos PoR frente a `sorafs_node_replication_backlog_total`.
- Valide o pipeline de verificação de testes (`crates/sorafs_node/src/potr.rs`) para aplicações recentes.
- Compare versões de firmware de provedores com o registro de operadoras.

**Opções de remediação**

- Dispare replays de PoR usando `sorafs_cli proof stream` com o manifesto mais recente.
- Se as tentativas falharem consistentemente, elimine o provedor do conjunto ativo atualizando o registro de governo e forçando o refresco de placares do orquestrador.

**Pós-incidente**

- Execute o cenário de perfuração de caos PoR antes de iniciar a produção.
- Captura aprendizagens na planta de postmortem e atualização da lista de verificação de qualificação de fornecedores.

## Retrocesso de replicação/crescimento do backlog

**Detecção**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importação
  `dashboards/alerts/sorafs_capacity_rules.yml` e ejetado
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  antes da promoção para que o Alertmanager reflita os umbrais documentados.
- Painel: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Ações imediatas**

1. Verifique o alcance do backlog (provedor único ou flota) e pause tarefas de replicação não essenciais.
2. Se o backlog estiver isolado, reatribua temporariamente novos pedidos a fornecedores alternativos por meio do agendador de replicação.

**Triagem**

- Inspecione a telemetria do orquestrador por ráfagas de reintenções que podem aumentar o backlog.
- Confirme que os alvos de armazenamento têm espaço suficiente (`sorafs_node_capacity_utilisation_percent`).
- Revisa mudanças recentes de configuração (atualizações de perfis de pedaço, cadência de provas).

**Opções de remediação**

- Execute `sorafs_cli` com a opção `--rebalance` para redistribuir o conteúdo.
- Escalar horizontalmente os trabalhadores de replicação para o fornecedor afetado.
- Dispara uma atualização de manifestos para realinhar as janelas TTL.

**Pós-incidente**

- Programa uma broca de capacidade focada em falhas por saturação de fornecedores.
- Atualize a documentação do SLA de replicação em `docs/source/sorafs_node_client_protocol.md`.

## Cadência de exercícios de caos

- **Trimestral**: simulação combinada de queda de gateway + tormenta de reintenções do orquestrador.
- **Semestral**: injeção de falhas PoR/PoTR em provedores com recuperação.
- **Cheque mensal**: cenário de retrocesso de replicação usando manifestos de teste.
- Registre os exercícios no log compartilhado (`ops/drill-log.md`) via:

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

- Valide o log antes dos commits com:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Usa `--status scheduled` para brocas futuras, `pass`/`fail` para ejecuciones completas, e `follow-up` quando queden acciones abertas.
- Escreva o destino com `--log` para simulações ou verificação automatizada; sin eso el script segue atualizando `ops/drill-log.md`.## Plantilla postmortem

Usa `docs/source/sorafs/postmortem_template.md` para cada incidente P1/P2 e para retrospectivas de exercícios de caos. A planta cobre cronologia, quantificação de impacto, fatores contribuintes, ações corretivas e tarefas de verificação de acompanhamento.