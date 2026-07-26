---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de refatoração do nexo
título: Plano de refatorização do razão Sora Nexus
description: Espejo de `docs/source/nexus_refactor_plan.md`, que detalha o trabalho de limpeza por fases para a base de código de Iroha 3.
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_refactor_plan.md`. Mantenha ambas as cópias alinhadas até que a edição multilíngue chegue ao portal.
:::

# Plano de refatoração do razão Sora Nexus

Este documento captura o roteiro imediatamente para a refatoração do Sora Nexus Ledger ("Iroha 3"). Reflita sobre o layout atual do repositório e as regiões observadas na contabilidade do genesis/WSV, o consenso Sumeragi, gatilhos de contratos inteligentes, consultas de snapshots, ligações de ponteiro de host-ABI e codecs Norito. O objetivo é convergir para uma arquitetura coerente e provável, sem tentar aterrizar todas as correções em uma única área monolítica.

## 0. Princípios guia
- Preservar o comportamento determinista em hardware heterogêneo; usar aceleração apenas com feature flags opt-in e fallbacks idênticos.
- Norito é a capacidade de serialização. Qualquer mudança de estado/esquema deve incluir testes de codificação/decodificação Norito de ida e volta e atualizações de fixtures.
- A configuração flui por `iroha_config` (usuário -> real -> padrões). Eliminar alterna ad-hoc de ambiente nos caminhos de produção.
- A política ABI segue em V1 e não é negociável. Os hosts devem rechazar tipos de ponteiro/syscalls desconhecidos de forma determinista.
- `cargo test --workspace` e os testes de ouro (`ivm`, `norito`, `integration_tests`) siguen siguen la compuerta base para cada hito.

## 1. Instantâneo da topologia do repositório
- `crates/iroha_core`: atores Sumeragi, WSV, carregador de gênese, pipelines (query, overlay, zk lanes), cola do host de contratos inteligentes.
- `crates/iroha_data_model`: esquema autoritativo para dados e consultas on-chain.
- `crates/iroha`: API de cliente usada por CLI, testes, SDK.
- `crates/iroha_cli`: CLI de operadores, atualmente refletindo inúmeras APIs em `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, pontos de entrada de ponteiro de integração-ABI do host.
- `crates/norito`: codec de serialização com adaptadores JSON e backends AoS/NCB.
- `integration_tests`: asserções cross-component que cubren genesis/bootstrap, Sumeragi, triggers, paginacion, etc.
- Os documentos delineiam metas do Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mas a implementação é fragmentada e parcialmente obsoleta em relação ao código.

## 2. Pilares de refatoração e resultados

### Fase A - Fundamentos e observabilidade
1. **Telemetria WSV + Instantâneos**
   - Estabeleça uma API canônica de snapshots em `state` (trait `WorldStateSnapshot`) usada por consultas, Sumeragi e CLI.
   - Usar `scripts/iroha_state_dump.sh` para produzir snapshots deterministas via `iroha state dump --format norito`.
