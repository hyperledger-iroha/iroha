---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-refactor-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-refactor-plan
title: Plano de refatoracao do ledger Sora Nexus
description: Espelho de `docs/source/nexus_refactor_plan.md`, detalhando o trabalho de limpeza por fases para a base de codigo do Iroha 3.
---

:::note Fonte canonica
Esta pagina reflete `docs/source/nexus_refactor_plan.md`. Mantenha as duas copias alinhadas ate que a edicao multilingue chegue ao portal.
:::

# Plano de refatoracao do ledger Sora Nexus

Este documento captura o roadmap imediato para o refactor do Sora Nexus Ledger ("Iroha 3"). Ele reflete o layout atual do repositorio e as regressoes observadas em genesis/WSV bookkeeping, consenso Sumeragi, triggers de smart contracts, consultas de snapshots, bindings de host pointer-ABI e codecs Norito. O objetivo e convergir para uma arquitetura coerente e testavel sem tentar entregar todas as correcoes em um unico patch monolitico.

## 0. Principios guia
- Preservar comportamento determinista em hardware heterogeneo; usar aceleracao apenas via feature flags opt-in com fallbacks identicos.
- Norito e a camada de serializacao. Qualquer mudanca de estado/schema deve incluir testes de round-trip Norito encode/decode e atualizacoes de fixtures.
- A configuracao flui por `iroha_config` (user -> actual -> defaults). Remover toggles ad-hoc de ambiente dos paths de producao.
- A politica ABI permanece V1 e inegociavel. Hosts devem rejeitar pointer types/syscalls desconhecidos de forma deterministica.
- `cargo test --workspace` e os golden tests (`ivm`, `norito`, `integration_tests`) continuam como gate base para cada marco.

## 1. Snapshot da topologia do repositorio
- `crates/iroha_core`: atores Sumeragi, WSV, loader de genesis, pipelines (query, overlay, zk lanes), glue do host de smart contracts.
- `crates/iroha_data_model`: schema autoritativo para dados e queries on-chain.
- `crates/iroha`: API de cliente usada por CLI, tests, SDK.
- `crates/iroha_cli`: CLI de operadores, atualmente espelha numerosas APIs em `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, pontos de entrada de integracao pointer-ABI do host.
- `crates/norito`: codec de serializacao com adaptadores JSON e backends AoS/NCB.
- `integration_tests`: assertions cross-component cobrindo genesis/bootstrap, Sumeragi, triggers, paginacao, etc.
- Docs ja delineiam metas do Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mas a implementacao esta fragmentada e parcialmente defasada em relacao ao codigo.

## 2. Pilares de refatoracao e marcos

### Fase A - Fundacoes e observabilidade
1. **Telemetria WSV + Snapshots**
   - Estabelecer uma API canonica de snapshots em `state` (trait `WorldStateSnapshot`) usada por queries, Sumeragi e CLI.
   - Usar `scripts/iroha_state_dump.sh` para produzir snapshots deterministas via `iroha state dump --format norito`.
2. **Determinismo de Genesis/Bootstrap**
   - Refatorar a ingestao de genesis para fluir por um unico pipeline com Norito (`iroha_core::genesis`).
   - Adicionar cobertura de integracao/regressao que reprocessa genesis mais o primeiro bloco e afirma roots WSV identicos entre arm64/x86_64 (acompanhado em `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Tests de fixity cross-crate**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline e ABI em um unico harness.
   - Introduzir um scaffold `cargo xtask check-shape` que panic em schema drift (acompanhado no backlog de tooling DevEx; ver action item em `scripts/xtask/README.md`).

### Fase B - WSV e superficie de queries
1. **Transacoes de state storage**
   - Colapsar `state/storage_transactions.rs` em um adaptador transacional que imponha ordenacao de commit e deteccao de conflitos.
   - Unit tests agora verificam que modificacoes de assets/world/triggers fazem rollback em falhas.
2. **Refator do modelo de queries**
   - Mover a logica de paginacao/cursor para componentes reutilizaveis em `crates/iroha_core/src/query/`. Alinhar representacoes Norito em `iroha_data_model`.
   - Adicionar snapshot queries para triggers, assets e roles com ordenacao determinista (acompanhado via `crates/iroha_core/tests/snapshot_iterable.rs` para a cobertura atual).
3. **Consistencia de snapshots**
   - Garantir que o CLI `iroha ledger query` use o mesmo caminho de snapshot que Sumeragi/fetchers.
   - Tests de regressao de snapshot no CLI vivem em `tests/cli/state_snapshot.rs` (feature-gated para runs lentos).

