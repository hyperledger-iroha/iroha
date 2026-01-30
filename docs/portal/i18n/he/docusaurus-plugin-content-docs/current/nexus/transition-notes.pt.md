---
lang: he
direction: rtl
source: docs/portal/docs/nexus/transition-notes.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-transition-notes
title: Notas de transicao do Nexus
description: Espelho de `docs/source/nexus_transition_notes.md`, cobrindo evidencia de transicao da Phase B, o calendario de auditoria e as mitigacoes.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transicao do Nexus

Este registro acompanha o trabalho pendente de **Phase B - Nexus Transition Foundations** ate que a checklist de lancamento multi-lane termine. Ele complementa as entradas de milestones em `roadmap.md` e mantem a evidencia referenciada por B1-B4 em um unico lugar para que governanca, SRE e lideres de SDK compartilhem a mesma fonte de verdade.

## Escopo e cadencia

- Cobre as auditorias routed-trace e os guardrails de telemetria (B1/B2), o conjunto de deltas de configuracao aprovado por governanca (B3) e os acompanhamentos do ensaio de lancamento multi-lane (B4).
- Substitui a nota temporaria de cadencia que antes vivia aqui; a partir da auditoria de Q1 2026 o relatorio detalhado reside em `docs/source/nexus_routed_trace_audit_report_2026q1.md`, enquanto esta pagina mantem o calendario corrente e o registro de mitigacoes.
- Atualize as tabelas apos cada janela routed-trace, voto de governanca ou ensaio de lancamento. Quando os artefatos se moverem, reflita a nova localizacao dentro desta pagina para que os docs posteriores (status, dashboards, portais SDK) possam linkar um ancoradouro estavel.

## Snapshot de evidencia (2026 Q1-Q2)

| Workstream | Evidencia | Owner(s) | Status | Notas |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Completo (Q1 2026) | Tres janelas de auditoria registradas; o atraso TLS de `TRACE-CONFIG-DELTA` foi fechado durante o rerun de Q2. |
| **B2 - Remediacao de telemetria e guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Completo | Alert pack, politica do diff bot e tamanho de lote OTLP (`nexus.scheduler.headroom` log + painel Grafana de headroom) enviados; sem waivers em aberto. |
| **B3 - Aprovacoes de deltas de configuracao** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Completo | Voto GOV-2026-03-19 registrado; o bundle assinado alimenta o pack de telemetria citado abaixo. |
| **B4 - Ensaio de lancamento multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (Q2 2026) | O rerun canary de Q2 fechou a mitigacao do atraso TLS; o validator manifest + `.sha256` captura o intervalo de slots 912-936, workload seed `NEXUS-REH-2026Q2` e o hash do perfil TLS registrado no rerun. |

## Calendario trimestral de auditorias routed-trace

| Trace ID | Janela (UTC) | Resultado | Notas |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Aprovado | Queue-admission P95 ficou bem abaixo do alvo <=750 ms. Nenhuma acao necessaria. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Aprovado | OTLP replay hashes anexados a `status.md`; a paridade do SDK diff bot confirmou zero drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resolvido | O atraso TLS foi fechado durante o rerun de Q2; o pack de telemetria para `NEXUS-REH-2026Q2` registra o hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) e zero atrasados. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Aprovado | Workload seed `NEXUS-REH-2026Q2`; pack de telemetria + manifest/digest em `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) com agenda em `artifacts/nexus/rehearsals/2026q2/`. |

Os trimestres futuros devem adicionar novas linhas e mover as entradas concluidas para um apendice quando a tabela crescer alem do trimestre atual. Referencie esta secao a partir de relatorios routed-trace ou atas de governanca usando a ancora `#quarterly-routed-trace-audit-schedule`.

## Mitigacoes e items de backlog

| Item | Descricao | Owner | Alvo | Status / Notas |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Finalizar a propagacao do perfil TLS que ficou atrasado durante `TRACE-CONFIG-DELTA`, capturar evidencia do rerun e fechar o registro de mitigacao. | @release-eng, @sre-core | Janela routed-trace de Q2 2026 | Fechado - hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado em `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; o rerun confirmou que nao ha atrasados. |
| `TRACE-MULTILANE-CANARY` prep | Programar o ensaio de Q2, anexar fixtures ao pack de telemetria e garantir que os SDK harnesses reutilizem o helper validado. | @telemetry-ops, SDK Program | Chamada de planejamento 2026-04-30 | Completo - agenda armazenada em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com metadados de slot/workload; reutilizacao do harness anotada no tracker. |
| Telemetry pack digest rotation | Executar `scripts/telemetry/validate_nexus_telemetry_pack.py` antes de cada ensaio/release e registrar digests ao lado do tracker de config delta. | @telemetry-ops | Por release candidate | Completo - `telemetry_manifest.json` + `.sha256` emitidos em `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests copiados no tracker e no indice de evidencia. |

