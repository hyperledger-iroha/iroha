---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de refatoração do nexo
title: Plano de refatoração do ledger Sora Nexus
description: Espelho de `docs/source/nexus_refactor_plan.md`, detalhando o trabalho de limpeza por fases para a base de código do Iroha 3.
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_refactor_plan.md`. Mantenha as duas cópias homologadas até que a educação multilíngue chegue ao portal.
:::

# Plano de refatoração do razão Sora Nexus

Este documento captura o roadmap imediatamente para o refator do Sora Nexus Ledger ("Iroha 3"). Ele reflete o layout atual do repositório e os regressos observados em genesis/WSV bookkeeping, consenso Sumeragi, triggers de contratos inteligentes, consultas de snapshots, bindings de host pointer-ABI e codecs Norito. O objetivo e convergir para uma arquitetura consistente e testada sem tentar entregar todas as correções em um único patch monolítico.

## 0. Princípios guia
- Preservar comportamento determinista em hardware heterogêneo; usar aceleração apenas via feature flags opt-in com fallbacks idênticos.
- Norito e a camada de serialização. Qualquer mudança de estado/esquema deve incluir testes de ida e volta Norito encode/decode e atualizações de fixtures.
- A configuração flui por `iroha_config` (usuário -> atual -> padrões). Removedor alterna ad-hoc de ambiente dos caminhos de produção.
- A política ABI permanece V1 e inegociavel. Hosts rejeitam tipos de ponteiro/syscalls desconhecidos de forma determinística.
- `cargo test --workspace` e os testes de ouro (`ivm`, `norito`, `integration_tests`) continuam como base de portão para cada marco.

## 1. Instantâneo da topologia do repositório
- `crates/iroha_core`: atores Sumeragi, WSV, loader de genesis, pipelines (query, overlay, zk lanes), cola do host de contratos inteligentes.
- `crates/iroha_data_model`: esquema autoritativo para dados e consultas on-chain.
- `crates/iroha`: API de cliente usada por CLI, testes, SDK.
- `crates/iroha_cli`: CLI de operadores, atualmente espelha inúmeras APIs em `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, pontos de entrada de integração pointer-ABI do host.
- `crates/norito`: codec de serialização com adaptadores JSON e backends AoS/NCB.
- `integration_tests`: assertions cross-component cobrindo genesis/bootstrap, Sumeragi, triggers, paginacao, etc.
- Docs já delineiam metas do Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mas a implementação está fragmentada e parcialmente defasada em relação ao código.

## 2. Pilares de refatoração e marcos

### Fase A - Fundamentos e observabilidade
1. **Telemetria WSV + Instantâneos**
   - Estabelecer uma API canônica de snapshots em `state` (trait `WorldStateSnapshot`) usada por queries, Sumeragi e CLI.
   - Usar `scripts/iroha_state_dump.sh` para produzir snapshots deterministas via `iroha state dump --format norito`.
