---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: armazenamento de nó
título: Дизайн хранения узла SoraFS
sidebar_label: A configuração da barra lateral
description: Архитектура хранения, квоты и lifecycle hooks para Torii узлов, хостящих данные SoraFS.
---

:::nota História Canônica
:::

## A configuração padrão do SoraFS (Verdadeiro)

Esta é uma descrição que pode ser feita com Iroha (Torii)
disponibilidade данных SoraFS e выделить часть локального диска для хранения и
обслуживания чанков. Sobre a descoberta-спецификацию
`sorafs_node_client_protocol.md` e instalado em luminárias SF-1b, descrição do arquiteto
сториджа, ресурсные ограничения e конфигурационную проводку, которые должны
появиться в узле и gateway-code. Procedimento de operação prática descrito em
[Runbook de operação](./node-operations).

###Céli

- Разрешить любому валидатору или вспомогательному Iroha процессу публиковать
  O disco inteiro que fornece o SoraFS não é transferido para o livro-razão.
- Defina a configuração do módulo para determinar e atualizar Norito: манифесты,
  planos de recuperação, prova de recuperação (PoR) e testes de anúncios - isso
  istóчник истины.
- Применять операторские квоты, чтобы узел не исчерпал свои ресурсы из-за
  слишком большого количества запросов pin ou fetch.
- Отдавать здоровье/телеметрию (amostragem PoR, busca de latência чанков, давление на
  disco) funciona em governança e cliente.

### O arquiteto está em seu lugar

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Módulo de seleção:

- **Gateway**: abre Norito HTTP эндпоинты para предложений pin, zaпросов fetch
  чанков, amostragem PoR e telemetria. Validar cargas úteis Norito e instalar
  comprado na loja de pedaços. Selecione a seção HTTP Torii, não há nada
  вводить новый демон.
- **Registro de Pins**: состояние pin манифестов в `iroha_data_model::sorafs` и
  `iroha_core`. При принятии манифеста registr хранит digest манифеста, digest
  plano чанков, корень PoR e флаги возможностей провайдера.
- **Armazenamento de pedaços**: disco realizável `ChunkStore`, которая принимает подписанные
  манифесты, материализует planos чанков через `ChunkProfile::DEFAULT`, e сохраняет
  чанки в детерминированном layout. Каждый чанк связан com impressão digital контента и
  PoR метаданными, поэтому sampling может повторно валидировать без перечитывания
  você está falando.
- **Quota/Scheduler**: применяет лимиты оператора (макс. байты диска, макс. pins
  na verdade, máx. busca paralela, TTL чанков) e координирует IO, чтобы задачи
  o razão não foi usado. O agendador também é usado para prova e proteção de PoR
  amostragem na CPU.

### Configuração

Faça uma nova seleção em `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # опциональный человекочитаемый тег
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled`: configuração correta. Когда false, gateway возвращает 503 para armazenamento
  эндпоинтов и узел не объявляется в descoberta.
- `data_dir`: diretório de transferência para чанков, PoR деревьев e busca de telemetria.
  Para usar `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: limite de pino definido. A fonte está disponível
  novos pinos при достижении лимита.
- `max_parallel_fetches`: limite de configuração, agendador de ativação, чтобы
  балансировать сеть/IO disco com нагрузкой валидатора.
- `max_pins`: máximo de pinos de manifesto, которые принимает узел до применения
  despejo/contrapressão.
- `por_sample_interval_secs`: каденция автоматических trabalhos de amostragem PoR. Каждый
  job usa a lista `N` (transformada por manifesto) e limita a telemetria.
  Governança pode ser determinada pelo mestre `N`, задавая ключ
  `profile.sample_multiplier` (é `1-4`). Você pode usar um círculo/estrela
  ou será substituído por substituição no perfil, por exemplo `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: estrutura, geração de anúncio de uso para proteção contra incêndio
  `ProviderAdvertV1` (ponteiro de piquetagem, dicas de QoS, tópicos). Ou seja, você é uma boina
  padrões do registro de governança.

Configurações de configuração:

- `[sorafs.storage]` instalado em `iroha_config` como `SorafsStorage` e desbloqueado
  da configuração usada.
- `iroha_core` e `iroha_torii` configuram configuração de armazenamento no construtor de gateway e bloco
  loja antes do início.
- Alterar substituições de desenvolvimento/teste (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), não
  na produção, o processo é usado para configurar o arquivo.

### CLI utilitários

