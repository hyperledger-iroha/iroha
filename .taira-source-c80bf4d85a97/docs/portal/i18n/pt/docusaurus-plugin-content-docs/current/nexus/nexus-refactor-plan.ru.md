---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de refatoração do nexo
título: Plano de refatoração Sora Nexus Ledger
descrição: Зеркало `docs/source/nexus_refactor_plan.md`, описывающее поэтапную зачистку кодовой базы Iroha 3.
---

:::nota História Canônica
Esta página é `docs/source/nexus_refactor_plan.md`. Selecione uma cópia sincronizada, pois muitas versões não estão disponíveis no portal.
:::

# Plano de refatoração Sora Nexus Ledger

Este documento é um roteiro de atualização para Sora Nexus Ledger ("Iroha 3"). Em uma estrutura de estrutura de repositório e registro construída, crie um nome no genesis/WSV, conversão Sumeragi, acionadores inteligentes, consultas de snapshot, ponteiro de ligações de host-ABI e codecs Norito. Цель - прийти к согласованной, тестируемой архитектуре, не пытаясь посадить все исправления одним монолитным патчем.

## 0. Princípios de execução
- Сохранять детерминированное поведение на гетерогенном железе; É possível usar sinalizadores de recurso de aceitação com substitutos de identificação.
- Norito - número serial. A configuração de estado/esquema é definida como Norito codifica/decodifica testes de ida e volta e fixa fixtures.
- Конфигурация проходит через `iroha_config` (usuário -> real -> padrões). Use alternadores de ambiente ad-hoc no produto produzido.
- Política ABI instalada V1 e não instalada. Hosts não determinam tipos de ponteiro/syscalls.
- `cargo test --workspace` e testes de ouro (`ivm`, `norito`, `integration_tests`) остаются базовым гейтом для каждого этапа.

## 1. Repositório de Topologia de Снимок
- `crates/iroha_core`: atores Sumeragi, WSV, carregador genesis, pipelines (query, overlay, zk lanes), cola para host de contrato inteligente.
- `crates/iroha_data_model`: consultas e consultas automáticas na cadeia.
- `crates/iroha`: API do cliente, CLI de uso, testes, SDK.
- `crates/iroha_cli`: CLI de operação, seleciona muitas APIs de `iroha`.
- `crates/ivm`: VM de bytecode Kotodama, pontos de entrada para integração de host ponteiro-ABI.
- `crates/norito`: codec de serialização com adaptadores JSON e backends AoS/NCB.
- `integration_tests`: asserções de codificação cruzada, configuração de gênese/bootstrap, Sumeragi, gatilhos, paginação, etc.
- Docs уже описывают цели Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), sem fragmentação realizável e частично устарела относительно кода.

## 2. Opções de refatoração e visualização de elementos

### Fase A - Fundações e Observabilidade
1. **Telemetria WSV + Instantâneos**
   - API de snapshot canônico em `state` (característica `WorldStateSnapshot`), consultas de uso, Sumeragi e CLI.
   - Use `scripts/iroha_state_dump.sh` para determinar instantâneos do `iroha state dump --format norito`.
