---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas de transição do nexo
título: Notas de transição de Nexus
descrição: Espelho de `docs/source/nexus_transition_notes.md`, cobrindo as previsões de transição da Fase B, o calendário de auditoria e as mitigações.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transição de Nexus

Este diário atende ao trabalho restante de **Fase B - Nexus Transition Foundations** apenas no final da lista de verificação de lançamento multi-lane. Ele completou as entradas de marcos em `roadmap.md` e guardou as referências anteriores de B1-B4 em um único local para o governo, SRE e os leads SDK que compartilham a fonte de verdade do meme.

## Porta e cadência

- Couvre les audits routed-trace et les guardrails de telemetrie (B1/B2), l'ensemble de deltas de configuration approuve par la gouvernance (B3) et les suivis de repetition de lancement multi-lane (B4).
- Substitua a nota temporária de cadência que você vive aqui; Após a auditoria do primeiro trimestre de 2026, o relatório detalhado reside em `docs/source/nexus_routed_trace_audit_report_2026q1.md`, e esta página mantém o calendário atualizado e o registro das mitigações.
- Coloque as tabelas no dia após cada fenetre routed-trace, vote de gouvernance ou repetição de lancement. Quando os artefatos são vivos, reflete a nova localização nesta página para que os documentos disponíveis (status, painéis, portais SDK) possam se tornar uma estabilidade estável.

## Instantâneo das prévias (2026 Q1-Q2)

| Fluxo de trabalho | Preúves | Proprietário(s) | Estatuto | Notas |
|------------|----------|----------|--------|-------|
| **B1 – Auditorias de rastreamento roteado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetria-ops, @governança | Concluído (1º trimestre de 2026) | Três janelas de auditoria registradas; O TLS retardado de `TRACE-CONFIG-DELTA` está fechado durante a repetição do Q2. |
| **B2 - Remediação de telemetria e guarda-corpos** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetria-ops | Completo | Pacote de alerta, política de comparação de bot e cauda de lote OTLP (`nexus.scheduler.headroom` log + painel Grafana de headroom) gratuitos; aucun renúncia aberta. |
| **B3 - Aprovações de deltas de configuração** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Vote GOV-2026-03-19 enregistre; le bundle signe alimente le pack de telemetrie cite plus bas. |
| **B4 - Repetição de lançamento multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Concluído (2º trimestre de 2026) | A reexecução canária de Q2 termina com a mitigação do retardo TLS; o manifesto do validador + `.sha256` captura a placa de slots 912-936, a semente da carga de trabalho `NEXUS-REH-2026Q2` e o hash do perfil TLS registrado durante a nova execução. |

## Calendário trimestral de auditorias de rastreamento roteado

| ID de rastreamento | Fenetre (UTC) | Resultado | Notas |
|----------|-------------|---------|-------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Reussi | Queue-admission P95 est reste bien en dessous de la cible <=750 ms. Aucune ação necessária. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Reussi | Os hashes de repetição OTLP anexa um `status.md`; a parte do diff bot SDK confirmou o desvio zero. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Resolução | O atraso do TLS foi encerrado durante a repetição do Q2; o pacote de telemetria para `NEXUS-REH-2026Q2` registra o hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (veja `artifacts/nexus/tls_profile_rollout_2026q2/`) e zero retardatários. |
| `TRACE-MULTILANE-CANARY` | 05/05/2026 09:12-10:14 | Reussi | Semente de carga de trabalho `NEXUS-REH-2026Q2`; pacote de telemetria + manifesto/resumo em `artifacts/nexus/rehearsals/2026q1/` (intervalo de slots 912-936) com agenda em `artifacts/nexus/rehearsals/2026q2/`. |

Os trimestres futuros devem adicionar novas linhas e substituir as entradas terminadas por um anexo quando a mesa passa do trimestre atual. Consulte esta seção a partir dos relatórios de rastreamento roteado ou dos minutos de governo usando o anel `#quarterly-routed-trace-audit-schedule`.

