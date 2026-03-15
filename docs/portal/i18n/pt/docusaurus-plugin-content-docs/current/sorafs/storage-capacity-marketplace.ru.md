---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado de capacidade de armazenamento
título: Маркетплейс емкости хранения SoraFS
sidebar_label: Mercado de Mercadorias
descrição: Plano SF-2c para mercados de mercado, ordens de replicação, telemetria e ganchos de governança.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/storage_capacity_marketplace.md`. Selecione uma cópia da sincronização para ativar a documentação.
:::

# Маркетплейс емкости хранения SoraFS (черновик SF-2c)

O roteiro do ponto SF-2c é um mercado de compras, onde os provedores são contratados
декларируют коммитнутую емкость, solicitando pedidos de replicação e taxas de pagamento
distribuição proporcional. Este documento oferece entregas
para obter total confiança e разбивает em trilhas acionáveis.

##Céli

- Фиксировать обязательства provedores по емкости (общие байты, лимиты по lane, срок действия)
  na forma de teste, projeto para governança, transporte SoraNet e Torii.
- Распределять pins между provedores согласно заявленной емкости, participação e política-ограничениям,
  сохраняя детерминированное поведение.
- Измерять доставку хранения (успех репликации, uptime, provas целостности) и
  taxas de transferência.
- Предоставлять процессы revogação e disputa, чтобы нечестные fornecedores могли быть
  наказаны или удалены.

##Configurações Domésticas

| Concepção | Descrição | Entregável |
|--------|-------------|---------------------|
| `CapacityDeclarationV1` | Carga útil Norito, provedor de ID de identificação, bloco de perfil de fornecimento, GiB de transação, limites de pista, dicas de preços, compromisso de piquetagem e destino. | Selecione + validador em `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Инструкция, выпущенная governança, назначающая Manifesto CID одному или нескольким provedores, включая уровень избыточности и метрики SLA. | Esquema Norito, criado para Torii + API de conversão inteligente. |
| `CapacityLedger` | Registro on-chain/off-chain, отслеживающий активные декларации емкости, ordens de replicação, métrica производительности e накопление taxas. | O módulo smart-контракта ou o serviço de stub off-chain é fornecido com um snapshot determinado. |
| `MarketplacePolicy` | Governança política, participação mínima определяющая, требования аудита и кривые штрафов. | Estrutura de configuração em `sorafs_manifest` + governança de documentos. |

### Реализованные схемы (статус)

## Разбиение работ