2. **Determinismo de Genesis/Bootstrap**
   - Refatorar a ingestão de gênese para que passe por um pipeline único com Norito (`iroha_core::genesis`).
   - Agregar cobertura de integração/regressão que reprocessa genesis mas o primeiro bloco e afirma raízes WSV idênticas entre arm64/x86_64 (seguido em `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Testes de fixidez entre caixas**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline e ABI em um único chicote.
   - Introduzindo um andaime `cargo xtask check-shape` que causou pânico antes do desvio de esquema (seguido abaixo do backlog de ferramentas DevEx; veja o item de ação em `scripts/xtask/README.md`).

### Fase B - WSV e superfície de consultas
1. **Transações de armazenamento estatal**
   - Colapsar `state/storage_transactions.rs` em um adaptador transacional que impõe ordem de commits e detecção de conflitos.
   - Os testes de unidade agora verificam se as modificações de assets/world/triggers fizeram rollback antes de falhar.
2. **Refator do modelo de consulta**
   - Mover a lógica de paginação/cursor para componentes reutilizáveis abaixo de `crates/iroha_core/src/query/`. Representações lineares Norito e `iroha_data_model`.
   - Agregar consultas instantâneas para gatilhos, ativos e funções com ordem determinista (seguido via `crates/iroha_core/tests/snapshot_iterable.rs` para a cobertura real).
3. **Consistência de snapshots**
   - Certifique-se de que a CLI `iroha ledger query` use a mesma rota de snapshot que Sumeragi/fetchers.
   - Os testes de regressão de snapshots na CLI vivem em `tests/cli/state_snapshot.rs` (feature-gated para execução lenta).### Fase C - Pipeline Sumeragi
1. **Topologia e gestão de épocas**
   - Extrair `EpochRosterProvider` para uma característica com implementações respaldadas por snapshots de stake em WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` oferece um construtor simples e amigável para simulações em bancos/testes.
2. **Simplificação do fluxo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` nos módulos: `pacemaker`, `aggregation`, `availability`, `witness` com tipos compartilhados abaixo de `consensus`.
   - Substitua a mensagem que passa ad-hoc pelos envelopes Norito digitados e introduza testes de propriedade de view-change (seguidos no backlog da mensagem Sumeragi).
3. **Pista/prova de integração**
   - Provas de pista linear com compromissos de DA e garantia de que RBC gating sea uniforme.
   - O teste de integração ponta a ponta `integration_tests/tests/extra_functional/seven_peer_consistency.rs` agora verifica a rota com RBC habilitado.

### Fase D - Contratos inteligentes e hosts ponteiro-ABI
1. **Auditoria de limite do anfitrião**
   - Consolidar as verificações do tipo ponteiro (`ivm::pointer_abi`) e os adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - As expectativas da tabela de ponteiros e as ligações do manifesto do host estão cobertas por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que ejetam os mapeamentos TLV dourados.
2. **Sandbox de execução de gatilhos**
   - Refatorar gatilhos para executar através de um `TriggerExecutor` comum que impone gás, validação de ponteiros e registro em diário de eventos.
   - Agregar testes de regressão para triggers de chamada/tempo identificando caminhos de falha (seguido via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alinhamento de CLI e cliente**
   - Certifique-se de que as operações CLI (`audit`, `gov`, `sumeragi`, `ivm`) dependem das funções compartilhadas do cliente `iroha` para evitar desvios.
   - Os testes de snapshots JSON da CLI vivem em `tests/cli/json_snapshot.rs`; mantenha-os no dia para que a saída dos comandos coincida com a referência JSON canônica.

### Fase E - Resistência do codec Norito
1. **Registro de esquemas**
   - Crie um registro de esquema Norito abaixo de `crates/norito/src/schema/` para abastecer codificações canônicas de tipos principais.
   - Adicione testes de documentos que verificam a codificação de cargas úteis da exibição (`norito::schema::SamplePayload`).
2. **Atualização de luminárias douradas**
   - Atualizar os golden fixtures de `crates/norito/tests/*` para que coincidam com o novo esquema WSV quando você terminar o refator.
   - `scripts/norito_regen.sh` regenera o JSON dourado de Norito de forma determinista por meio do auxiliar `norito_regen_goldens`.
3. **Integração IVM/Norito**
   - Validar a serialização de manifestos Kotodama ponta a ponta via Norito, garantindo que o ponteiro de metadados ABI seja consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantém a paridade Norito codifica/decodifica para manifestos.

## 3. Temas transversais
- **Estratégia de testes**: Cada fase promove testes unitários -> testes de caixa -> testes de integração. Os testes falharam nas regressões capturadas atuais; os novos testes evitam que reapareçam.
- **Documentação**: Após cada fase, atualizar `status.md` e mover itens abertos para `roadmap.md` enquanto as tarefas podem ser concluídas.
- **Benchmarks de desempenho**: Manutenção de bancos existentes em `iroha_core`, `ivm` e `norito`; agregar medicamentos base pós-refator para validar que não há regressões.
- **Sinalizadores de recursos**: o Mantener alterna uma caixa nivelada apenas para back-ends que exigem cadeias de ferramentas externas (`cuda`, `zk-verify-batch`). Os caminhos SIMD da CPU sempre são construídos e selecionados em tempo de execução; proveer fallbacks escalares deterministas para hardware não suportado.## 4. Ações imediatas
- Andaime da Fase A (traço instantâneo + fiação de telemetria) - ver tarefas acionáveis ​​e atualizações do roteiro.
- Os auditórios recentes com defeitos para `sumeragi`, `state` e `ivm` revelam os seguintes pontos:
  - `sumeragi`: permissões de código morto protegem a transmissão de testes de mudança de visualização, estado de replay VRF e exportação de telemetria EMA. Esses permanecem fechados até que a simplificação do fluxo de consenso da Fase C e as entregas de via de integração/prova sejam aterradas.
  - `state`: a limpeza de `Cell` e o caminho de telemetria passam pela trilha de telemetria WSV da Fase A, enquanto as notas de SoA/parallel-apply se integram ao backlog de otimização do pipeline da Fase C.
  - `ivm`: a exposição de alternâncias CUDA, a validação de envelopes e a cobertura Halo2/Metal são mapeadas para o trabalho de fronteira de host da Fase D, mas o tema transversal de aceleração GPU; Os kernels permanecem no backlog dedicado da GPU até serem listados.
- Preparar uma equipe cruzada de RFC que retome este plano para assinar antes de aterrizar mudanças de código invasivo.

## 5. Perguntas abertas
- O RBC deve seguir o caminho opcional, mas o P1, ou é obrigatório para as pistas do razão Nexus? Requer decisão das partes interessadas.
- Impulsamos grupos de composibilidad DS em P1 ou os mantemos desativados até que amadurecemos as provas de pista?
- Qual é a localização canônica dos parâmetros ML-DSA-87? Candidato: nova caixa `crates/fastpq_isi` (criação pendente).

---

_Atualização final: 12/09/2025_