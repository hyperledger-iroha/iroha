---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operações
título: Manual de exploração SoraFS
sidebar_label: Manual de exploração
descrição: Guias de resposta a incidentes e procedimentos de exercícios de caos para operadores SoraFS.
---

:::nota Fonte canônica
Esta página reflete o runbook mantido em `docs/source/sorafs_ops_playbook.md`. Garanta as duas cópias sincronizadas até que a documentação do Sphinx esteja totalmente migrada.
:::

## Referências Clés

- Ativos de observação: consulte os painéis Grafana sob `dashboards/grafana/` e as regras de alerta Prometheus em `dashboards/alerts/`.
- Catálogo de métricas: `docs/source/sorafs_observability_plan.md`.
- Superfícies de télémétrie do orquestrador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de Escalada

| Prioridade | Exemplos de encerramento | Diretor de plantão | Backup | Notas |
|----------|---------------------------|------------------|--------|-------|
| P1 | Gateway global, taux d’échec PoR > 5% (15 min), backlog de replicação duplicado durante todos os 10 min | Armazenamento SRE | Observabilidade TL | Envolva o conselho de governança se o impacto ultrapassar 30 min. |
| P2 | Violação do SLO do gateway de latência regional, foto de novas tentativas do orquestrador sem impacto do SLA | Observabilidade TL | Armazenamento SRE | Continue a implementação mais bloqueando os novos manifestos. |
| P3 | Alertas não críticos (desatualização dos manifestos, capacidade 80–90%) | Triagem de admissão | Guilda de operações | À traiter dans le prochain jour ouvré. |

## Panne gateway / disponibilidade degradada

**Detecção**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Painel: `dashboards/grafana/sorafs_gateway_overview.json`.

**Ações imediatas**

1. Confirme a porta (fornecedor exclusivo vs flotte) através do painel de taxas de requêtes.
2. Bascule o roteamento Torii para os fornecedores sagrados (se multi-fornecedores) e bascule `sorafs_gateway_route_weights` nas operações de configuração (`docs/source/sorafs_gateway_self_cert.md`).
3. Se todos os fornecedores forem afetados, ative o substituto “busca direta” para os clientes CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triagem**

- Verifique a utilização de tokens de stream relacionados a `sorafs_gateway_stream_token_limit`.
- Inspecione os logs do gateway para erros de TLS ou de admissão.
- Execute `scripts/telemetry/run_schema_diff.sh` para verificar se o esquema exportado pelo gateway corresponde à versão atendida.

**Opções de remediação**

- Reiniciar exclusivamente o processo de gateway afetado; evite reciclar todos os clusters seguros, se houver mais fornecedores disponíveis.
- Aumente temporariamente o limite de tokens de fluxo de 10–15% se uma saturação for confirmada.
- Relance o self-cert (`scripts/sorafs_gateway_self_cert.sh`) após a estabilização.

**Pós-incidente**

- Grave um post-mortem P1 com `docs/source/sorafs/postmortem_template.md`.
- Planeje uma broca de caos de suivi se a remediação exigir intervenções manuais.

## Pic d’échecs de preuve (PoR / PoTR)

**Detecção**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Painel: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetria: `torii_sorafs_proof_stream_events_total` e eventos `sorafs.fetch.error` com `provider_reason=corrupt_proof`.

**Ações imediatas**1. Visualize as novas admissões de manifestos em quantidade no registro de manifestos (`docs/source/sorafs/manifest_pipeline.md`).
2. Notificar o governo para suspender as incitações aos fornecedores impactados.

**Triagem**

- Verifique o profundor do arquivo de desafios enfrentados pelo PoR em `sorafs_node_replication_backlog_total`.
- Valide o pipeline de verificação de testes anteriores (`crates/sorafs_node/src/potr.rs`) para implantações recentes.
- Compare as versões de firmware dos fornecedores com o registro dos operadores.

**Opções de remediação**

- Desligue os replays PoR via `sorafs_cli proof stream` com o manifesto anterior.
- Se as previsões forem consistentes, retire o fornecedor do conjunto ativo, mantendo no dia o registro de governo e forçando uma atualização dos placares do orquestrador.

**Pós-incidente**

- Lance o cenário de perfuração do caos PoR antes da próxima implantação na produção.
- Envie os ensinamentos para o modelo de postmortem e coloque no dia a lista de verificação de qualificação dos fornecedores.

## Retardo de replicação / croissance du backlog

**Detecção**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importar
  `dashboards/alerts/sorafs_capacity_rules.yml` e executado
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  promoção antecipada para que o Alertmanager reflita seus documentos.
- Painel: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Ações imediatas**

1. Verifique a porta do backlog (fornecedor exclusivo ou flotte) e coloque em pausa as etapas de replicação não essenciais.
2. Se o backlog estiver isolado, ative temporariamente os novos comandos nas fontes alternativas por meio do agendador de replicação.

**Triagem**

- Inspecione o orquestrador de telefonia para os rafales de novas tentativas que podem explodir o backlog.
- Confirme se as opções de armazenamento são suficientes para o headroom (`sorafs_node_capacity_utilisation_percent`).
- Passe em revista as alterações recentes de configuração (mises à jour de chunk profile, cadência de provas).

**Opções de remediação**

- Execute `sorafs_cli` com a opção `--rebalance` para redistribuir o conteúdo.
- Escala horizontalmente os trabalhadores de replicação para o fornecimento impactado.
- Desative uma atualização dos manifestos para realinhar as janelas TTL.

**Pós-incidente**

- Planeje uma broca de capacidade para fornecer os níveis de saturação fornecidos.
- Atualize a documentação SLA de replicação em `docs/source/sorafs_node_client_protocol.md`.

## Cadence des drills de caos

- **Trimestriel**: simulação combinada de gateway de painel + orquestrador de tentativas de repetição.
- **Semestriel**: injeção de cheques PoR/PoTR em dois fornecedores com recuperação.
- **Mensual de verificação pontual**: cenário de retardo de replicação com manifestos de encenação.
- Execute os exercícios no log do runbook compartilhado (`ops/drill-log.md`) via:

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
  ```- Utilize `--status scheduled` para as brocas à frente, `pass`/`fail` para as operações finais e `follow-up` quando as ações restantes forem abertas.
- Substitua o destino com `--log` para testes ou verificação automática; sem cela, o script continua atualizado `ops/drill-log.md`.

## Modelo de postmortem

Use `docs/source/sorafs/postmortem_template.md` para cada incidente P1/P2 e para as retrospectivas de exercícios de caos. O modelo cobre a cronologia, a quantificação do impacto, os fatores contribuintes, as ações corretivas e as taxas de verificação de acompanhamento.