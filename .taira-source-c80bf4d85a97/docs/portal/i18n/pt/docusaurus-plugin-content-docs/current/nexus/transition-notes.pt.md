---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/transition-notes.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas de transição do nexo
título: Notas de transição do Nexus
descrição: Espelho de `docs/source/nexus_transition_notes.md`, cobrindo evidências de transição da Fase B, o calendário de auditorias e as mitigações.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transição do Nexus

Este registro acompanha o trabalho pendente de **Fase B - Nexus Transition Foundations** até que a checklist de lançamento multi-lane termine. Ele complementa as entradas de marcos em `roadmap.md` e mantém a evidência referenciada por B1-B4 em um lugar único para que governanca, SRE e líderes de SDK compartilham a mesma fonte de verdade.

## Escopo e cadência

- Cobre os auditorias routed-trace e os guardrails de telemetria (B1/B2), o conjunto de deltas de configuração aprovados por governança (B3) e os acompanhamentos do ensaio de lançamento multi-lane (B4).
- Substituir a nota temporária de cadência que antes vivia aqui; a partir das auditorias do primeiro trimestre de 2026 o relatório detalhado reside em `docs/source/nexus_routed_trace_audit_report_2026q1.md`, enquanto esta página mantém o calendário atual e o registro de mitigações.
- Atualize as tabelas após cada janela routed-trace, voto de governança ou ensaio de lançamento. Quando os artistas se movem, reflita a nova localização nesta página para que os documentos posteriores (status, dashboards, portais SDK) possam vincular um ancoradouro estavel.

## Instantâneo da evidência (1º-2º trimestre de 2026)

| Fluxo de trabalho | Evidência | Proprietário(s) | Estado | Notas |
|------------|----------|----------|--------|-------|
| **B1 – Auditorias de rastreamento roteado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetria-ops, @governança | Completo (1º trimestre de 2026) | Três janelas de auditoria registradas; o atraso TLS de `TRACE-CONFIG-DELTA` foi fechado durante a repetição do segundo trimestre. |
| **B2 - Remediação de telemetria e guarda-corpos** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetria-ops | Completo | Alert pack, política do diff bot e tamanho de lote OTLP (`nexus.scheduler.headroom` log + painel Grafana de headroom) enviados; sem renúncias em aberto. |
| **B3 - Aprovações de deltas de configuração** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governança | Completo | Voto GOV-2026-03-19 registrado; o pacote contratado alimenta o pacote de telemetria citado abaixo. |
| **B4 - Ensaio de lançamento multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (2º trimestre de 2026) | A reexecução do canário do segundo trimestre fechou a mitigação do atraso TLS; o manifesto do validador + `.sha256` captura o intervalo de slots 912-936, workload seed `NEXUS-REH-2026Q2` e o hash do perfil TLS registrado no rerun. |

## Calendário trimestral de auditorias routed-trace

| ID de rastreamento | Janela (UTC) | Resultado | Notas |
|----------|-------------|---------|-------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Aprovado | A fila-admissão P95 ficou bem abaixo do alvo <=750 ms. Nenhuma ação necessária. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Aprovado | Hashes de repetição OTLP anexados a `status.md`; a paridade do SDK diff bot confirmou desvio zero. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Resolvido | O atraso do TLS foi fechado durante a repetição do segundo trimestre; o pack de telemetria para `NEXUS-REH-2026Q2` registra o hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) e zero atrasos. |
| `TRACE-MULTILANE-CANARY` | 05/05/2026 09:12-10:14 | Aprovado | Semente de carga de trabalho `NEXUS-REH-2026Q2`; pack de telemetria + manifest/digest em `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) com agenda em `artifacts/nexus/rehearsals/2026q2/`. |

Os trimestres futuros devem adicionar novas linhas e mover as entradas concluídas para um apêndice quando a tabela crescer além do trimestre atual. Consulte esta seção a partir de relatórios routed-trace ou atas de governança usando a âncora `#quarterly-routed-trace-audit-schedule`.