## Mitigações e itens do backlog| Artigo | Descrição | Proprietário | Cível | Estatuto/Notas |
|------|------------|-------|--------|----------------|
| `NEXUS-421` | Termine a propagação do perfil TLS com o preço do retardo pendente `TRACE-CONFIG-DELTA`, capture as previsões da reexecução e feche o registro de mitigação. | @release-eng, @sre-core | Fenetre Routed-Trace Q2 2026 | Clos - hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado em `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; le reexecutado para confirmar zero retardatários. |
| Preparação `TRACE-MULTILANE-CANARY` | Programe a repetição Q2, junte os equipamentos ao pacote de telemetria e certifique-se de que os chicotes do SDK reutilizem o auxiliar válido. | @telemetry-ops, Programa SDK | Apelação de planejamento 2026-04-30 | Completo - agenda armazenada em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com slot/carga de trabalho de metadados; reutilização do arnês anotado no rastreador. |
| Rotação de resumo do pacote de telemetria | Execute `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada repetição/liberação e registre os resumos na parte inferior do rastreador de configuração delta. | @telemetria-ops | Par candidato a lançamento | Completo - `telemetry_manifest.json` + `.sha256` emis em `artifacts/nexus/rehearsals/2026q1/` (faixa de slots `912-936`, semente `NEXUS-REH-2026Q2`); resume cópias no rastreador e no índice de pré-visualizações. |