## Integracao do bundle de config delta

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` segue como resumo canonico de diffs. Quando chegarem novos `defaults/nexus/*.toml` ou mudancas de genesis, atualize esse tracker primeiro e depois reflita os destaques aqui.
- Os signed config bundles alimentam o telemetry pack de ensaio. O pack, validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, deve ser publicado junto com a evidencia de config delta para que os operadores possam reproduzir os artefatos exatos usados durante B4.
- Os bundles de Iroha 2 permanecem sem lanes: configs com `nexus.enabled = false` agora rejeitam overrides de lane/dataspace/routing a menos que o perfil Nexus esteja habilitado (`--sora`), entao remova as secoes `nexus.*` das templates single-lane.
- Mantenha o log de voto de governanca (GOV-2026-03-19) linkado tanto no tracker quanto nesta nota para que futuros votos possam copiar o formato sem redescobrir o ritual de aprovacao.

## Acompanhamentos do ensaio de lancamento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captura o plano canary, a lista de participantes e os passos de rollback; atualize o runbook quando a topologia de lanes ou os exporters de telemetria mudarem.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lista cada artefato checado durante o ensaio de 9 de abril e agora inclui notas/agenda de preparacao Q2. Adicione ensaios futuros ao mesmo tracker em vez de abrir trackers isolados para manter a evidencia monotona.
- Publique snippets do coletor OTLP e exports do Grafana (ver `docs/source/telemetry.md`) quando a orientacao de batching do exporter mudar; a atualizacao de Q1 elevou o batch size para 256 amostras para evitar alertas de headroom.
- A evidencia de CI/tests multi-lane agora vive em `integration_tests/tests/nexus/multilane_pipeline.rs` e roda sob o workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), substituindo a referencia aposentada `pytests/nexus/test_multilane_pipeline.py`; mantenha o hash de `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) em sync com o tracker ao atualizar bundles de ensaio.

## Ciclo de vida de lanes em runtime

- Os planos de ciclo de vida de lanes em runtime agora validam bindings de dataspace e abortam quando a reconciliacao Kura/armazenamento em camadas falha, mantendo o catalogo inalterado. Os helpers podam relays de lanes em cache para lanes aposentadas, para que a sintese merge-ledger nao reutilize proofs obsoletas.
- Aplique planos pelos helpers de config/lifecycle do Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para adicionar/retirar lanes sem reinicio; routing, snapshots TEU e registries de manifests recarregam automaticamente apos um plano bem-sucedido.
- Orientacao para operadores: quando um plano falha, verifique dataspaces ausentes ou storage roots que nao podem ser criados (tiered cold root/diretorios Kura por lane). Corrija os caminhos base e tente novamente; planos bem-sucedidos re-emitem o diff de telemetria de lane/dataspace para que os dashboards reflitam a nova topologia.

## Telemetria NPoS e evidencia de backpressure

O retro do ensaio de lancamento da Phase B pediu capturas de telemetria deterministas que provem que o pacemaker NPoS e as camadas de gossip permanecem dentro de seus limites de backpressure. O harness de integracao em `integration_tests/tests/sumeragi_npos_performance.rs` exercita esses cenarios e emite resumos JSON (`sumeragi_baseline_summary::<scenario>::...`) quando novas metricas chegam. Execute localmente com:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Defina `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para explorar topologias de maior estresse; os valores padrao refletem o perfil de coletores 1 s/`k=3` usado em B4.

| Cenario / test | Cobertura | Telemetria chave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueia 12 rodadas com o block time do ensaio para registrar envelopes de latencia EMA, profundidades de fila e gauges de redundant-send antes de serializar o bundle de evidencia. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda a fila de transacoes para garantir que as deferrals de admissao sejam ativadas de forma determinista e que a fila exporte contadores de capacidade/saturacao. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Amostra o jitter do pacemaker e os timeouts de view ate provar que a banda +/-125 permille e aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Empurra payloads RBC grandes ate os limites soft/hard do store para mostrar que sessoes e contadores de bytes sobem, recuam e se estabilizam sem ultrapassar o store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Forca retransmissoes para que os gauges de ratio redundant-send e os contadores de collectors-on-target avancem, provando que a telemetria pedida pelo retro esta conectada end-to-end. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Descarta chunks em intervalos deterministas para verificar que os monitores de backlog levantam falhas em vez de drenar silenciosamente os payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Anexe as linhas JSON que o harness imprime junto com o scrape do Prometheus capturado durante a execucao sempre que a governanca solicitar evidencia de que os alarmes de backpressure correspondem a topologia do ensaio.

## Checklist de atualizacao

1. Adicione novas janelas routed-trace e retire as antigas quando os trimestres girarem.
2. Atualize a tabela de mitigacao apos cada acompanhamento do Alertmanager, mesmo que a acao seja fechar o ticket.
3. Quando os config deltas mudarem, atualize o tracker, esta nota e a lista de digests do telemetry pack no mesmo pull request.
4. Linke aqui qualquer novo artefato de ensaio/telemetria para que futuras atualizacoes de roadmap possam referenciar um unico documento em vez de notas ad-hoc dispersas.

## Indice de evidencia

| Ativo | Localizacao | Notas |
|-------|----------|-------|
| Relatorio de auditoria routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fonte canonica da evidencia de Phase B1; espelhado para o portal em `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contem os resumos de diffs TRACE-CONFIG-DELTA, iniciais de revisores e o log de voto GOV-2026-03-19. |
| Plano de remediacao de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | Documenta o alert pack, o tamanho de lote OTLP e os guardrails de orcamento de exportacao vinculados a B2. |
| Tracker de ensaio multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista os artefatos do ensaio de 9 de abril, manifest/digest do validator, notas/agenda Q2 e evidencia de rollback. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Registra o slot range 912-936, seed `NEXUS-REH-2026Q2` e hashes de artefatos para bundles de governanca. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash do perfil TLS aprovado capturado durante o rerun de Q2; cite em apendices routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planejamento para o ensaio Q2 (janela, slot range, workload seed, owners de acoes). |
| Runbook de ensaio de lancamento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist operacional para staging -> execucao -> rollback; atualizar quando a topologia de lanes ou a orientacao de exporters mudar. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciado pelo retro B4; arquive digests ao lado do tracker sempre que o pack mudar. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prova `nexus.enabled = true` para configs multi-lane, preserva os hashes do catalogo Sora e provisiona caminhos Kura/merge-log por lane (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` antes de publicar digests de artefatos. |