### Fase C - Pipeline Sumeragi
1. **Topologia e gestao de epocas**
   - Extrair `EpochRosterProvider` para um trait com implementacoes baseadas em snapshots de stake WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` oferece um construtor simples e amigavel para mocks em benches/tests.
2. **Simplificacao do fluxo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` em modulos: `pacemaker`, `aggregation`, `availability`, `witness` com tipos compartilhados sob `consensus`.
   - Substituir message passing ad-hoc por envelopes Norito tipados e introduzir property tests de view-change (acompanhado no backlog de mensageria Sumeragi).
3. **Integracao lane/proof**
   - Alinhar lane proofs com commitments de DA e garantir que o gating de RBC seja uniforme.
   - O teste de integracao end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs` agora verifica o caminho com RBC habilitado.

### Fase D - Smart contracts e hosts pointer-ABI
1. **Auditoria da fronteira do host**
   - Consolidar verificacoes de pointer-type (`ivm::pointer_abi`) e adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - As expectativas de pointer table e os bindings de host manifest sao cobertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que exercitam os mappings TLV golden.
2. **Sandbox de execucao de triggers**
   - Refatorar triggers para rodar via um `TriggerExecutor` comum que impoe gas, validacao de pointers e journaling de eventos.
   - Adicionar tests de regressao para triggers de call/time cobrindo paths de falha (acompanhado via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alinhamento de CLI e client**
   - Garantir que operacoes do CLI (`audit`, `gov`, `sumeragi`, `ivm`) usem funcoes compartilhadas do cliente `iroha` para evitar drift.
   - Tests de snapshots JSON do CLI vivem em `tests/cli/json_snapshot.rs`; mantenha-os atualizados para que a saida dos comandos continue alinhada a referencia JSON canonica.

### Fase E - Hardening do codec Norito
1. **Registro de schemas**
   - Criar um registro de schema Norito em `crates/norito/src/schema/` para fornecer encodings canonicos para tipos core.
   - Adicionados doc tests verificando a codificacao de payloads de exemplo (`norito::schema::SamplePayload`).
2. **Refresh de golden fixtures**
   - Atualizar os golden fixtures em `crates/norito/tests/*` para coincidir com o novo schema WSV quando o refactor pousar.
   - `scripts/norito_regen.sh` regenera os golden JSON Norito de forma deterministica via o helper `norito_regen_goldens`.
3. **Integracao IVM/Norito**
   - Validar a serializacao de manifests Kotodama end-to-end via Norito, garantindo que a metadata pointer ABI seja consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantem paridade Norito encode/decode para manifests.

## 3. Preocupacoes transversais
- **Estrategia de testes**: Cada fase promove unit tests -> crate tests -> integration tests. Tests que falham capturam regressoes atuais; novos tests impedem que retornem.
- **Documentacao**: Apos cada fase, atualizar `status.md` e levar itens em aberto para `roadmap.md` enquanto remove tarefas concluidas.
- **Benchmarks de performance**: Manter benches existentes em `iroha_core`, `ivm` e `norito`; adicionar medicoes base pos-refactor para validar que nao ha regressoes.
- **Feature flags**: Manter toggles em nivel de crate apenas para backends que exigem toolchains externos (`cuda`, `zk-verify-batch`). Paths SIMD de CPU sao sempre construidos e selecionados em runtime; fornecer fallbacks escalares deterministas para hardware nao suportado.

## 4. Acoes imediatas
- Scaffolding da Fase A (snapshot trait + wiring de telemetria) - ver tarefas acionaveis nas atualizacoes do roadmap.
- A auditoria recente de defeitos para `sumeragi`, `state` e `ivm` revelou os seguintes destaques:
  - `sumeragi`: allowances de dead-code protegem o broadcast de provas de view-change, estado de replay VRF e exportacao de telemetria EMA. Eles permanecem gated ate que a simplificacao do fluxo de consenso da Fase C e os entregaveis de integracao lane/proof sejam entregues.
  - `state`: a limpeza de `Cell` e o roteamento de telemetria entram na trilha de telemetria WSV da Fase A, enquanto as notas de SoA/parallel-apply entram no backlog de otimizacao de pipeline da Fase C.
  - `ivm`: exposicao do toggle CUDA, validacao de envelopes e cobertura Halo2/Metal mapeiam para o trabalho de host-boundary da Fase D mais o tema transversal de aceleracao GPU; kernels permanecem no backlog dedicado de GPU ate estarem prontos.
- Preparar um RFC cross-team resumindo este plano para sign-off antes de aplicar mudancas invasivas de codigo.

## 5. Questoes em aberto
- RBC deve permanecer opcional alem de P1, ou e obrigatorio para lanes do ledger Nexus? Requer decisao de stakeholders.
- Impomos grupos de composabilidade DS em P1 ou os mantemos desativados ate que as lane proofs amadurecam?
- Qual e o local canonico para parametros ML-DSA-87? Candidato: novo crate `crates/fastpq_isi` (criacao pendente).

---

_Ultima atualizacao: 2025-09-12_
