---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de refatoração do nexo
título: Plano de refatoração do razão Sora Nexus
descrição: Espelho de `docs/source/nexus_refactor_plan.md`, detalhando o trabalho de limpeza por fases para a base de código Iroha 3.
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_refactor_plan.md`. Gardez les duas cópias alinhadas até que a edição multilíngue chegue ao portal.
:::

# Plano de refatoração do razão Sora Nexus

Este documento captura o roteiro imediatamente após a refatoração do Sora Nexus Ledger ("Iroha 3"). Ele reflete a topologia atual do depósito e as regressões observadas no comptabilite genesis/WSV, o consenso Sumeragi, os gatilhos de contratos inteligentes, as solicitações de snapshots, as ligações de host pointer-ABI e os codecs Norito. O objetivo é convergir para uma arquitetura coerente e testável sem tentar liberar todas as correções em um patch monolítico.

## 0. Príncipes diretores
- Preservar um comportamento determinado sobre o material heterogêneo; use a aceleração exclusivamente por meio dos sinalizadores de recurso ativados com substitutos idênticos.
- Norito é o sofá de serialização. Todas as alterações de estado/esquema incluem testes de ida e volta Norito, codificação/decodificação e erros no dia a dia.
- La configuração transite par `iroha_config` (usuário -> real -> padrões). Suprima as alternâncias de ambiente ad hoc dos caminhos de produção.
- La politique ABI reste V1 e não negociável. Os hosts devem rejeitar a determinação dos tipos de ponteiro/syscalls sem conhecimento de causa.
- `cargo test --workspace` e os testes de ouro (`ivm`, `norito`, `integration_tests`) restam o portão de base para cada jalon.

## 1. Instantâneo da topologia do depósito
- `crates/iroha_core`: atores Sumeragi, WSV, loader genesis, pipelines (query, overlay, zk lanes), cola host de contratos inteligentes.
- `crates/iroha_data_model`: esquema autorizado para dados e solicitações na cadeia.
- `crates/iroha`: cliente API utilizado por CLI, testes, SDK.
- `crates/iroha_cli`: operação CLI, reflete a atualidade dos nomes de APIs em `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, pontos de entrada do ponteiro de integração-ABI do host.
- `crates/norito`: codec de serialização com adaptadores JSON e backends AoS/NCB.
- `integration_tests`: asserções cross-component couvrant genesis/bootstrap, Sumeragi, gatilhos, paginação, etc.
- Os documentos descrevem os objetivos do Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mas a implementação é fragmentada e parte obsoleta por relatório de código.

## 2. Pilares de refatoração e grades

### Fase A - Fundações e observabilidade
1. **Telemetria WSV + Instantâneos**
   - Defina uma API canônica de snapshots em `state` (trait `WorldStateSnapshot`) usada por consultas, Sumeragi e CLI.
   - Use `scripts/iroha_state_dump.sh` para produzir instantâneos determinados via `iroha state dump --format norito`.
