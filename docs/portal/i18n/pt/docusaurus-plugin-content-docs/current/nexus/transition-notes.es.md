---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/transition-notes.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas de transição do nexo
título: Notas de transição de Nexus
descrição: Espejo de `docs/source/nexus_transition_notes.md`, que contém evidências de transição da Fase B, o calendário de auditorias e as mitigações.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transição de Nexus

Este registro rastreia o trabalho pendente de **Fase B - Nexus Transition Foundations** até finalizar a lista de verificação de lançamento multi-lane. Complementa as entradas de marcos em `roadmap.md` e mantém a evidência referenciada por B1-B4 em um único lugar para que governo, SRE e líderes de SDK compartilhem a mesma fonte de verdade.

## Alcance e cadência

- Cubra os auditórios route-trace e os guardrails de telemetria (B1/B2), o conjunto de deltas de configuração aprovados pela governança (B3) e os acompanhamentos do ensaio de lançamento multi-lane (B4).
- Reemplaza a nota temporal de cadência que antes vivia aqui; desde os auditórios do primeiro trimestre de 2026, o relatório detalhado reside em `docs/source/nexus_routed_trace_audit_report_2026q1.md`, enquanto esta página mantém o calendário operacional e o registro de mitigações.
- Atualize as tabelas após cada janela de rastreamento roteado, voto de governança ou ensaio de lançamento. Quando os artefatos são alterados, reflita a nova localização nesta página para que os documentos posteriores (status, painéis, portais SDK) possam ser inseridos em uma lista estável.

## Instantâneo da evidência (1º-2º trimestre de 2026)

| Fluxo de trabalho | Evidência | Proprietário(s) | Estado | Notas |
|------------|----------|----------|--------|-------|
| **B1 – Auditorias de rastreamento roteado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetria-ops, @governança | Completo (1º trimestre de 2026) | Três janelas de auditorias registradas; o retrocesso TLS de `TRACE-CONFIG-DELTA` foi interrompido durante a repetição do segundo trimestre. |
| **B2 - Remediação de telemetria e guarda-corpos** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetria-ops | Completo | Pacote de alerta, política de bot diff e quantidade de lote OTLP (`nexus.scheduler.headroom` log + painel de Grafana de headroom) enviados; renúncia ao pecado aberta. |
| **B3 - Aprovações de deltas de configuração** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Voto GOV-2026-03-19 registrado; el bundle firmado alimenta o pacote de telemetria citado abaixo. |
| **B4 - Ensaio de lançamento multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (2º trimestre de 2026) | A reexecução canária do segundo trimestre cerrou a mitigação do retrocesso TLS; o manifesto do validador + `.sha256` captura o intervalo dos slots 912-936, a semente da carga de trabalho `NEXUS-REH-2026Q2` e o hash do perfil TLS registrado na reexecução. |

## Calendário trimestral de auditorias routed-trace

| ID de rastreamento | Ventana (UTC) | Resultado | Notas |
|----------|-------------|---------|-------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Aprovado | Queue-admission P95 se mantuvo muy por baixo do objetivo <=750 ms. Não é necessária ação. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Aprovado | Hashes de repetição OTLP anexados a `status.md`; a paridade do SDK diff bot confirmou zero desvio. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Resultado | O retrocesso TLS foi interrompido durante a repetição do segundo trimestre; el pack de telemetria para `NEXUS-REH-2026Q2` registra o hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) e zero rezagados. |
| `TRACE-MULTILANE-CANARY` | 05/05/2026 09:12-10:14 | Aprovado | Semente de carga de trabalho `NEXUS-REH-2026Q2`; pacote de telemetria + manifesto/resumo em `artifacts/nexus/rehearsals/2026q1/` (intervalo de slots 912-936) com agenda em `artifacts/nexus/rehearsals/2026q2/`. |

Os trimestres futuros devem adicionar novas filas e mover as entradas completadas para um apêndice quando a tabela cresce mas no trimestre atual. Referência nesta seção de relatórios de rastreamento roteado ou minutos de governança usando a ancla `#quarterly-routed-trace-audit-schedule`.

