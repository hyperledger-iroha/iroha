---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b634937fd6fdd8a275f07347d85dc113c752ee790ef76187610667a7376c7162
source_last_modified: "2025-11-14T04:43:21.914046+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Канонический источник
:::

## Дизайн хранения узла SoraFS (Черновик)

Эта записка уточняет, как узел Iroha (Torii) может подключиться к слою
availability данных SoraFS и выделить часть локального диска для хранения и
обслуживания чанков. Она дополняет discovery-спецификацию
`sorafs_node_client_protocol.md` и работу по fixtures SF-1b, описывая архитектуру
сториджа, ресурсные ограничения и конфигурационную проводку, которые должны
появиться в узле и gateway-коде. Практические операционные процедуры описаны в
[Runbook операций узла](./node-operations).

### Цели

- Разрешить любому валидатору или вспомогательному Iroha процессу публиковать
  свободный диск как провайдер SoraFS без влияния на обязанности ledger.
- Держать модуль хранения детерминированным и управляемым Norito: манифесты,
  планы чанков, корни Proof-of-Retrievability (PoR) и advert провайдеров — это
  источник истины.
- Применять операторские квоты, чтобы узел не исчерпал свои ресурсы из-за
  слишком большого количества запросов pin или fetch.
- Отдавать здоровье/телеметрию (PoR sampling, latency fetch чанков, давление на
  диск) обратно в governance и клиентам.

### Архитектура высокого уровня

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

Ключевые модули:

- **Gateway**: открывает Norito HTTP эндпоинты для предложений pin, запросов fetch
  чанков, PoR sampling и телеметрии. Валидирует Norito payloads и направляет
  запросы в chunk store. Переиспользует существующий HTTP стек Torii, чтобы не
  вводить новый демон.
- **Pin Registry**: состояние pin манифестов в `iroha_data_model::sorafs` и
  `iroha_core`. При принятии манифеста регистр хранит digest манифеста, digest
  плана чанков, корень PoR и флаги возможностей провайдера.
- **Chunk Storage**: дисковая реализация `ChunkStore`, которая принимает подписанные
  манифесты, материализует планы чанков через `ChunkProfile::DEFAULT`, и сохраняет
  чанки в детерминированном layout. Каждый чанк связан с fingerprint контента и
  PoR метаданными, поэтому sampling может повторно валидировать без перечитывания
  всего файла.
- **Quota/Scheduler**: применяет лимиты оператора (макс. байты диска, макс. pins
  в очереди, макс. параллельные fetch, TTL чанков) и координирует IO, чтобы задачи
  ledger не были вытеснены. Scheduler также отвечает за выдачу PoR proof и запросы
  sampling с ограниченным CPU.

### Конфигурация

Добавьте новую секцию в `iroha_config`:

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
```

- `enabled`: переключатель участия. Когда false, gateway возвращает 503 для storage
  эндпоинтов и узел не объявляется в discovery.
- `data_dir`: корневая директория для чанков, PoR деревьев и телеметрии fetch.
  По умолчанию `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: жесткий лимит на pin-данные. Фоновая задача отклоняет
  новые pins при достижении лимита.
- `max_parallel_fetches`: лимит конкурентности, накладываемый scheduler, чтобы
  балансировать сеть/IO диска с нагрузкой валидатора.
- `max_pins`: максимум manifest pins, которые принимает узел до применения
  eviction/back pressure.
- `por_sample_interval_secs`: каденция автоматических PoR sampling jobs. Каждый
  job выбирает `N` листьев (настраивается per manifest) и эмитит телеметрию.
  Governance может детерминированно масштабировать `N`, задавая ключ
  `profile.sample_multiplier` (целое `1-4`). Значение может быть числом/строкой
  или объектом с override по профилю, например `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: структура, используемая генератором advert для заполнения полей
  `ProviderAdvertV1` (stake pointer, QoS hints, topics). Если опущено, узел берет
  defaults из governance registry.

Прокладка конфигурации:

- `[sorafs.storage]` определен в `iroha_config` как `SorafsStorage` и загружается
  из конфигурационного файла узла.
- `iroha_core` и `iroha_torii` прокидывают storage config в gateway builder и chunk
  store при старте.
- Существуют dev/test overrides (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), но
  в production следует использовать конфиг-файл.

### CLI утилиты

Пока HTTP поверхность Torii еще подключается, crate `sorafs_node` поставляет
тонкий CLI, чтобы операторы могли скриптовать ingestion/export drills против
персистентного backend.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` ожидает Norito-encoded manifest `.to` плюс соответствующие payload bytes.
  Он восстанавливает план чанков из профиля чанкинга манифеста, проверяет паритет
  digest, сохраняет файлы чанков и опционально эмитит JSON `chunk_fetch_specs`, чтобы
  downstream инструменты могли проверить layout.
- `export` принимает manifest ID и пишет сохраненные manifest/payload на диск
  (с опциональным plan JSON), чтобы fixtures оставались воспроизводимыми.

Обе команды печатают Norito JSON summary в stdout, что удобно для скриптов. CLI
покрыт интеграционным тестом, который подтверждает корректный round-trip manifests
и payloads до появления Torii API.【crates/sorafs_node/tests/cli.rs:1】

