---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: node-plan
title: План реализации узла SoraFS
sidebar_label: План реализации узла
description: Преобразует дорожную карту хранилища SF-3 в инженерную работу с вехами, задачами и покрытием тестами.
---

:::note Канонический источник
:::

SF-3 поставляет первый исполняемый crate `sorafs-node`, который превращает процесс Iroha/Torii в провайдера хранения SoraFS. Используйте этот план вместе с [гайдом по хранилищу узла](node-storage.md), [политикой допуска провайдеров](provider-admission-policy.md) и [дорожной картой маркетплейса емкости хранения](storage-capacity-marketplace.md) при выстраивании последовательности работ.

## Целевой объем (веха M1)

1. **Интеграция chunk store.** Обернуть `sorafs_car::ChunkStore` в персистентный backend, который хранит байты чанков, манифесты и PoR-деревья в заданной директории данных.
2. **Эндпоинты gateway.** Открыть Norito HTTP эндпоинты для подачи pin, fetch чанков, PoR sampling и телеметрии хранилища внутри процесса Torii.
3. **Прокладка конфигурации.** Добавить структуру конфигурации `SoraFsStorage` (флаг включения, емкость, директории, лимиты конкурентности), проведенную через `iroha_config`, `iroha_core` и `iroha_torii`.
4. **Квоты/планирование.** Применять лимиты диска/параллелизма оператора и ставить запросы в очередь с back-pressure.
5. **Телеметрия.** Эмитить метрики/логи по успехам pin, задержке fetch чанков, загрузке емкости и результатам PoR sampling.

## Разбиение работ

### A. Структура крейта и модулей

| Задача | Ответственный(е) | Примечания |
|------|------------------|------------|
| Создать `crates/sorafs_node` с модулями: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Storage Team | Реэкспортировать переиспользуемые типы для интеграции с Torii. |
| Реализовать `StorageConfig`, сопоставленный с `SoraFsStorage` (user → actual → defaults). | Storage Team / Config WG | Гарантировать детерминизм слоев Norito/`iroha_config`. |
| Предоставить фасад `NodeHandle`, который Torii использует для pins/fetches. | Storage Team | Инкапсулировать внутренности хранения и async-проводку. |

### B. Персистентный chunk store

| Задача | Ответственный(е) | Примечания |
|------|------------------|------------|
| Построить дисковый backend вокруг `sorafs_car::ChunkStore` с on-disk индексом манифестов (`sled`/`sqlite`). | Storage Team | Детерминированный layout: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Поддерживать PoR метаданные (деревья 64 KiB/4 KiB) через `ChunkStore::sample_leaves`. | Storage Team | Поддерживает replay после рестарта; fail fast при коррупции. |
| Реализовать integrity replay на старте (rehash манифестов, чистка незавершенных pins). | Storage Team | Блокирует старт Torii до завершения replay. |

### C. Эндпоинты gateway

| Эндпоинт | Поведение | Задачи |
|---------|-----------|--------|
| `POST /sorafs/pin` | Принимает `PinProposalV1`, валидирует манифесты, ставит ingestion в очередь, отвечает CID манифеста. | Валидировать профиль chunker, применять квоты, стримить данные через chunk store. |
| `GET /sorafs/chunks/{cid}` + range query | Отдает байты чанка с заголовками `Content-Chunker`; соблюдает спецификацию range capability. | Использовать scheduler + бюджеты стрима (связать с SF-2d range capability). |
| `POST /sorafs/por/sample` | Запускает PoR sampling для манифеста и возвращает proof bundle. | Переиспользовать sampling chunk store, отвечать Norito JSON payloads. |
| `GET /sorafs/telemetry` | Сводки: емкость, успех PoR, счетчики ошибок fetch. | Предоставлять данные для дашбордов/операторов. |

Рантайм-проводка направляет PoR взаимодействия через `sorafs_node::por`: трекер фиксирует каждый `PorChallengeV1`, `PorProofV1` и `AuditVerdictV1`, чтобы метрики `CapacityMeter` отражали вердикты governance без отдельной логики Torii.【crates/sorafs_node/src/scheduler.rs#L147】

Примечания по реализации:

- Использовать Axum стек Torii с `norito::json` payloads.
- Добавить Norito схемы ответов (`PinResultV1`, `FetchErrorV1`, telemetry structs).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` теперь показывает глубину backlog, самый старый epoch/deadline и последние success/failure timestamps по каждому провайдеру, за счет `sorafs_node::NodeHandle::por_ingestion_status`, а Torii фиксирует метрики `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` для дашбордов.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Scheduler и применение квот

| Задача | Детали |
|------|--------|
| Дисковая квота | Отслеживать байты на диске; отклонять новые pins при превышении `max_capacity_bytes`. Подготовить хуки эвикции для будущих политик. |
| Конкурентность fetch | Глобальный семафор (`max_parallel_fetches`) плюс бюджеты per-provider из лимитов SF-2d range. |
| Очередь pins | Лимитировать незавершенные ingestion jobs; предоставить Norito статус эндпоинты для глубины очереди. |
| Каденция PoR | Фоновый воркер, управляемый `por_sample_interval_secs`. |

### E. Телеметрия и логирование

Метрики (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (гистограмма с метками `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Логи / события:

- Структурированная Norito телеметрия для governance ingestion (`StorageTelemetryV1`).
- Алерты при использовании > 90% или превышении порога серии PoR-ошибок.

### F. Стратегия тестирования

1. **Юнит-тесты.** Персистентность chunk store, вычисления квот, инварианты scheduler (см. `crates/sorafs_node/src/scheduler.rs`).
2. **Интеграционные тесты** (`crates/sorafs_node/tests`). Pin → fetch round trip, восстановление после рестарта, отказ по квоте, проверка PoR proof sampling.
3. **Torii интеграционные тесты.** Запустить Torii с включенным хранилищем, прогнать HTTP эндпоинты через `assert_cmd`.
4. **Roadmap chaos.** Будущие drills моделируют исчерпание диска, медленный IO, удаление провайдеров.

## Зависимости

- Политика допуска SF-2b — убедиться, что узлы проверяют admission envelopes до публикации advert.
- Маркетплейс емкости SF-2c — связать телеметрию с декларациями емкости.
- Расширения advert SF-2d — использовать range capability + бюджеты стримов по мере появления.

## Критерии завершения вехи

- `cargo run -p sorafs_node --example pin_fetch` работает на локальных fixtures.
- Torii собирается с `--features sorafs-storage` и проходит интеграционные тесты.
- Документация ([гайд по хранилищу узла](node-storage.md)) обновлена с default конфигурацией + примерами CLI; runbook для операторов доступен.
- Телеметрия видна в staging dashboard; алерты настроены на насыщение емкости и PoR ошибки.

## Документация и операционные deliverables

- Обновить [справочник по хранилищу узла](node-storage.md) с дефолтами конфигурации, использованием CLI и шагами troubleshooting.
- Держать [runbook операций узла](node-operations.md) в соответствии с реализацией по мере эволюции SF-3.
- Опубликовать ссылки на API `/sorafs/*` в портале разработчиков и связать с OpenAPI manifest, когда Torii handlers появятся.