### 1. Слой схем и реестра| Bem | Proprietário(s) | Nomeação |
|------|----------|-------|
| Selecione `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipe de Armazenamento / Governança | Use Norito; selecione a versão semântica e selecione os recursos. |
| Realize o módulo analisador + validador em `sorafs_manifest`. | Equipe de armazenamento | Обеспечить монотонные IDs, ограничения емкости, требования по estaca. |
| Расширить metadata реестра chunker значением `min_capacity_gib` para каждого профиля. | GT Ferramentaria | Para configurar o cliente, você precisa de um mínimo de esforço no hardware do perfil. |
| Consulte o documento `MarketplacePolicy`, descreva os guarda-corpos de admissão e as grades gráficas. | Conselho de Governança | Consulte os documentos sobre os padrões de política. |

#### Определения схем (реализованы)

- `CapacityDeclarationV1` фиксирует подписанные обязательства емкости para o provedor de каждого, включая канонические lida com chunker, ссылки на capacidades, limites opcionais para pista, dicas sobre preços, validade válida e metadados. Валидация обеспечивает ненулевой estaca, канонические alças, дедуплицированные aliases, caps по lane в пределах заявленного total и монотонный учет GiB.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` связывает manifestos com назначениями, выпущенными governança, с целями избыточности, порогами SLA и гарантиями на atribuição; validadores обеспечивают канонические lida com chunker, provedores exclusivos e ограничения по prazo para того, como Torii ou registro примут ordem.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` описывает snapshots эпох (заявленные vs использованные GiB, счетчики репликации, проценты uptime/PoR), которые питают распределение taxas. Проверки границ удерживают использование внутри деклараций, а проценты - в пределах 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Общие helpers (`CapacityMetadataEntry`, `PricingScheduleV1`, валидаторы lane/asignment/SLA) para determinar a classe e os relatórios Portanto, você pode usar ferramentas de CI e downstream.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` é um snapshot on-chain publicado por `/v1/sorafs/capacity/state`, fornece informações de provedores e registra taxas de registro para детерминированным Norito JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Покрытие валидации проверяет соблюдение канонических alças, обнаружение дубликатов, границы по lane, guardas назначения репликации и проверки диапазонов телеметрии, чтобы регрессии всплывали сразу в CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Ferramentas do operador: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` конвертирует человекочитаемые especificações em канонические Norito cargas úteis, blobs base64 e resumos JSON, operações de operação você pode obter luminárias `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` e luminárias de pedido de replicação no local валидацией.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Reference fixtures encontrados em `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) e генерируются через `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.### 2. Plano de controle de integração

| Bem | Proprietário(s) | Nomeação |
|------|----------|-------|
| Adicione Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` e Norito cargas úteis JSON. | Equipe Torii | Зеркалировать логику валидации; переиспользовать Norito Ajudantes JSON. |
| Produza snapshots `CapacityDeclarationV1` no orquestrador de placar de metadados e planeje o gateway de busca. | GT Ferramentaria / Equipe Orquestrador | Расширить `provider_metadata` ссылками на capacidade, чтобы мульти-источниковый pontuação соблюдал лимиты по lane. |
| Fornecer pedidos de replicação no orquestrador/gateway dos clientes para executar atribuições e dicas de failover. | Equipe de TL/Gateway de rede | O construtor de placar fornece ordens de replicação de governança. |
| Ferramentas CLI: número `sorafs_cli` e comandos `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Ferramentaria | A definição de JSON + gera placar. |

### 3. Política de mercado e governança

| Bem | Proprietário(s) | Nomeação |
|------|----------|-------|
| Утвердить `MarketplacePolicy` (participação минимальный, мультипликаторы штрафов, периодичность аудита). | Conselho de Governança | Leia nos documentos, verifique a revisão da história. |
| Para criar ganchos de governança, o Parlamento pode aprovar, renovar e revogar declarações. | Conselho de Governança / Equipe de Contrato Inteligente | Use eventos Norito + manifestos de ingestão. |
| Реализовать график штрафов (taxas de cobrança, redução de títulos), привязанный к телеметрируемым нарушениям SLA. | Conselho de Governança / Tesouraria | Согласовать с saídas de liquidação `DealEngine`. |
| Disputa de processo de documentação e escala de matrícula. | Documentos / Governança | Сослаться на disputa runbook + helpers CLI. |

### 4. Taxas de medição e transferência

| Bem | Proprietário(s) | Nomeação |
|------|----------|-------|
| Verifique a medição de ingestão em Torii para o exemplo `CapacityTelemetryV1`. | Equipe Torii | Validar GiB-hora, успех PoR, tempo de atividade. |
| Обновить medição de pipeline `sorafs_node` para obter o fornecimento sob pedido + SLA de especificação. | Equipe de armazenamento | Identifica ordens de replicação e manipula chunker. |
| Liquidação de pipeline: conversão de telefone + replicação em pagamentos, nomeação em XOR, exibição de resumos prontos para governança e registro de registro de faturamento. | Equipe de Tesouraria/Armazenagem | Подключить к Exportações do Deal Engine / Treasury. |
| Exiba painéis/alertas para medição de controle (ingestão de pendências, utilização de telemetria). | Observabilidade | Selecione o pacote Grafana, no conjunto SF-6/SF-7. |- Torii теперь публикует `/v1/sorafs/capacity/telemetry` e `/v1/sorafs/capacity/state` (JSON + Norito), operadores de operação que podem usar telemetria snapshots para um relatório, um inspetor - um livro-razão canônico para auditoria ou упаковки доказательств.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- Интеграция `PinProviderRegistry` garante que as ordens de replicação são fornecidas para o endpoint; helpers CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) теперь валидируют/puбликуют телеметрию из automação executa с детерминированным hashing e разрешением alias.
- Medição de instantâneos формируют записи `CapacityTelemetrySnapshot`, закрепленные за snapshot `metering`, а Prometheus exporta питают готовый к импорту Placa Grafana em `docs/source/grafana_sorafs_metering.json`, seus comandos de faturamento oferecem capacidade de GiB-hora, taxas de programação nano-SORA e o SLA definido na versão real.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Когда включено metering smoothing, snapshot включает `smoothed_gib_hours` e `smoothed_por_success_bps`, чтобы операторы могли сравнивать EMA-трендовые значения сырыми счетчиками, которые governança é usada para pagamentos.【crates/sorafs_node/src/metering.rs:401】

### 5. Disputa de resolução e revogação

| Bem | Proprietário(s) | Nomeação |
|------|----------|-------|
| Определить payload `CapacityDisputeV1` (заявитель, evidência, provedor principal). | Conselho de Governança | Norito esquema + validador. |
| Use CLI para lidar com disputas e disputas (com evidências de anexos). | GT Ferramentaria | Verifique a evidência do pacote de hashing. |
| Faça um teste automático de acordo com o SLA (escalonamento automático em disputa). | Observabilidade | Por alerta e ganchos de governança. |
| Revogação do manual de documentação (período de carência, dados fixados de transferência). | Documentos / Equipe de armazenamento | Consulte o documento de política e o runbook do operador. |

## Teste de teste e CI

- Юнит-тесты para seu novo esquema de validação (`sorafs_manifest`).
- Интеграционные тесты, которые симулируют: декларация → ordem de replicação → medição → pagamento.
- Fluxo de trabalho de CI para regenerar amostra de declaração/телеметрии емкости e provерки синхронизации подписей (расширить `ci/check_sorafs_fixtures.sh`).
- Testes de carga para API de registro (10 mil provedores, 100 mil pedidos).

## Telemetria e dados

- Painel de controle:
  - Декларированная vs использованная емкость по provedor.
  - Pedidos de replicação de pendências e ordem de execução.
  - Соответствие SLA (% de tempo de atividade, частота успеха PoR).
  - Taxas de pagamento e taxas de depósito.
- Alertas:
  - Provedor não tem valor mínimo de dinheiro.
  - Ordem de replicação завис более чем на SLA.
  - Pipeline de medição Сбои.

## Materiais de documentação

- Operador de operação de decodificação de dados, software de transferência e monitoramento de transferência.
- Руководство по governança для утверждения деклараций, выдачи ordens, обработки disputas.
- Referência de API para endpoints e ordem de replicação de formato.
- FAQ do Marketplace para uso.## Чеклист готовности к GA

Roteiro de Пункт **SF-2c** lançamento de produção de bloco para появления конкретных доказательств
por isso, resolva disputas e negociações. Use artefatos novos, seus critérios de criação
entre em contato com a realidade.

### Não use e mude o XOR
- Exporte o snapshot do repositório e exporte o ledger XOR para esse período, para que você possa verificar:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Хелпер завершится с ненулевым кодом при недостающих/переплаченных liquidação или штрафах и
  выдаст текстовый файл Prometheus summary.
- Alerta `SoraFSCapacityReconciliationMismatch` (em `dashboards/alerts/sorafs_capacity_rules.yml`)
  срабатывает, когда métrica de reconciliação сообщают о расхождениях; painéis colocados em
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Arquivar resumo JSON e hashes em `docs/examples/sorafs_capacity_marketplace_validation/`
  вместе с pacotes de governança.

### Disputa de Доказательства e corte
- Disputas Подавайте через `sorafs_manifest_stub capacity dispute` (testes:
  `cargo test -p sorafs_car --test capacity_cli`), essas cargas úteis são canônicas.
- Insira `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e instale-o
  штрафов (`record_capacity_telemetry_penalises_persistent_under_delivery`), чтобы доказать
  детерминированное воспроизведение disputas e barras.
- Conecte `docs/source/sorafs/dispute_revocation_runbook.md` para obter dados e
  эскалации; привязывайте greve de aprovações обратно в relatório de validação.

### Provedores de serviços on-line e gratuitos
- Регенерируйте artefatos деклараций/телеметрии через `sorafs_manifest_stub capacity ...` и
  Progоняйте testes CLI antes de serem executados (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Отправляйте через Torii (`/v1/sorafs/capacity/declare`), фиксируйте
  `/v1/sorafs/capacity/state` mais telas Grafana. Следуйте flow выхода в
  `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Архивируйте подписанные artefatos e resultados de reconciliação внутри
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Зависимости и последовательность

1. Завершить SF-2b (política de admissão) - mercado опирается на проверенных fornecedores.
2. Realize o registro + registro (documento) através da integração Torii.
3. Coloque o pipeline de medição na saída.
4. Fase final: taxas de governança-contaminação de taxas de medição de dados podem ser testadas na preparação.

Прогресс следует отслеживать в roadmap со ссылками на этот документ. Обновляйте roteiro depois disso,
как каждая основная секция (схемы, plano de controle, integração, medição, disputas de resolução) entrega
apresentam status completo.