> HTTP паритет
>
> Torii gateway теперь предоставляет read-only helpers на основе того же
> `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — возвращает сохраненный
>   Norito manifest (base64) вместе с digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — возвращает детерминированный
>   JSON план чанков (`chunk_fetch_specs`) для downstream tooling.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Эти эндпоинты повторяют CLI вывод, поэтому пайплайны могут перейти от локальных
> скриптов к HTTP probes без смены парсеров.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Жизненный цикл узла

1. **Startup**:
   - При включенном storage узел инициализирует chunk store с заданной директорией
     и емкостью. Это включает проверку/создание PoR базы манифестов и replay pin
     манифестов для прогрева кэшей.
   - Зарегистрировать маршруты SoraFS gateway (Norito JSON POST/GET эндпоинты для pin,
     fetch, PoR sampling, телеметрии).
   - Запустить PoR sampling worker и монитор квот.
2. **Discovery / Adverts**:
   - Сформировать `ProviderAdvertV1` на основе текущей емкости/здоровья, подписать
     ключом, одобренным советом, и опубликовать через discovery канал.
     оставались доступными.
3. **Pin Workflow**:
   - Gateway получает подписанный manifest (с планом чанков, корнем PoR и подписями
     совета). Валидирует список alias (`sorafs.sf1@1.0.0` обязателен) и убеждается,
     что план чанков соответствует метаданным манифеста.
   - Проверяет квоты. При превышении capacity/pin лимитов отвечает политикой ошибки
     (структурированный Norito).
   - Стримит chunk данные в `ChunkStore`, проверяя digests на ingest. Обновляет PoR
     деревья и хранит metadata манифеста в registry.
4. **Fetch Workflow**:
   - Отдает запросы range чанков с диска. Scheduler применяет `max_parallel_fetches`
     и возвращает `429` при насыщении.
   - Эмитит структурированную телеметрию (Norito JSON) с latency, bytes served и
     счетчиками ошибок для downstream мониторинга.
5. **PoR Sampling**:
   - Worker выбирает манифесты пропорционально весу (например, байтам хранения) и
     запускает детерминированный sampling через PoR дерево chunk store.
   - Сохраняет результаты для governance аудита и включает сводки в provider adverts
     / telem endpoint.
6. **Eviction / квоты**:
   - При достижении емкости узел по умолчанию отклоняет новые pins. Опционально
     операторы смогут настроить политики eviction (например, TTL, LRU) после
     согласования модели governance; пока дизайн предполагает строгие квоты и
     unpin операции, инициируемые оператором.

### Интеграция деклараций емкости и scheduling

- Torii теперь ретранслирует обновления `CapacityDeclarationRecord` из `/v1/sorafs/capacity/declare`
  во встроенный `CapacityManager`, так что каждый узел строит in-memory представление своих
  зафиксированных chunker/lane аллокаций. Менеджер публикует read-only snapshots для телеметрии
  (`GET /v1/sorafs/capacity/state`) и применяет резервы per-profile/per-lane до принятия новых
  заказов.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Эндпоинт `/v1/sorafs/capacity/schedule` принимает governance-issued `ReplicationOrderV1`
  payloads. Когда заказ нацелен на локального провайдера, менеджер проверяет дублирование
  расписаний, валидирует емкость chunker/lane, резервирует слот и возвращает `ReplicationPlan`
  с описанием оставшейся емкости, чтобы оркестрация могла продолжить ingestion. Заказы для
  других провайдеров подтверждаются ответом `ignored`, упрощая multi-operator workflows.【crates/iroha_torii/src/routing.rs:4845】
- Completion hooks (например, после успешной ingest) вызывают
  `POST /v1/sorafs/capacity/complete` для освобождения резерва через
  `CapacityManager::complete_order`. Ответ включает snapshot `ReplicationRelease`
  (остаточные totals, остатки chunker/lane), чтобы orchestration tooling могло
  ставить следующий заказ без polling. Дальнейшая работа подключит это к pipeline
  chunk store, как только ingestion логика появится.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Встроенный `TelemetryAccumulator` можно обновлять через `NodeHandle::update_telemetry`,
  что позволяет background workers фиксировать PoR/uptime samples и со временем
  выводить канонические `CapacityTelemetryV1` payloads без изменения внутренних
  частей scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Интеграции и будущая работа

- **Governance**: расширить `sorafs_pin_registry_tracker.md` телеметрией хранения
  (PoR success rate, disk utilization). Политики допуска могут требовать минимальную
  емкость или минимальный PoR success rate до принятия adverts.
- **Client SDKs**: раскрыть новую storage config (лимиты диска, alias), чтобы
  management tooling могло bootstrapper узлы программно.
- **Telemetry**: интегрировать с существующим metrics stack (Prometheus /
  OpenTelemetry), чтобы метрики хранения были видимы в observability dashboards.
- **Security**: запускать модуль хранения в отдельном async task pool с back-pressure
  и рассмотреть sandboxing чтения чанков через io_uring или ограниченные tokio pools,
  чтобы злоумышленники не исчерпали ресурсы.

Этот дизайн сохраняет модуль хранения опциональным и детерминированным, давая
операторам нужные knobs для участия в слое доступности данных SoraFS. Реализация
потребует изменений в `iroha_config`, `iroha_core`, `iroha_torii` и Norito gateway,
а также tooling для provider advert.
