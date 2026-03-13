---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e9f2cc35c57ca6e054276972f341d4fa44ff4b164a5c0bb707025b80c4e7bf25
source_last_modified: "2025-11-08T17:35:21.580244+00:00"
translation_last_reviewed: 2026-01-30
---

# Мульти-источниковые объявления провайдеров и планирование

Эта страница сводит каноническую спецификацию в
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Используйте этот документ для дословных схем Norito и changelog; версия портала
держит рядом операционные инструкции, заметки SDK и ссылки на телеметрию для остального
набора runbooks SoraFS.

## Дополнения к схеме Norito

### Диапазонная возможность (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – максимальный непрерывный интервал (байты) на запрос, `>= 1`.
- `min_granularity` – разрешение seek, `1 <= значение <= max_chunk_span`.
- `supports_sparse_offsets` – допускает несмежные offsets в одном запросе.
- `requires_alignment` – если true, offsets должны выравниваться по `min_granularity`.
- `supports_merkle_proof` – указывает поддержку свидетельств PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` обеспечивают каноническое
кодирование, чтобы payloads gossip оставались детерминированными.

### `StreamBudgetV1`
- Поля: `max_in_flight`, `max_bytes_per_sec`, опциональный `burst_bytes`.
- Правила валидации (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, если задан, должен быть `> 0` и `<= max_bytes_per_sec`.

### `TransportHintV1`
- Поля: `protocol: TransportProtocol`, `priority: u8` (окно 0-15 контролируется
  `TransportHintV1::validate`).
- Известные протоколы: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Дублирующиеся записи протоколов на провайдера отклоняются.

### Дополнения к `ProviderAdvertBodyV1`
- Опциональный `stream_budget: Option<StreamBudgetV1>`.
- Опциональный `transport_hints: Option<Vec<TransportHintV1>>`.
- Оба поля проходят через `ProviderAdmissionProposalV1`, governance envelopes,
  CLI fixtures и телеметрический JSON.

## Валидация и привязка к governance

`ProviderAdvertBodyV1::validate` и `ProviderAdmissionProposalV1::validate`
отклоняют поврежденные метаданные:

- Диапазонные возможности должны корректно декодироваться и соблюдать лимиты
  диапазона/гранулярности.
- Stream budgets / transport hints требуют TLV `CapabilityType::ChunkRangeFetch`
  и непустого списка hints.
- Дублирующиеся транспортные протоколы и некорректные приоритеты вызывают ошибки
  валидации до gossip рассылки adverts.
- Admission envelopes сравнивают proposal/adverts по диапазонным метаданным через
  `compare_core_fields`, чтобы несовпадающие gossip payloads отклонялись заранее.

Регрессионное покрытие находится в
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Инструменты и fixtures

- Payloads объявлений провайдеров должны включать `range_capability`, `stream_budget`
  и `transport_hints`. Проверяйте через ответы `/v2/sorafs/providers` и admission fixtures;
  JSON-резюме должны включать разобранную capability, stream budget и массивы hints
  для телеметрического ingest.
- `cargo xtask sorafs-admission-fixtures` выводит stream budgets и transport hints
  в своих JSON artefacts, чтобы dashboards отслеживали внедрение функций.
- Fixtures в `fixtures/sorafs_manifest/provider_admission/` теперь включают:
  - канонические мульти-источниковые adverts,
  - `multi_fetch_plan.json`, чтобы SDK наборы могли воспроизводить детерминированный
    multi-peer fetch план.

## Интеграция с оркестратором и Torii

- Torii `/v2/sorafs/providers` возвращает разобранные метаданные диапазонных возможностей
  вместе с `stream_budget` и `transport_hints`. Предупреждения downgrade срабатывают, когда
  провайдеры пропускают новые метаданные, а range endpoints шлюза применяют те же ограничения
  для прямых клиентов.
- Мульти-источниковый оркестратор (`sorafs_car::multi_fetch`) теперь применяет лимиты диапазона,
  выравнивание возможностей и stream budgets при распределении работы. Unit-тесты покрывают
  случаи слишком больших chunk, разреженного seek и throttling.
- `sorafs_car::multi_fetch` передает сигналы downgrade (ошибки выравнивания,
  throttled запросы), чтобы операторы могли понимать, почему конкретные провайдеры
  были пропущены при планировании.

## Справочник телеметрии

Инструментация range fetch в Torii питает Grafana dashboard **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) и соответствующие правила алертов
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| Метрика | Тип | Метки | Описание |
|---------|-----|-------|----------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Провайдеры, объявляющие функции диапазонной возможности. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | Попытки range fetch с throttling, сгруппированные по политике. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | Активные защищенные потоки, потребляющие общий бюджет конкуренции. |

Примеры PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Используйте счетчик throttling, чтобы подтвердить применение квот перед включением
дефолтов мульти-источникового оркестратора, и поднимайте алерты, когда конкуренция
приближается к максимальным значениям stream budget по вашей флотилии.