## Mitigações e itens do backlog| Artigo | Descrição | Proprietário | Objetivo | Estado/Notas |
|------|------------|-------|--------|----------------|
| `NEXUS-421` | Terminar de propagar o perfil TLS que se retrocedeu durante `TRACE-CONFIG-DELTA`, capturar evidências de reexecução e fechar o registro de mitigação. | @release-eng, @sre-core | Ventana roteado-traço do segundo trimestre de 2026 | Cerrado - hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado em `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; a reprise confirma que não há rezagados. |
| Preparação `TRACE-MULTILANE-CANARY` | Programe o ensaio do Q2, adicione fixtures ao pacote de telemetria e certifique-se de que os chicotes do SDK reutilizem o helper validado. | @telemetry-ops, Programa SDK | Chamada de avião 2026-04-30 | Completado - agenda armazenada em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com metadados de slot/carga de trabalho; reutilização do arnês anotado no rastreador. |
| Rotação de resumo do pacote de telemetria | Execute `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada ensaio/liberação e registre resumos junto com o rastreador de configuração delta. | @telemetria-ops | Por candidato a lançamento | Completado - `telemetry_manifest.json` + `.sha256` emitidos em `artifacts/nexus/rehearsals/2026q1/` (intervalo de slots `912-936`, seed `NEXUS-REH-2026Q2`); resumos copiados no rastreador e no índice de evidência. |