2. **Gênesis/Determinismo Bootstrap**
   - Переписать ingest genesis так, чтобы он проходил через единый pipeline Norito (`iroha_core::genesis`).
   - Добавить integração/regressão покрытие, которое воспроизводит genesis плюс первый блок и подтверждает идентичные raízes WSV между arm64/x86_64 (transferido em `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Testes de fixação entre caixas**
   - Расширить `integration_tests/tests/genesis_json.rs`, ele valida WSV, pipeline e invariantes ABI no chicote externo.
   - Use o andaime `cargo xtask check-shape`, que entrará em pânico com o desvio de esquema (tornado no backlog de ferramentas DevEx; item de ação em `scripts/xtask/README.md`).

### Fase B - WSV e superfície de consulta
1. **Transações de armazenamento estatal**
   - Use o adaptador de transação `state/storage_transactions.rs` para que ele execute o commit e detecte os conectores.
   - Testes de unidade são testados, e os ativos/mundo/triggers são modificados antes de serem executados.
2. **Refator do modelo de consulta**
   - Altere a lógica de paginação/cursor nos componentes de configuração em `crates/iroha_core/src/query/`. Согласовать Representações Norito em `iroha_data_model`.
   - Faça consultas de instantâneo para gatilhos, ativos e funções com um propósito determinado (ele é definido como `crates/iroha_core/tests/snapshot_iterable.rs` para a configuração correta).
3. **Consistência do Instantâneo**
   - Use o `iroha ledger query` CLI usado para o caminho do snapshot, este e Sumeragi/fetchers.
   - Testes de regressão de instantâneo CLI realizados em `tests/cli/state_snapshot.rs` (recursos limitados para programas integrados).### Fase C - Pipeline Sumeragi
1. **Topologia e gerenciamento de época**
   - Verifique `EpochRosterProvider` em trait с реализациями, основанными на WSV stake snapshots.
   - `WsvEpochRosterAdapter::from_peer_iter` é um construtor de projeto, usado para simulação em bancadas/testes.
2. **Simplificação do Fluxo de Consenso**
   - Organize `crates/iroha_core/src/sumeragi/*` nos módulos: `pacemaker`, `aggregation`, `availability`, `witness` com tipos ideais em `consensus`.
   - Aumentar a passagem de mensagens ad-hoc em envelopes Norito típicos e realizar testes de propriedade de alteração de visualização (transferidos no backlog de mensagens Sumeragi).
3. **Integração de pista/prova**
   - Согласовать provas de pista com compromissos DA e controle de controle RBC.
   - Teste de integração de ponta a ponta `integration_tests/tests/extra_functional/seven_peer_consistency.rs` é testado pelo RBC.

### Fase D - Contratos inteligentes e hosts Pointer-ABI
1. **Auditoria de limite do anfitrião**
   - Consolar provedores do tipo ponteiro (`ivm::pointer_abi`) e adaptadores host (`iroha_core::smartcontracts::ivm::host`).
   - Organize a tabela de ponteiros e as ligações de manifesto do host usando os testes `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que provam mapeamentos TLV dourados.
2. **Sandbox de execução de gatilho**
   - Перефакторить triggers, чтобы они выполнялись через общий `TriggerExecutor`, который применяет gás, validação de ponteiro e registro de eventos.
   - Faça testes de regressão para gatilhos de chamada/tempo com caminhos de falha de identificação (transferência `crates/iroha_core/tests/trigger_failure.rs`).
3. **CLI e alinhamento do cliente**
   - Обеспечить, чтобы CLI операции (`audit`, `gov`, `sumeragi`, `ivm`) instalado cliente funções `iroha`, isso elimina o desvio.
   - Testes de snapshot CLI JSON executados em `tests/cli/json_snapshot.rs`; Na verdade, isso significa que seu comando principal é compatível com referência JSON canônica.

### Fase E - Endurecimento do Codec Norito
1. **Registro de esquema**
   - Создать Norito registro de esquema em `crates/norito/src/schema/`, чтобы задавать codificações canônicas para tipos de dados principais.
   - Desenvolvimento de testes de documentos, codificação de cargas úteis de amostra (`norito::schema::SamplePayload`).
2. **Atualização de luminárias douradas**
   - Обновить luminárias douradas em `crates/norito/tests/*`, чтобы они соответствовали novo esquema WSV после посадки рефакторинга.
   - `scripts/norito_regen.sh` детерминированно регенерирует Norito JSON goldens через ajudante `norito_regen_goldens`.
3. **Integração IVM/Norito**
   - Провалидировать сериализацию Kotodama manifesto ponta a ponta через Norito, гарантируя консистентность ponteiro metadados ABI.
   - `crates/ivm/tests/manifest_roundtrip.rs` удерживает Norito codifica/decodifica parte para manifestos.

## 3. Сквозные вопросы
- **Estratégia de teste**: Каждый этап продвигает testes unitários -> testes de caixa -> testes de integração. Падающие тесты фиксируют текущие регрессии; novos testes não foram realizados.
- **Documentação**: После посадки каждой фазы обновлять `status.md` e переносить незакрытые пункты в `roadmap.md`, удаляя завершенные задачи.
- **Benchmarks de desempenho**: Сохранять существующие bancos em `iroha_core`, `ivm` e `norito`; Você pode usar uma configuração de refatoração para poder abrir a registro.
- **Sinalizadores de recursos**: Alterne alternadores de nível de caixa para back-ends e conjuntos de ferramentas adicionais (`cuda`, `zk-verify-batch`). CPU SIMD пути всегда собираются и выбираются во время выполнения; обеспечить детерминированные fallbacks escalares para неподдерживаемого железа.

## 4. Destino de destino
- Andaime da Fase A (traço instantâneo + fiação de telemetria) - см. tarefas acionáveis ​​​​no roteiro de обновлениях.
- Quaisquer defeitos de auditoria em `sumeragi`, `state` e `ivm` serão exibidos nos momentos seguintes:
  - `sumeragi`: permissões de código morto para prova de alteração de visualização de transmissão, estado de repetição VRF e exportação de telemetria EMA. Se você estiver fechado, não há resultados disponíveis na Fase C para melhorar o fluxo de consenso e integrar a pista/prova.
  - `state`: limpeza `Cell` e roteamento de telemetria executados na trilha de telemetria WSV da Fase A, e um backlog de otimização de pipeline da Fase C com SoA/aplicação paralela.
  - `ivm`: alternância CUDA de comutação, validação de envelope e mapa de cobertura Halo2/Metal na fronteira do host da Fase D, além de acelerar a GPU; kernels são armazenados no backlog da GPU fora do lugar.
- Verifique o RFC de equipe cruzada com a configuração deste plano para assinatura antes do código de inicialização.## 5. Abrir itens
- Você está no RBC para usar a posição P1 ou está trabalhando nas pistas do razão Nexus? Требуется решение стейкхолдеров.
- Quais são os grupos de composição do DS em P1 ou quais são as provas de pista?
- Qual é a melhor opção para parâmetros ML-DSA-87? Candidato: caixa nova `crates/fastpq_isi` (removida).

---

_Atualização: 12/09/2025_