2. **Determinismo Gênesis/Bootstrap**
   - Refatorar a gênese da ingestão para passar por um pipeline exclusivo com base em Norito (`iroha_core::genesis`).
   - Adicionar uma cobertura de integração/regressão que rejue genesis plus le premier bloc et verifique des racines WSV idênticos entre arm64/x86_64 (suivi em `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Testes de fixidez entre caixas**
   - Crie `integration_tests/tests/genesis_json.rs` para validar as invariantes WSV, pipeline e ABI em seu próprio chicote.
   - Introduza um scaffold `cargo xtask check-shape` que entra em pânico no desvio de esquema (a seguir no backlog DevEx Tooling; veja o item de ação em `scripts/xtask/README.md`).

### Fase B - WSV e superfície de pacotes
1. **Transações de armazenamento de estado**
   - Collapser `state/storage_transactions.rs` em um adaptador transacional que aplica a ordem de confirmação e a detecção de conflitos.
   - Os testes de unidade verificam desormais que as modificações de ativos/mundo/acionam a reversão de fontes em caso de echec.
2. **Refator do modelo de solicitações**
   - Substitua a lógica de paginação/cursor nos componentes reutilizáveis sob `crates/iroha_core/src/query/`. Alinhe as representações Norito em `iroha_data_model`.
   - Adicionar consultas de instantâneo para gatilhos, ativos e funções com uma ordem determinada (suivi via `crates/iroha_core/tests/snapshot_iterable.rs` para a cobertura atual).
3. **Consistência dos instantâneos**
   - Certifique-se de que o CLI `iroha ledger query` utiliza o meme do caminho de snapshot que Sumeragi/fetchers.
   - Os testes de regressão de snapshots CLI foram encontrados sob `tests/cli/state_snapshot.rs` (fechado por recurso para execução lenta).### Fase C - Pipeline Sumeragi
1. **Topologia e gestão de épocas**
   - Extraia `EpochRosterProvider` e trait com implementações baseadas em snapshots de stake WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` fornece um construtor simples e amigável para simulações em bancadas/testes.
2. **Simplificação do fluxo de consenso**
   - Reorganizador `crates/iroha_core/src/sumeragi/*` nos módulos: `pacemaker`, `aggregation`, `availability`, `witness` com os tipos de partes sob `consensus`.
   - Substitua a mensagem que passa ad-hoc pelos envelopes Norito e introduza os testes de propriedade de view-change (a seguir no backlog de mensagens Sumeragi).
3. **Faixa/prova de integração**
   - Alinhe as provas de pista com os compromissos DA e garanta um uniforme RBC.
   - O teste de integração ponta a ponta `integration_tests/tests/extra_functional/seven_peer_consistency.rs` verifica a manutenção do caminho com RBC ativo.

### Fase D - Contratos inteligentes e hosts ponteiro-ABI
1. **Auditoria de la frontiere host**
   - Consolidar verificações do tipo ponteiro (`ivm::pointer_abi`) e adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - As verificações da tabela de ponteiros e as ligações do manifesto do host são cobertas por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que exercem os mapeamentos TLV dourados.
2. **Sandbox de execução de gatilhos**
   - Refatorar gatilhos para passar por um `TriggerExecutor` comum com gás de aplicação, validação de ponteiros e registro em diário de eventos.
   - Adiciona testes de regressão para gatilhos de chamada/tempo que cobrem os caminhos de execução (suivi via `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alinhamento CLI e cliente**
   - Certifique-se de que as operações CLI (`audit`, `gov`, `sumeragi`, `ivm`) são representadas nas funções do cliente compartilhadas `iroha` para evitar desvios.
   - Os testes de snapshots JSON da CLI vivem em `tests/cli/json_snapshot.rs`; Gardez-les a jour para que a classificação principal continue correspondendo à referência JSON canônica.

### Fase E - Duração do codec Norito
1. **Registro de esquemas**
   - Crie um registro de esquemas Norito sob `crates/norito/src/schema/` para gerar codificações canônicas de tipos principais.
   - Adicione os testes do documento para verificar a codificação das cargas úteis do exemplo (`norito::schema::SamplePayload`).
2. **Atualize os acessórios dourados**
   - Mettre a jour les golden fixtures `crates/norito/tests/*` para corresponder ao novo esquema WSV um foi o refator livre.
   - `scripts/norito_regen.sh` regenera o JSON dourado Norito do ícone determinado por meio do auxiliar `norito_regen_goldens`.
3. **Integração IVM/Norito**
   - Validar a serialização dos manifestos Kotodama ponta a ponta via Norito, garantindo um ponteiro de metadados ABI coerente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantém a parte Norito codificada/decodificada para os manifestos.

## 3. Preocupações transversais
- **Estratégia de testes**: Testes de unidade realizados em cada fase -> testes de caixa -> testes de integração. Os testes em echec capturam as regressões atuais; os novos testes evitam seu retorno.
- **Documentação**: Após cada fase, coloque um dia `status.md` e relate os itens abertos em `roadmap.md` e suprima os taches terminados.
- **Benchmarks de desempenho**: Manter os bancos existentes em `iroha_core`, `ivm` e `norito`; adiciona medidas de base pós-refator para validar a ausência de regressões.
- **Sinalizadores de recursos**: Mantenha as alternâncias no nível da caixa exclusivamente para back-ends que exigem cadeias de ferramentas externas (`cuda`, `zk-verify-batch`). Os caminhos da CPU SIMD são sempre construídos e selecionados para execução; fornecer substitutos escalares determinísticos para materiais não suportados.## 4. Ações imediatas
- Andaime da Fase A (característica de instantâneo + telemetria de fiação) - veja os taches acionáveis ​​nas mises a jour du roadmap.
- A auditoria recente dos padrões para `sumeragi`, `state` e `ivm` revela os seguintes pontos:
  - `sumeragi`: as permissões de código morto protegem a transmissão de pré-mudança de visualização, o estado de replay VRF e a exportação de telemetria EMA. As portas restantes apenas fazem com que a simplificação do fluxo de consenso da Fase C e as soluções de integração/prova sejam livres.
  - `state`: a rede de `Cell` e a rota de telemetria passam pela pista de telemetria WSV da Fase A, enquanto as notas SoA/parallel-apply são basculentas no backlog de otimização do pipeline da Fase C.
  - `ivm`: a exposição de alternância CUDA, a validação de envelopes e a cobertura Halo2/Metal são mapeadas no trabalho host-boundary da Fase D mais o tema transversal de aceleração GPU; Os kernels permanecem no backlog da GPU apenas até o final.
- Preparar um resumo de equipe cruzada RFC deste plano para aprovação antes de liberar alterações de código invasivo.

## 5. Perguntas abertas
- RBC doit-il rester optionnel apres P1, ou é obrigatório para as pistas do razão Nexus? Decisão das partes prenantes requeridas.
- Doit-on impor des grupos de composabilite DS em P1 ou les laisser desactives jusqu'a maturite des lane proofs?
- Qual é a localização canônica dos parâmetros ML-DSA-87? Candidato: nouveau crate `crates/fastpq_isi` (criação atenta).

---

_Derniere mise a jour: 2025-09-12_