## Integração do pacote de configuração delta

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` resta o currículo canônico das diferenças. Quando novos `defaults/nexus/*.toml` ou mudanças de gênese chegam, coloque um dia este rastreador a bordo e depois reflita os pontos aqui.
- Os pacotes de configuração assinados alimentam o pacote de telemetria de repetição. O pacote, válido por `scripts/telemetry/validate_nexus_telemetry_pack.py`, será publicado com as instruções de configuração delta para que os operadores possam recarregar os artefatos exatos usando o pingente B4.
- Os pacotes Iroha 2 permanecem sem pistas: as configurações com `nexus.enabled = false` rejeitam a manutenção das substituições de pista/espaço de dados/roteamento, exceto se o perfil Nexus estiver ativo (`--sora`), então suprima as seções `nexus.*` de modelos de pista única.
- Gardez le log de voto de gouvernance (GOV-2026-03-19) lie depuis le tracker et esta nota para que os futuros votos possam copiar o formato sem devoir redescobrir o rito de aprovação.

## Suítes de repetição de lancemento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura o plano canário, a lista de participantes e as etapas de reversão; atualize o runbook quando a topologia das pistas ou os exportadores de telemetria mudam.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefato verificado durante a repetição de 9 de abril e contém a manutenção das notas/agenda de preparação Q2. Adicione repetições futuras no rastreador de meme em vez de abrir rastreadores ad-hoc para manter os preuves monótonos.
- Publique os trechos do coletor OTLP e as exportações Grafana (veja `docs/source/telemetry.md`) quando as remessas de lote do exportador forem alteradas; la mise a jour Q1 a porte la taille de lot a 256 echantillons para evitar alertas de headroom.
- Os testes CI/testes multi-lane são mantidos em `integration_tests/tests/nexus/multilane_pipeline.rs` e passam pelo fluxo de trabalho `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), substituindo a referência aposentada `pytests/nexus/test_multilane_pipeline.py`; gardez le hash para `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) e sincronizar com o rastreador para rastrear pacotes de repetição.

## Ciclo de vida em tempo de execução das pistas

- Os planos de ciclo de vida das pistas em tempo de execução são válidos, mantendo as ligações do espaço de dados e abortados quando a reconciliação Kura/stockage em paliers echoue, libera a troca de catálogo. Os ajudantes eliminaram os caches dos relés para as pistas retiradas para que o livro-razão de síntese não seja reutilizado nas provas obsoletas.
- Aplique os planos por meio dos auxiliares de configuração/ciclo de vida Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para adicionar/retirar as pistas sem redemarrage; roteamento, instantâneos TEU e registros de manifestos são recarregados automaticamente após um plano reussi.
- Operador de guia: quando um plano é ecoado, verifica os espaços de dados manquants ou raízes de armazenamento impossíveis de criar (raiz fria em camadas/repertórios Kura par lane). Corrija os caminhos de base e reessayez; Os planos reemitem a diferença de faixa de telemetria/espaço de dados para que os painéis reflitam a nova topologia.

## Telemetria NPoS e testes de contrapressãoO retrocesso da repetição do lançamento da Fase B exige capturas de telemetria determinadas, provando que o marca-passo NPoS e os sofás de fofoca permanecem em seus limites de contrapressão. O chicote de integração em `integration_tests/tests/sumeragi_npos_performance.rs` utiliza esses cenários e emite currículos JSON (`sumeragi_baseline_summary::<scenario>::...`) quando novas métricas chegam. Lancez-le localização com:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Defina `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologias e estresses; Os valores por padrão refletem o perfil dos colecionadores 1 s/`k=3` utilizados em B4.

| Cenário/teste | Cobertura | Código de telemetria |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueie 12 rodadas com o tempo de repetição do bloco para registrar os envelopes de latência EMA, os profundores de arquivo e os medidores de envio redundante antes de serializar o pacote de anteriores. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inonde o arquivo de transações para garantir que os diferimentos de admissão sejam determinados pelo fator ativo e que o arquivo exporte os contadores de capacidade/saturação. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Acione o jitter do marca-passo e os tempos limite de vue apenas provam que a faixa +/-125 pontos está aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pousse de gros payloads RBC jusqu'aux limites soft/hard du store pour montrer que les session et compteurs de bytes montent, reculent et se stabilisent sans depasser le store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Force as retransmissões para que os medidores de taxa de envio redundante e os computadores de coletores no alvo avancem, provando que a telemetria exigida pelo retro é ramificada de ponta a ponta. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Deixe os pedaços em intervalos determinados para verificar se os monitores de backlog sinalizam falhas em vez de drenar as cargas úteis. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Conecte as linhas JSON impressas no chicote com a captura Prometheus durante a execução, sempre que o governo exigir precauções para que os alarmes de contrapressão correspondam à topologia de repetição.

## Checklist de mise a jour

1. Adicione novas janelas routed-trace e remova os antigos durante os trimestres do torneio.
2. Adicione a tabela de mitigação depois de cada suivi Alertmanager, meme se a ação consistir em fechar o ticket.
3. Quando a configuração delta for alterada, adicione o rastreador, esta nota e a lista de resumos do pacote de telemetria no meme pull request.
4. Liez aqui todo novo artefato de repetição/telemetria para que os futuros erros do roteiro possam referenciar um único documento em vez de notas ad-hoc dispersas.

## Index des preuves| Ativo | Colocação | Notas |
|-------|----------|-------|
| Relatório de auditoria roteado-traço (1º trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fonte canônica para as previsões Fase B1; espelho para portal sob `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Rastreador de configuração delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contém os currículos das diferenças TRACE-CONFIG-DELTA, as iniciais dos revisores e o registro de votação GOV-2026-03-19. |
| Plano de remediação de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | Documente o pacote de alertas, a cauda do lote OTLP e as grades de proteção do orçamento de exportação estão em B2. |
| Rastreador de repetição multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Listar os artefatos verifica durante a repetição de 9 de abril, validador de manifesto/resumo, notas/agenda Q2 e testes de reversão. |
| Manifesto/resumo do pacote de telemetria (mais recente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+`.sha256`) | Registre a plage 912-936, seed `NEXUS-REH-2026Q2` e os hashes de artefatos para os pacotes de governo. |
| Manifesto do perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+`.sha256`) | Hash do perfil TLS aprovado para captura durante a reexecução Q2; cite-o nos anexos routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planejamento para a repetição Q2 (fenetre, intervalo de slots, semente de carga de trabalho, proprietários de ações). |
| Runbook de repetição de lancemento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operacional para teste -> execução -> reversão; leia o dia quando a topologia das pistas ou os conselhos dos exportadores mudarem. |
| Validador de pacote de telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | Referência CLI par retro B4; arquive os resumos com o rastreador sempre que o pacote for alterado. |
| Regressão multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Válido `nexus.enabled = true` para configurações multi-lane, preserve os hashes do catálogo Sora e forneça os caminhos Kura/merge-log por lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` antes de publicar os resumos dos artefatos. |