2. **Determinismo de Genesis/Bootstrap**
   - Refatorar uma ingestão de genesis para fluir por um único pipeline com Norito (`iroha_core::genesis`).
   - Adicionar cobertura de integração/regressão que reprocessa genesis mais o primeiro bloco e afirma raízes WSV idênticas entre arm64/x86_64 (acompanhado em `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Testes de fixidez entre caixas**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline e ABI em um chicote único.
   - Introduzindo um scaffold `cargo xtask check-shape` que entra em pânico em esquema drift (acompanhado no backlog de tooling DevEx; ver action item em `scripts/xtask/README.md`).

### Fase B - WSV e superfície de consultas
1. **Transações de armazenamento estadual**
   - Colapsar `state/storage_transactions.rs` em um adaptador transacional que impõe ordenação de commit e detecção de conflitos.
   - Testes unitários agora verificam que modificações de assets/world/triggers fazem rollback em falhas.
2. **Refator do modelo de consultas**
   - Mover a lógica de paginação/cursor para componentes reutilizáveis em `crates/iroha_core/src/query/`. Alinhar representações Norito em `iroha_data_model`.
   - Adicionar consultas instantâneas para gatilhos, ativos e funções com ordenação determinista (acompanhado via `crates/iroha_core/tests/snapshot_iterable.rs` para a cobertura atual).
3. **Consistência de snapshots**
   - Garantir que o CLI `iroha ledger query` use o mesmo caminho de snapshot que Sumeragi/fetchers.
   - Testes de regressão de snapshot no CLI vivem em `tests/cli/state_snapshot.rs` (feature-gated para execução lenta).### Fase C - Pipeline Sumeragi
1. **Topologia e gestão de épocas**
   - Extrair `EpochRosterProvider` para um trait com implementações baseadas em snapshots de stake WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` oferece um construtor simples e amigável para simulações em bancadas/testes.
2. **Simplificação do fluxo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` em módulos: `pacemaker`, `aggregation`, `availability`, `witness` com tipos compartilhados sob `consensus`.
   - Substituir mensagem passando ad-hoc por envelopes Norito tipados e inserir testes de propriedade de view-change (acompanhado no backlog de mensagem Sumeragi).
3. **Pista de integração/prova**
   - Alinhar provas de pista com compromissos de DA e garantir que o gating de RBC seja uniforme.
   - O teste de integração ponta a ponta `integration_tests/tests/extra_functional/seven_peer_consistency.rs` agora verifica o caminho com RBC habilitado.

### Fase D - Contratos inteligentes e hosts ponteiro-ABI
1. **Auditoria da fronteira do anfitrião**
   - Consolidar verificações do tipo ponteiro (`ivm::pointer_abi`) e adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - As expectativas de tabela de ponteiros e as ligações de manifesto de host são cobertas por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que exercitam os mapeamentos TLV dourados.
2. **Sandbox de execução de gatilhos**
   - Refatorar triggers para rodar via um `TriggerExecutor` comum que impoe gás, validação de ponteiros e registro de eventos.
   - Adicionar testes de regressão para triggers de chamada/tempo cobrindo caminhos de falha (acompanhados via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alinhamento de CLI e cliente**
   - Garantir que as operações do CLI (`audit`, `gov`, `sumeragi`, `ivm`) usem funções compartilhadas do cliente `iroha` para evitar desvios.
   - Testes de snapshots JSON do CLI vivem em `tests/cli/json_snapshot.rs`; atualize-os atualizados para que a dita dos comandos continue homologada a referência JSON canonica.

### Fase E - Endurecimento do codec Norito
1. **Registro de esquemas**
   - Crie um registro de esquema Norito em `crates/norito/src/schema/` para fornecer codificações canônicas para tipos core.
   - Adicionados testes de documentação verificando a codificação de payloads de exemplo (`norito::schema::SamplePayload`).
2. **Atualizar os acessórios dourados**
   - Atualizar os golden fixtures em `crates/norito/tests/*` para coincidir com o novo esquema WSV quando o refatorar plantados.
   - `scripts/norito_regen.sh` regenera o golden JSON Norito de forma determinística via o helper `norito_regen_goldens`.
3. **Integração IVM/Norito**
   - Validar a serialização dos manifestos Kotodama ponta a ponta via Norito, garantindo que um ponteiro de metadados ABI seja consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantem paridade Norito codifica/decodifica para manifestos.

## 3. Preocupações transversais
- **Estratégia de testes**: Cada fase promove testes unitários -> testes de caixa -> testes de integração. Testes que falham capturam regressos atuais; novos testes impedem que retornem.
- **Documentacao**: Após cada fase, atualize `status.md` e leve itens em aberto para `roadmap.md` enquanto remove tarefas concluídas.
- **Benchmarks de desempenho**: Manter bancos existentes em `iroha_core`, `ivm` e `norito`; adicionar medicamentos base pos-refactor para validar que não há regressos.
- **Sinalizadores de recursos**: Manter alterna o nível da caixa apenas para back-ends que desativam cadeias de ferramentas externas (`cuda`, `zk-verify-batch`). Paths SIMD de CPU são sempre construídos e selecionados em tempo de execução; fornecer substitutos escalares deterministas para hardware não suportado.## 4. Ações Imediatas
- Andaime da Fase A (traço instantâneo + fiação de telemetria) - ver tarefas acionaveis nas atualizações do roadmap.
- Os recentes auditorias de defeitos para `sumeragi`, `state` e `ivm` revelaram os seguintes destaques:
  - `sumeragi`: permissões de dead-code protegem o broadcast de provas de view-change, estado de replay VRF e exportação de telemetria EMA. Eles permanecem fechados até que a simplificação do fluxo de consenso da Fase C e as entregas de via de integração/prova sejam entregues.
  - `state`: a limpeza de `Cell` e o roteamento de telemetria entram na trilha de telemetria WSV da Fase A, enquanto as notas de SoA/parallel-apply entram no backlog de otimização de pipeline da Fase C.
  - `ivm`: exposição do toggle CUDA, validação de envelopes e cobertura Halo2/Metal mapeiam para o trabalho de host-boundary da Fase D mais o tema transversal de aceleração GPU; kernels permaneceram no backlog dedicado de GPU ainda estavam prontos.
- Preparar um RFC cross-team resumindo este plano para aprovação antes de aplicar mudanças invasivas de código.

## 5. Perguntas em aberto
- RBC deve permanecer opcional além de P1, ou é obrigatório para as pistas do razão Nexus? Solicitar decisão das partes interessadas.
- Impomos grupos de composabilidade DS em P1 ou os mantemos desativados até que as lane proofs amadurecam?
- Qual e o canônico local para parâmetros ML-DSA-87? Candidato: novo crate `crates/fastpq_isi` (criação pendente).

---

_Última atualização: 2025-09-12_