## Integração do pacote de configuração delta

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` sigue siendo el resumen canonico de diffs. Ao trazer novos `defaults/nexus/*.toml` ou mudanças de gênese, atualize esse rastreador primeiro e depois reflita os pontos chaves aqui.
- Os pacotes de configuração assinados alimentam o pacote de telemetria do ensaio. O pacote, validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, deve ser publicado junto com a evidência de configuração delta para que os operadores possam reproduzir os artefatos exatos usados ​​durante B4.
- Os pacotes de Iroha 2 seguem sem pistas: as configurações com `nexus.enabled = false` agora rechazan substituições de lane/dataspace/routing a menos que o perfil Nexus esteja habilitado (`--sora`), assim que eliminar as seções `nexus.*` das plantações de pista única.
- Mantenha o registro do voto de governo (GOV-2026-03-19) enlazado tanto desde o rastreador como desde esta nota para que futuras votações possam copiar o formato sem redescobrir o ritual de aprovação.

## Seguimentos do ensaio de lançamento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura o plano canário, a lista de participantes e os passos de rollback; atualiza o runbook quando muda a topologia das pistas ou dos exportadores de telemetria.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefato revisado durante o ensaio de 9 de abril e agora inclui notas/agenda de preparação Q2. Agregar futuros ensaios ao mesmo rastreador em vez de abrir rastreadores isolados para manter a evidência monótona.
- Publica trechos do coletor OTLP e exportações de Grafana (ver `docs/source/telemetry.md`) quando muda o guia de lote do exportador; a atualização do primeiro trimestre subio o tamanho do lote para 256 telas para evitar alertas de espaço livre.
- A evidência de CI/testes multi-lane agora vive em `integration_tests/tests/nexus/multilane_pipeline.rs` e corre abaixo do fluxo de trabalho `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), substituindo a referência retirada `pytests/nexus/test_multilane_pipeline.py`; Mantenha o hash para `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) em sincronização com o rastreador para atualizar pacotes de ensaio.

## Ciclo de vida de pistas em tempo de execução

- Os planos do ciclo de vida das pistas em tempo de execução agora validam as ligações do espaço de dados e abortam quando a reconciliação de Kura/almacenamiento por níveis falha, deixando o catálogo sem mudanças. Os ajudantes podem retransmitir as pistas em cache para as pistas retiradas, de modo que a síntese do merge-ledger não reutilize provas obsoletas.
- Aplicar aviões através dos auxiliares de configuração/ciclo de vida de Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para adicionar/retirar pistas sem reiniciar; roteamento, snapshots TEU e registros de manifestos são recarregados automaticamente após um plano exitoso.
- Guia para operadores: quando um plano falha, revise dataspaces faltantes ou raízes de armazenamento que não podem ser criadas (tiered cold root/diretórios Kura por lane). Corrige las rutas base y reintenta; os aviões exitosos reemitem o diferencial de telemetria de pista/espaço de dados para que os painéis reflitam a nova topologia.

## Telemetria NPoS e evidência de contrapressãoO retro ensaio de lançamento da Fase B pidio capturas de telemetria deterministas que testam que o marca-passo NPoS e as capas de fofoca se mantêm dentro de seus limites de contrapressão. O chicote de integração em `integration_tests/tests/sumeragi_npos_performance.rs` gera esses cenários e emite currículos JSON (`sumeragi_baseline_summary::<scenario>::...`) quando são adicionadas novas métricas. Execute-o localmente com:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Configure `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologias de maiores estrelas; os valores por defeito refletem o perfil dos coletores 1 s/`k=3` usados ​​em B4.

| Cenário/teste | Cobertura | Chave de telemetria |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueie 12 rodadas com o tempo de bloco do ensaio para registrar envelopes de latência EMA, profundidades de cola e medidores de envio redundante antes de serializar o pacote de evidências. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda a cola de transações para garantir que as deferências de admissão sejam ativas de forma determinista e que a cola exporte contadores de capacidade/saturação. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Mostre o jitter do marca-passo e os tempos limite de vista até demonstrar que a banda +/-125 pontos foi aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empuja payloads RBC grandes até os limites soft/hard do armazenamento para mostrar que as sessões e contadores de bytes suben, retrocedem e se estabilizam sem ultrapassar o armazenamento. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Fortalecemos as retransmissões para que os medidores de taxa de envio redundante e os contadores de coletores no alvo avancem, demonstrando que a telemetria pedida pelo retro está conectada de ponta a ponta. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Descarte pedaços de intervalos deterministas para verificar se os monitores de backlog falharam no lugar de drenar silenciosamente as cargas úteis. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Adicione as linhas JSON que imprimem o chicote junto com o arranhão de Prometheus capturado durante a execução sempre que o governo solicitar evidência de que os alarmes de contrapressão coincidem com a topologia do ensaio.

## Checklist de atualização

1. Agrega novas janelas roteadas e retira as antigas quando roten os trimestres.
2. Atualize a tabela de mitigações após cada acompanhamento do Alertmanager, mesmo que a ação feche o ticket.
3. Ao alterar os deltas de configuração, atualize o rastreador, esta nota e a lista de resumos do pacote de telemetria no mesmo pull request.
4. Veja aqui qualquer novo artefato de ensaio/telemetria para que futuras atualizações de roteiro possam referenciar um único documento em vez de notas dispersas ad-hoc.

## Índice de evidência| Ativo | Localização | Notas |
|-------|----------|-------|
| Relatório de auditoria routed-trace (1º trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fonte canônica para evidência da Fase B1; refletido para o portal em `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Rastreador de configuração delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contém os resumos de diferenças TRACE-CONFIG-DELTA, iniciais de revisores e o log de voto GOV-2026-03-19. |
| Plano de remediação de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | Documente o pacote de alertas, o tamanho do lote OTLP e os guardrails de pressuposto de exportação vinculados à B2. |
| Rastreador de ensaio multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista os artefatos do ensaio de 9 de abril, manifesto/resumo do validador, notas/agenda Q2 e evidências de reversão. |
| Manifesto/resumo do pacote de telemetria (mais recente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+`.sha256`) | Registra o slot range 912-936, seed `NEXUS-REH-2026Q2` e hashes de artefatos para pacotes de governo. |
| Manifesto do perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+`.sha256`) | Hash do perfil TLS aprovado capturado durante a repetição do segundo trimestre; citar em apêndices routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planejamento para o ensaio Q2 (ventana, intervalo de slots, semente de carga de trabalho, proprietários de ações). |
| Runbook de ensaio de lançamento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operativo para staging -> execução -> rollback; atualizar quando muda a topologia das pistas ou o guia dos exportadores. |
| Validador de pacote de telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciado pelo retro B4; archive digests junto com o tracker quando o pacote muda. |
| Regressão multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Verifique `nexus.enabled = true` para configurações multi-lane, preserve os hashes do catálogo Sora e forneça rotas Kura/merge-log por lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` antes de publicar resumos de artefatos. |