## Mitigações e itens de backlog| Artigo | Descrição | Proprietário | Alvo | Status/Notas |
|------|------------|-------|--------|----------------|
| `NEXUS-421` | Finalizar a propagação do perfil TLS que ficou atrasado durante `TRACE-CONFIG-DELTA`, capturar evidência da reexecução e fechar o registro de mitigação. | @release-eng, @sre-core | Janela roteado-traço do segundo trimestre de 2026 | Fechado - hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado em `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; a reprise confirmou que não há atrasos. |
| Preparação `TRACE-MULTILANE-CANARY` | Programando o ensaio de Q2, anexar fixtures ao pacote de telemetria e garantir que os chicotes SDK reutilizem o helper validado. | @telemetry-ops, Programa SDK | Chamada de planejamento 2026-04-30 | Completo - agenda armazenada em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com metadados de slot/carga de trabalho; reutilizacao do arnês anotada no tracker. |
| Rotação de resumo do pacote de telemetria | Executar `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada ensaio/release e registrar digests ao lado do tracker de config delta. | @telemetria-ops | Por candidato a lançamento | Completo - `telemetry_manifest.json` + `.sha256` emitidos em `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); resumos copiados sem rastreador e sem índice de evidência. |

## Integração do pacote de configuração delta

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` segue como resumo canônico de diffs. Quando chegarem novos `defaults/nexus/*.toml` ou mudancas de genesis, atualize esse tracker primeiro e depois reflita os destaques aqui.
- Os pacotes de configuração assinados alimentam o pacote de ensaio de telemetria. O pacote, validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, deve ser publicado junto com a evidência de config delta para que os operadores possam reproduzir os artefatos exatos usados ​​durante B4.
- Os bundles de Iroha 2 permanecem sem lanes: configs com `nexus.enabled = false` agora rejeitam overrides de lane/dataspace/routing a menos que o perfil Nexus esteja habilitado (`--sora`), então remova as secas `nexus.*` dos templates faixa única.
- Manter o log de voto de governança (GOV-2026-03-19) linkado tanto no tracker quanto nesta nota para que futuros votos possam copiar o formato sem redescobrir o ritual de aprovação.

## Acompanhamentos do ensaio de lançamento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura o plano canary, a lista de participantes e os passos de rollback; atualizar o runbook quando a topologia de pistas ou os exportadores de telemetria mudarem.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefato verificado durante o ensaio de 9 de abril e agora inclui notas/agenda de preparação Q2. Adicione ensaios futuros ao mesmo tracker em vez de abrir trackers isolados para manter a evidência monótona.
- Public snippets do coletor OTLP e exports do Grafana (ver `docs/source/telemetry.md`) quando a orientação de batching do exportador muda; a atualização do 1º trimestre elevou o tamanho do lote para 256 amostras para evitar alertas de headroom.
- A evidência de CI/testes multi-lane agora vive em `integration_tests/tests/nexus/multilane_pipeline.rs` e roda sob o fluxo de trabalho `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), substituindo a referência aposentada `pytests/nexus/test_multilane_pipeline.py`; mantenha o hash de `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) em sincronização com o tracker ao atualizar pacotes de ensaio.

## Ciclo de vida de pistas em tempo de execução

- Os planos de ciclo de vida de pistas em tempo de execução agora validam ligações de dataspace e abortam quando a reconciliação Kura/armazenamento em camadas falha, mantendo o catálogo inalterado. Os helpers podem retransmitir as pistas em cache para as pistas aposentadas, para que um merge-ledger sintese não reutilize provas obsoletas.
- Aplique planos pelos helpers de config/lifecycle do Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para adicionar/retirar lanes sem reinicio; routing, snapshots TEU e registos de manifestos recarregam automaticamente após um plano bem sucedido.
- Orientação para operadores: quando um plano falha, verifique espaços de dados ausentes ou raízes de armazenamento que não podem ser criadas (tiered cold root/diretórios Kura por lane). Corrija os caminhos base e tente novamente; planos bem-sucedidos reemitem o diferencial de telemetria de lane/dataspace para que os dashboards reflitam a nova topologia.

