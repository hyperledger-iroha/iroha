---
lang: pt
direction: ltr
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

## Desdobramento de tarefas Sumeragi + NPoS

Esta nota expande o roadmap da Fase A em tarefas pequenas de engenharia para podermos entregar o trabalho restante de Sumeragi/NPoS de forma incremental. As anotacoes de status seguem a convencao: `DONE` concluido, `IN PROGRESS` em andamento, `NOT STARTED` nao iniciado, e `NEEDS TESTS` precisa de testes.

### A2 - Adocao de mensagens em nivel wire
- DONE: Expor os tipos Norito `Proposal`/`Vote`/`Qc` em `BlockMessage` e exercitar round-trips de encode/decode (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- DONE: Bloquear os frames antigos `BlockSigned/BlockCommitted`; o toggle de migracao ficou em `false` antes da retirada.
- DONE: Retirar o knob de migracao que alternava as mensagens de bloco antigas; o modo Vote/commit certificate agora e o unico caminho wire.
- DONE: Atualizar routers Torii, comandos CLI e consumidores de telemetria para preferir snapshots JSON `/v1/sumeragi/*` aos frames de bloco antigos.
- DONE: A cobertura de integracao exercita os endpoints `/v1/sumeragi/*` somente pelo pipeline Vote/commit certificate (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- DONE: Remover os frames antigos quando houver paridade de recursos e testes de interoperabilidade.

### Plano de remocao de frames
1. DONE: Testes soak multi-no rodaram 72 h nos harnesses de telemetria e CI; os snapshots do Torii mostraram throughput estavel do proposer e formacao de commit certificate sem regressao.
2. DONE: A cobertura de testes de integracao agora roda apenas no caminho Vote/commit certificate (`sumeragi_vote_qc_commit.rs`), garantindo que peers mistos alcancem consenso sem os frames antigos.
3. DONE: A documentacao de operador e a ajuda do CLI nao mencionam mais o caminho wire anterior; a orientacao de troubleshooting agora aponta para a telemetria Vote/commit certificate.
4. DONE: Variantes de mensagens, contadores de telemetria e caches de commit pendentes foram removidos; a matriz de compatibilidade agora reflete a superficie somente Vote/commit certificate.

### A3 - Aplicacao do motor e pacemaker
- DONE: Invariantes Lock/Highestcommit certificate aplicadas em `handle_message` (ver `block_created_header_sanity`).
- DONE: O acompanhamento de disponibilidade de dados valida o hash do payload RBC ao registrar a entrega (`Actor::ensure_block_matches_rbc_payload`), para que sessoes divergentes nao sejam tratadas como entregues.
- DONE: Inserir o requisito Precommitcommit certificate (`require_precommit_qc`) nas configs padrao e adicionar testes negativos (padrao agora `true`; testes cobrem caminhos com gate e opt-out).
- DONE: Substituir heuristicas de redundant-send em nivel de view por controladores de pacemaker baseados em EMA (`aggregator_retry_deadline` agora deriva da EMA em tempo real e define deadlines de redundant send).
- DONE: Bloquear a montagem de propostas sob backpressure da fila (`BackpressureGate` agora para o pacemaker quando a fila esta saturada e registra deferrals para status/telemetry).
- DONE: Votos de availability sao emitidos apos a validacao da proposta quando DA e requerido (sem esperar o `DELIVER` local de RBC), e a evidencia de availability e acompanhada via `availability evidence` como prova de seguranca enquanto o commit prossegue sem esperar. Isso evita esperas circulares entre transporte de payload e votacao.
- DONE: A cobertura de restart/liveness agora exercita recuperacao RBC em cold-start (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) e retomada do pacemaker apos downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- DONE: Adicionar testes de regressao deterministas de restart/view-change cobrindo convergencia de lock (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 - Pipeline de collectors e aleatoriedade
- DONE: Helpers de rotacao determinista de collectors vivem em `collectors.rs`.
- DONE: GA-A4.1 - A selecao de collectors apoiada por PRF agora registra seeds deterministas e height/view em `/status` e telemetria; hooks de refresh de VRF propagam o contexto apos commits e reveals. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md` (fechado).
- DONE: GA-A4.2 - Expor telemetria de participacao de reveal + comandos CLI de inspecao e atualizar manifests Norito. Owners: `@telemetry-ops`, `@torii-sdk`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:6`.
- DONE: GA-A4.3 - Codificar recuperacao de late-reveal e testes de epoch sem participacao em `integration_tests/tests/sumeragi_randomness.rs` (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`), exercitando telemetria de limpeza de penalidades. Owners: `@sumeragi-core`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 - Reconfiguracao conjunta e evidence
- DONE: O scaffolding de evidence, a persistencia em WSV e os roundtrips Norito agora cobrem double-vote, invalid proposal, invalid commit certificate e variantes de double exec com deduplicacao determinista e poda do horizonte (`sumeragi::evidence`).
- DONE: GA-A5.1 - Ativacao de joint-consensus (set antigo commita, novo set ativa no proximo bloco) aplicada com cobertura de integracao direcionada.
- DONE: GA-A5.2 - Docs de governance e fluxos CLI para slashing/jailing atualizados, com testes de sincronizacao mdBook para fixar defaults e o wording do evidence horizon.
- DONE: GA-A5.3 - Testes de evidence em caminho negativo (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) mais fixtures fuzz chegaram e rodam nightly para proteger a validacao de roundtrip Norito.

### A6 - Tooling, docs, validacao
- DONE: Telemetria/reporting RBC em dia; o relatorio DA gera metricas reais (incluindo contadores de eviction).
- DONE: GA-A6.1 - O teste happy-path NPoS com VRF e 4 peers agora roda em CI com limites pacemaker/RBC aplicados via `integration_tests/tests/sumeragi_npos_happy_path.rs`. Owners: `@qa-consensus`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:11`.
- DONE: GA-A6.2 - Capturar baseline de desempenho NPoS (blocos de 1 s, k=3) e publicar em `status.md`/docs de operador com seeds reproduziveis do harness + matriz de hardware. Owners: `@performance-lab`, `@telemetry-ops`. Report: `docs/source/generated/sumeragi_baseline_report.md`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:12`. A execucao ao vivo foi registrada no Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) usando o comando documentado em `scripts/run_sumeragi_baseline.py`.
- DONE: GA-A6.3 - Guias de troubleshooting de operador para instrumentacao de RBC/pacemaker/backpressure chegaram (`docs/source/telemetry.md:523`); a correlacao de logs agora e feita por `scripts/sumeragi_backpressure_log_scraper.py`, permitindo que operadores obtenham pares de deferral de pacemaker/availability ausente sem grep manual. Owners: `@operator-docs`, `@telemetry-ops`. Tracker: `project_tracker/npos_sumeragi_phase_a.md:13`.
- DONE: Foram adicionados cenarios de desempenho de RBC store/chunk-loss (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`), cobertura de fan-out redundante (`npos_redundant_send_retries_update_metrics`) e um harness de jitter limitado (`npos_pacemaker_jitter_within_band`) para que a suite A6 exercite deferrals de soft-limit do store, drops deterministas de chunks, telemetria de redundant-send e bandas de jitter do pacemaker sob estresse. [integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### Proximos passos imediatos
1. DONE: O harness de jitter limitado exercita metricas de jitter do pacemaker (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. DONE: Promover as assercoes de deferral RBC em `npos_queue_backpressure_triggers_metrics` ao preparar pressao determinista do store RBC (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. DONE: Estender o soak de `/v1/sumeragi/telemetry` para cobrir epochs longos e collectors adversarios, comparando snapshots com contadores Prometheus em multiplos heights. Coberto por `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`.

Manter esta lista aqui mantem `roadmap.md` focado em marcos enquanto oferece ao time uma checklist viva para executar. Atualize as entradas (e marque a conclusao) conforme os patches chegam.
