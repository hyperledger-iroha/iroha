---
lang: pt
direction: ltr
source: docs/source/nexus_refactor_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44b7100fddd377c97dfcab678ce425ec35edfa4a1276f9b6a22aa2c64135a94d
source_last_modified: "2025-11-02T04:40:40.017979+00:00"
translation_last_reviewed: 2026-01-01
---

# Plano de refatoracao do ledger Sora Nexus

Este documento captura o roadmap imediato para a refatoracao do Sora Nexus Ledger ("Iroha 3"). Ele
reflete o layout atual do repositorio e as regressoes observadas na contabilidade de genesis/WSV,
consenso Sumeragi, triggers de smart-contract, consultas de snapshot, bindings de host pointer-ABI
e codecs Norito. O objetivo e convergir para uma arquitetura coerente e testavel sem tentar
entregar todas as correcoes em um patch monolitico.

## 0. Principios guia
- Preservar comportamento deterministico em hardware heterogeneo; usar aceleracao apenas via
  feature flags opt-in com fallbacks identicos.
- Norito e a camada de serializacao. Qualquer mudanca de estado/schema deve incluir testes de
  round-trip Norito encode/decode e atualizacoes de fixtures.
- A configuracao flui por `iroha_config` (user -> actual -> defaults). Remover toggles ad-hoc de
  ambiente dos caminhos de producao.
- A politica ABI permanece em V1 e nao e negociavel. Hosts devem rejeitar tipos de
  ponteiro/syscalls desconhecidos de forma deterministica.
- `cargo test --workspace` e testes golden (`ivm`, `norito`, `integration_tests`) seguem como gate
  base para cada marco.

## 1. Snapshot da topologia do repositorio
- `crates/iroha_core`: atores Sumeragi, WSV, loader de genesis, pipelines (query, overlay, zk lanes),
  glue de host para smart-contracts.
- `crates/iroha_data_model`: schema autoritativo para dados on-chain e queries.
- `crates/iroha`: API de cliente usada por CLI, testes, SDK.
- `crates/iroha_cli`: CLI de operador, atualmente espelha varias APIs em `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, pontos de entrada de integracao host pointer-ABI.
- `crates/norito`: codec de serializacao com adaptadores JSON e backends AoS/NCB.
- `integration_tests`: asserts cross-component cobrindo genesis/bootstrap, Sumeragi, triggers,
  paginacao, etc.
- Docs ja descrevem objetivos do Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mas a
  implementacao esta fragmentada e parcialmente desatualizada em relacao ao codigo.

## 2. Pilares e marcos da refatoracao

### Fase A - Fundacoes e observabilidade
1. **Telemetria WSV + snapshots**
   - Estabelecer API de snapshot canonica em `state` (trait `WorldStateSnapshot`) usada por
     queries, Sumeragi e CLI.
   - Usar `scripts/iroha_state_dump.sh` para produzir snapshots deterministas via
     `iroha state dump --format norito`.
2. **Determinismo de genesis/bootstrap**
   - Refatorar a ingestao de genesis para fluir por um unico pipeline com Norito
     (`iroha_core::genesis`).
   - Adicionar cobertura de integracao/regressao que reproduz genesis mais o primeiro bloco e
     afirma raizes WSV identicas em arm64/x86_64 (trackeado em
     `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Testes de fixidez cross-crate**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline e
     ABI em um unico harness.
   - Introduzir um scaffold `cargo xtask check-shape` que panica com schema drift (trackeado no
     backlog de tooling DevEx; ver action item em `scripts/xtask/README.md`).

### Fase B - WSV e superficie de queries
1. **Transacoes de state storage**
   - Colapsar `state/storage_transactions.rs` em um adaptador transacional que imponha ordem de
     commit e deteccao de conflitos.
   - Testes unitarios agora verificam que modificacoes de assets/world/triggers fazem rollback em
     falhas.
2. **Refatoracao do modelo de queries**
   - Mover logica de paginacao/cursor para componentes reutilizaveis sob
     `crates/iroha_core/src/query/`. Alinhar representacoes Norito em `iroha_data_model`.
   - Adicionar snapshot queries para triggers, assets e roles com ordenacao deterministica
     (trackeado via `crates/iroha_core/tests/snapshot_iterable.rs` para a cobertura atual).
3. **Consistencia de snapshots**
   - Garantir que o CLI `iroha ledger query` use o mesmo caminho de snapshot que Sumeragi/fetchers.
   - Testes de regressao de snapshot do CLI vivem em `tests/cli/state_snapshot.rs` (feature-gated
     para runs lentos).

### Fase C - Pipeline Sumeragi
1. **Topologia e gestao de epocas**
   - Extrair `EpochRosterProvider` para um trait com implementacoes suportadas por snapshots de
     stake WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` oferece um construtor simples e mock-friendly para
     benches/tests.
2. **Simplificacao do fluxo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` em modulos: `pacemaker`, `aggregation`,
     `availability`, `witness` com tipos compartilhados sob `consensus`.
   - Substituir message passing ad-hoc por envelopes Norito tipados e introduzir property tests de
     view-change (trackeado no backlog de mensageria Sumeragi).
