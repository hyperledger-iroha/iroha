---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Мультиисточниковые объявления провайдеров и планирование

Эта страница содержит стандартную спецификацию
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Используйте этот документ для дословных схем Norito и журнала изменений; версия портала
держите рядом операционные инструкции, заметки SDK и ссылки на телеметрию для остального
набор Runbook SoraFS.

## Дополнения к схеме Norito

### Диапазонная возможность (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – максимальный непрерывный интервал (байты) в запросе, `>= 1`.
- `min_granularity` – поиск разрешения, `1 <= значение <= max_chunk_span`.
- `supports_sparse_offsets` – допускает несмежные смещения в одном запросе.
- `requires_alignment` – если true, зачеты должны соревноваться по `min_granularity`.
- `supports_merkle_proof` – подтверждает подтверждение PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` соответствует каноническому
кодирование, чтобы сплетни полезной нагрузки были частично определены.

### `StreamBudgetV1`
- Поля: `max_in_flight`, `max_bytes_per_sec`, опциональный `burst_bytes`.
- Правила валидации (`StreamBudgetV1::validate`):
  - И18НИ00000027Х, И18НИ00000028Х.
  - `burst_bytes`, если задано, должны быть `> 0` и `<= max_bytes_per_sec`.

### `TransportHintV1`
- Поля: `protocol: TransportProtocol`, `priority: u8` (окно 0-15 контролируется)
  `TransportHintV1::validate`).
- Известные протоколы: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Дублирующиеся записи протоколов на провайдера отклоняются.

### Дополнения к `ProviderAdvertBodyV1`
- Опциональный `stream_budget: Option<StreamBudgetV1>`.
- Опциональный `transport_hints: Option<Vec<TransportHintV1>>`.
- Оба поля транзита через `ProviderAdmissionProposalV1`, конверты управления,
  CLI-фиксаторы и телеметрический JSON.

## Валидация и привязка к управлению

`ProviderAdvertBodyV1::validate` и `ProviderAdmissionProposalV1::validate`
отклонить поврежденные метаданные:

- Расширенные возможности должны корректно декодироваться и соблюдать лимиты.
  настройки/гранулярности.
- Бюджеты потоков/подсказки по транспортировке требуют TLV `CapabilityType::ChunkRangeFetch`
  и непустого списка подсказок.
- Дублирование транспортных протоколов и некорректированных приоритетов вызывает ошибки.
  валидации до сплетен, рассылки рекламы.
- Приемные конверты сравнивают предложения/объявления по широкому спектру метаданных через
  `compare_core_fields`, чтобы несовпадающие сплетни отклонялись заранее.

Регрессионное покрытие находится в
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

##Инструменты и приспособления

- Полезные нагрузки бесплатных провайдеров должны включать `range_capability`, `stream_budget`.
  и `transport_hints`. Проверяйте через ответы `/v2/sorafs/providers` и приемные приспособления;
  JSON-резюме должно включать в себя разобранную возможность, бюджет потока и подсказки по массивам.
  для телеметрического приема.
- `cargo xtask sorafs-admission-fixtures` выводит бюджеты потоков и подсказки по транспортировке
  в своих артефактах JSON, чтобы информационные панели отслеживали добавления функций.
- Светильники в `fixtures/sorafs_manifest/provider_admission/` теперь включают:
  - стандартные мульти-источниковые объявления,
  - `multi_fetch_plan.json`, чтобы наборы SDK могли воспроизвести определенный
    план многоранговой выборки.

## Интеграция с оркестратором и Torii- Torii `/v2/sorafs/providers` возвращает разобранные метаданные разнообразные возможности
  вместе с `stream_budget` и `transport_hints`. Предупреждения о понижении рейтинга сработают, когда
  провайдеры пропускают новые метаданные, диапазон конечных точек шлюза с теми же ограничениями
  для прямых клиентов.
- Мульти-источниковый оркестратор (`sorafs_car::multi_fetch`) теперь применяется лимиты связи,
  спортивные возможности и поток бюджетов при распределении работы. Юнит-тесты раскрывают
  бывает слишком большой кусок, разреженного поиска и дросселирования.
- `sorafs_car::multi_fetch` передает сигналы понижения рейтинга (ошибки спортсменния,
  регулируемые запросы), чтобы операторы понимали, почему конкретные провайдеры
  были пропущены при планировании.

## Справочник телеметрии

Инструментарий выборки диапазона в Torii питает Grafana приборную панель **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) и соответствующие правила оповещений
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| Метрика | Тип | Метки | Описание |
|---------|-----|-------|----------|
| `torii_sorafs_provider_range_capability_total` | Калибр | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Провайдеры, объявляющие функции широкого спектра возможностей. |
| `torii_sorafs_range_fetch_throttle_events_total` | Счетчик | `reason` (`quota`, `concurrency`, `byte_rate`) | Эксперименты по выборке диапазона с регулированием, сгруппированные по политике. |
| `torii_sorafs_range_fetch_concurrency_current` | Калибр | — | Активные защищенные потоки, потребляющие общую бюджетную конкуренцию. |

Примеры PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Используйте регулирование счетчика, чтобы надежно применить квоту перед включением.
дефолтов мульти-источникового оркестратора и поднимайте тревогу при конкуренции
Это соответствует максимальным значениям потока бюджета вашей флотилии.