## Telemetria NPoS e evidência de contrapressão

O retro do ensaio de lançamento da Fase B pediu capturas de telemetria deterministas que provam que o marca-passo NPoS e as camadas de fofoca permanecem dentro de seus limites de contrapressão. O chicote de integração em `integration_tests/tests/sumeragi_npos_performance.rs` exercita esses cenários e emite resumos JSON (`sumeragi_baseline_summary::<scenario>::...`) quando novas métricas chegam. Execute localmente com:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```Defina `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologias de maior estresse; os valores padrão refletem o perfil de coletores 1 s/`k=3` usado em B4.

| Cenário / teste | Cobertura | Chave de telemetria |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueia 12 rodadas com o block time do ensaio para registrar envelopes de latência EMA, profundidades de fila e medidores de redundante-envio antes de serializar o pacote de evidências. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda a fila de transações para garantir que os diferimentos de admissão sejam ativados de forma determinista e que a fila exporte contadores de capacidade/saturação. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Amostramos o jitter do marcapasso e os timeouts de view que testamos que a banda +/-125 permille e aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empurra payloads RBC grandes até os limites soft/hard do store para mostrar que sessões e contadores de bytes sobem, recuam e se estabilizam sem ultrapassar o store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Força retransmissões para que os medidores de taxa redundante-send e os contadores de coletores no alvo avancem, provando que a telemetria pedida pelo retro está conectada ponta a ponta. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Descarte pedaços em intervalos deterministas para verificar se os monitores de backlog apresentam falhas ao drenar silenciosamente as cargas úteis. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Anexe as linhas JSON que o chicote imprime junto com o scrape do Prometheus capturado durante a execução sempre que a governança solicita evidência de que os alarmes de contrapressão provenientes da topologia do ensaio.

## Checklist de atualização

1. Adicione novas janelas routed-trace e retire as antigas quando os trimestres girarem.
2. Atualize uma tabela de mitigação após cada acompanhamento do Alertmanager, mesmo que a ação seja fechar o ticket.
3. Quando os config deltas mudam, atualize o tracker, esta nota e a lista de resumos do pacote de telemetria no mesmo pull request.
4. Linke aqui quaisquer novos artefatos de ensaio/telemetria para que futuras atualizações de roadmap possam referenciar um documento único em vez de notas ad-hoc dispersas.

## Índice de evidência

| Ativo | Localização | Notas |
|-------|----------|-------|
| Relatório de auditoria routed-trace (1º trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fonte canônica da evidência da Fase B1; espelhado para o portal em `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Rastreador de configuração delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contem os resumos de diffs TRACE-CONFIG-DELTA, iniciais de revisores e o log de voto GOV-2026-03-19. |
| Plano de remediação de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | Documenta o pacote de alertas, o tamanho do lote OTLP e os guardrails de orcamento de exportação garantidos a B2. |
| Rastreador de ensaio multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista dos artistas do ensaio de 9 de abril, manifesto/digest do validador, notas/agenda Q2 e evidências de rollback. |
| Manifesto/resumo do pacote de telemetria (mais recente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+`.sha256`) | Registra o slot range 912-936, seed `NEXUS-REH-2026Q2` e hashes de artistas para pacotes de governança. |
| Manifesto do perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+`.sha256`) | Hash do perfil TLS aprovado capturado durante a repetição do segundo trimestre; cite em apêndices routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planejamento para o ensaio Q2 (janela, faixa de slots, workload seed, proprietários de ações). |
| Runbook de ensaio de lançamento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operacional para staging -> execução -> rollback; atualizar quando a topologia de pistas ou a orientação de exportadores mudarem. |
| Validador de pacote de telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciado pelo retro B4; arquive digests ao lado do tracker sempre que o pack muda. |
| Regressão multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Teste `nexus.enabled = true` para configurações multi-lane, preserve os hashes do catálogo Sora e provisione caminhos Kura/merge-log por lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` antes de publicar resumos de artefatos. |