3. **Integracao de lanes/proofs**
   - Alinhar proofs de lane com compromissos DA e garantir que o gating RBC seja uniforme.
   - O teste de integracao end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs`
     agora valida o caminho com RBC habilitado.

### Fase D - Smart contracts e hosts pointer-ABI
1. **Auditoria de fronteira de host**
   - Consolidar checks de tipos de ponteiro (`ivm::pointer_abi`) e adaptadores de host
     (`iroha_core::smartcontracts::ivm::host`).
   - Expectativas da tabela de ponteiros e bindings de host manifest sao cobertas por
     `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que exercitam
     os mappings TLV golden.
2. **Sandbox de execucao de triggers**
   - Refatorar triggers para executar via um `TriggerExecutor` comum que imponha gas, validacao de
     ponteiros e journaling de eventos.
   - Adicionar testes de regressao para triggers de call/time cobrindo caminhos de falha
     (trackeado via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alinhamento CLI e cliente**
   - Garantir que operacoes CLI (`audit`, `gov`, `sumeragi`, `ivm`) dependam das funcoes
     compartilhadas do cliente `iroha` para evitar drift.
   - Testes de snapshot JSON do CLI vivem em `tests/cli/json_snapshot.rs`; manter atualizados para
     que a saida de comandos core siga a referencia JSON canonica.

### Fase E - Hardening do codec Norito
1. **Schema registry**
   - Criar um registry de schema Norito em `crates/norito/src/schema/` para obter codificacoes
     canonicas de tipos core.
   - Adicionar doc tests que verificam encoding de payloads de exemplo
     (`norito::schema::SamplePayload`).
2. **Refresh de golden fixtures**
   - Atualizar `crates/norito/tests/*` golden fixtures para corresponder ao novo schema WSV quando
     a refatoracao chegar.
   - `scripts/norito_regen.sh` regenera os goldens Norito JSON de forma deterministica via o helper
     `norito_regen_goldens`.
3. **Integracao IVM/Norito**
   - Validar a serializacao de manifest Kotodama end-to-end via Norito, garantindo que a metadata
     de pointer ABI seja consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantem paridade Norito encode/decode para manifests.

## 3. Preocupacoes transversais
- **Estrategia de testes**: Cada fase promove testes unitarios -> testes de crate -> testes de
  integracao. Testes com falha capturam regressoes atuais; novos testes evitam retorno.
- **Documentacao**: Apos cada fase, atualizar `status.md` e mover itens abertos para `roadmap.md`
  enquanto remove tarefas concluidas.
- **Benchmarks de performance**: Manter benches existentes em `iroha_core`, `ivm`, `norito`; adicionar
  medidas base pos-refatoracao para validar que nao ha regressoes.
- **Feature flags**: Manter toggles de nivel crate apenas para backends que exigem toolchains
  externos (`cuda`, `zk-verify-batch`). Caminhos SIMD de CPU sempre sao construidos e selecionados
  em runtime; fornecer fallbacks escalares deterministas para hardware sem suporte.

## 4. Acoes imediatas
- Scaffolding da Fase A (snapshot trait + wiring de telemetria) - ver tarefas acionaveis nas
  atualizacoes de roadmap.
- A auditoria recente de defeitos para `sumeragi`, `state` e `ivm` destacou:
  - `sumeragi`: allowances de dead-code guardam broadcast de proofs de view-change, estado de replay
    VRF e export de telemetria EMA. Permanecem gated ate que a simplificacao do fluxo de consenso e
    a integracao de lanes/proofs da Fase C cheguem.
  - `state`: limpeza de `Cell` e roteamento de telemetria seguem para o track de telemetria WSV da
    Fase A, enquanto as notas de SoA/parallel-apply entram no backlog de otimizacao do pipeline da
    Fase C.
  - `ivm`: exposicao do toggle CUDA, validacao de envelopes e cobertura Halo2/Metal se alinham ao
    trabalho de fronteira de host da Fase D mais o tema transversal de aceleracao GPU; kernels
    permanecem no backlog GPU dedicado ate readiness.
- Preparar um RFC cross-team resumindo este plano para sign-off antes de aterrar mudancas de codigo
  invasivas.

## 5. Perguntas abertas
- RBC deve permanecer opcional apos P1, ou e obrigatorio para lanes do ledger Nexus? Requer decisao
  dos stakeholders.
- Devemos forcar grupos de composabilidade DS em P1 ou manter desabilitados ate as proofs de lane
  amadurecerem?
- Qual e o local canonico para parametros ML-DSA-87? Candidato: novo crate `crates/fastpq_isi`
  (criacao pendente).

---

_Ultima atualizacao: 2025-09-12_