Para HTTP usar Torii, você pode enviar a caixa `sorafs_node`
тонкий CLI, чтобы операторы могли скриптовать exercícios de ingestão/exportação prov
backend específico.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` ожидает Manifesto codificado em Norito `.to` mais соответствующие bytes de carga útil.
  Ao usar o plano чанков no perfil чанкинга манифеста, prove a participação
  digerir, armazenar arquivos чанков e opcionalmente эмитит JSON `chunk_fetch_specs`, чтобы
  layout downstream pode fornecer layout.
- `export` identifica o ID do manifesto e coloca o manifesto/carga útil no disco
  (no plano JSON especificado), esses fixtures serão configurados.

Esses comandos carregam o resumo JSON Norito no stdout, o que é feito para o script. CLI
teste de integração, que fornece manifestos de ida e volta corretos
e cargas úteis para a API Torii.【crates/sorafs_node/tests/cli.rs:1】> Partida HTTP
>
> Torii gateway теперь предоставляет auxiliares somente leitura em основе того же
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — configuração de segurança
> Manifesto Norito (base64) em digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — determinação de valor
> Configurações de plano JSON (`chunk_fetch_specs`) para ferramentas downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Este ponto de acesso é o CLI que você deseja, pois a configuração pode ser transferida do local
> скриптов к HTTP probes без смены парсеров.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Жизненный цикл узла

1. **Inicialização**:
   - Seu armazenamento inicial é inicializado na loja de blocos com o diretório de destino
     e емкостью. Este é o teste/configuração do PoR para todos os manifestos e pinos de repetição
     манифестов для прогрева кэшей.
   - Зарегистрировать маршруты SoraFS gateway (Norito JSON POST/GET эндпоинты для pin,
     busca, amostragem PoR, telemetria).
   - Запустить trabalhador de amostragem PoR e monitor квот.
2. **Descoberta/Anúncios**:
   - Сформировать `ProviderAdvertV1` na основе текущей емкости/здоровья, подписать
     ключом, одобренным советом, e опубликовать через Discovery canal.
     оставались доступными.
3. **Fluxo de trabalho de fixação**:
   - Gateway получает подписанный manifesto (с планом чанков, корнем PoR и подписями
     совета). Валидирует список alias (`sorafs.sf1@1.0.0` обязателен) e убеждается,
     Este plano de чанков é uma manifestação metadanным.
   - Проверяет квоты. Para verificar os limites de capacidade/pino лимитов отвечает политикой ошибки
     (структурированный Norito).
   - Стримит chunk данные в `ChunkStore`, проверяя digests на ingest. Ativar PoR
     деревья и хранит metadados são exibidos no registro.
4. **Buscar fluxo de trabalho**:
   - Отдает запросы range чанков с диска. Agendador usado `max_parallel_fetches`
     e instale `429` antes de usar.
   - Emita uma estrutura de telefonia (Norito JSON) com latência, bytes servidos e
     счетчиками ошибок para monitoramento downstream.
5. **Amostragem PoR**:
   - Trabalhador выбирает манифесты пропорционально весу (например, байтам хранения) и
     запускает детерминированный amostragem через PoR дерево chunk store.
   - Obtenha resultados para auditoria de governança e obtenha resultados em anúncios de provedores
     / terminal telem.
6. **Despejo / квоты**:
   - Antes de usar, use novos pinos para abrir. Opcional
     операторы смогут настроить политики despejo (например, TTL, LRU) после
     modelo de governança corporativa; пока дизайн предполагает строгие квоты и
     desafixar operação, iniciar operação.

### Интеграция деклараций емкости и agendamento- Torii tem a capacidade de recuperar `CapacityDeclarationRecord` de `/v1/sorafs/capacity/declare`
  no `CapacityManager`, então este caso está usando uma estrutura na memória para fornecer seu serviço
  Alocação de chunker/lane. Менеджер публикует instantâneos somente leitura para telemetria
  (`GET /v1/sorafs/capacity/state`) e selecione configurações por perfil/por pista para novas atualizações
  заказов.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Эндпоинт `/v1/sorafs/capacity/schedule` принимает `ReplicationOrderV1` emitido pela governança
  cargas úteis. Você pode fazer isso em um provedor local, mas não precisa de um provedor local
  расписаний, валидирует емкость chunker/lane, резервирует слот e возвращает `ReplicationPlan`
  com a descrição de que o consumo está disponível, essa orquestração pode produzir ingestão. Заказы для
  других провайдеров подтверждаются ответом `ignored`, упрощая fluxos de trabalho multi-operador.【crates/iroha_torii/src/routing.rs:4845】
- Ganchos de conclusão (например, после успешной ingerir) вызывают
  `POST /v1/sorafs/capacity/complete` para operação de reserva de energia
  `CapacityManager::complete_order`. Ответ включает snapshot `ReplicationRelease`
  (totais de остаточные, остатки chunker/lane), ferramentas de orquestração de чтобы могло
  ставить следующий заказ без polling. A tarefa de trabalhar com este pipeline é
  chunk store, как только ingestão логика появится.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- O modelo `TelemetryAccumulator` pode ser usado para `NodeHandle::update_telemetry`,
  Isso permite que trabalhadores em segundo plano фиксировать amostras de PoR/tempo de atividade e assim por diante
  выводить канонические `CapacityTelemetryV1` cargas úteis sem uso de energia
  частей agendador.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integração e trabalho

- **Governança**: расширить `sorafs_pin_registry_tracker.md` телеметрией хранения
  (Taxa de sucesso de PoR, utilização de disco). A declaração política pode ser mínima
  Taxa de sucesso de PoR alta ou mínima para anúncios de publicidade.
- **SDKs do cliente**: раскрыть новую configuração de armazenamento (лимиты диска, alias), чтобы
  ferramentas de gerenciamento podem ser usadas no programa bootstrapper.
- **Telemetria**: integração da pilha de métricas atualizada (Prometheus /
  OpenTelemetry), essas métricas são visualizadas em painéis de observabilidade.
- **Segurança**: запускать модуль хранения в отдельном pool de tarefas assíncronas com contrapressão
  e рассмотреть sandboxing чтения чанков через io_uring ou ограниченные tokio pools,
  чтобы злоумышленники не исчерпали ресурсы.

Este é o módulo de configuração de operação e determinação do módulo, dia
операторам нужные botões para участия в слое доступности данных SoraFS. Realização
pode ser usado em `iroha_config`, `iroha_core`, `iroha_torii` e Norito gateway,
uma ferramenta semelhante para anúncio do fornecedor.