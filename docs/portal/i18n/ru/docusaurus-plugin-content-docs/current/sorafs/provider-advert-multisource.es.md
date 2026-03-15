---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Объявления о различных источниках и планировании

Это страница с каноническими спецификациями в
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Используйте этот документ для подобных вопросов Norito дословно и журналы изменений; копия дель портала
mantiene la Guía de Operadores, las notas de SDK и las references de telemetería cerca del resto
Runbooks SoraFS.

## Дополнительные сведения Norito

### Возможность рангомирования (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – главный проход (байты) по запросу, `>= 1`.
- `min_granularity` – разрешение ошибки, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` – разрешается смещение смежных участков по запросу.
- `requires_alignment` – если это правда, смещения будут линейными с `min_granularity`.
- `supports_merkle_proof` – индикаторный образец PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` каноническая кодировка
для того, чтобы сплетни были детерминистическими.

### `StreamBudgetV1`
- Кампос: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` опционально.
- Правила проверки (`StreamBudgetV1::validate`):
  - И18НИ00000027Х, И18НИ00000028Х.
  - `burst_bytes`, когда оно представлено, должно быть `> 0` и `<= max_bytes_per_sec`.

### `TransportHintV1`
- Кампос: `protocol: TransportProtocol`, `priority: u8` (приложение 0-15 для
  `TransportHintV1::validate`).
- Известные протоколы: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Если были введены дубликаты протокола проверяющего.

### Добавление `ProviderAdvertBodyV1`
- `stream_budget` дополнительно: `Option<StreamBudgetV1>`.
- `transport_hints` дополнительно: `Option<Vec<TransportHintV1>>`.
- Ambos Campos ahora fluyen por `ProviderAdmissionProposalV1`, лос конверты де гобернанса,
  данные о средствах CLI и телеметрии JSON.

## Проверка и винкуляция с правительством

`ProviderAdvertBodyV1::validate` и `ProviderAdmissionProposalV1::validate`
неверные метаданные rechazan:

- Возможности декодирования ранга и ограничения границ диапазона/детализации.
- Бюджеты потоков/подсказки по транспортировке требуют TLV `CapabilityType::ChunkRangeFetch`.
  совпадение и список подсказок не исчезли.
- Протоколы транспортировки дубликатов и приоритетов недействительности, связанных с ошибками проверки.
  до того, как о рекламе сплетничают.
- Конверты для сравнения предложений/рекламных объявлений для метаданных рангоу через
  `compare_core_fields` для того, чтобы полезные данные сплетен были опустошены, и они были восстановлены вовремя.

La cobertura de regresión vive en
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Инструменты и приспособления- Полезные данные объявлений поставщика включают метаданные `range_capability`,
  `stream_budget` и `transport_hints`. Действительно через ответы `/v2/sorafs/providers` y
  входное оборудование; резюме в формате JSON должны включать в себя возможность анализа,
  бюджет потока и массивы подсказок для приема телеметрии.
- `cargo xtask sorafs-admission-fixtures` отображает бюджеты потоков и подсказки по транспортировке.
  наши артефакты JSON для того, чтобы панели мониторинга могли использовать эту функцию.
- В комплект поставки входят светильники bajo `fixtures/sorafs_manifest/provider_admission/`:
  - реклама канонических мульти-оригиналов,
  - альтернативный вариант без ранго для перехода на более раннюю версию, y
  - `multi_fetch_plan.json` для воспроизведения наборов SDK по плану выборки.
    многоранговая детерминированность.

## Интеграция с орвестором и Torii

- Torii `/v2/sorafs/providers` Расширение метаданных емкости ранго синтаксического анализа
  `stream_budget` и `transport_hints`. Вы не согласны с объявлениями о понижении рейтинга, когда
  Проверяйте новые метаданные, а также конечные точки ранго-дель-шлюза.
  неправильные ограничения для прямых клиентов.
- Многопрофильный оркератор (`sorafs_car::multi_fetch`) сейчас имеет ограничения
  Ранго, распределение мощностей и поток бюджетов на работу. Лас-пруэбас унитариас
  cubren сценарии демасиадо больших блоков, быстрое рассеивание и регулирование.
- `sorafs_car::multi_fetch` выдает сигналы понижения версии (fallos de alineación,
  заботы задушены) для того, чтобы операдоры были растренированы, потому что их опускают
  проверяйте особые условия во время планирования.

## Справочная информация по телеметрии

Инструмент выборки рангом Torii питания приборной панели Grafana
**SoraFS Получить наблюдаемость** (`dashboards/grafana/sorafs_fetch_observability.json`) y
правила оповещения о партнерстве (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Метрика | Типо | Этикет | Описание |
|---------|------|-----------|-------------|
| `torii_sorafs_provider_range_capability_total` | Калибр | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Поставщики, сообщающие о возможностях рангом. |
| `torii_sorafs_range_fetch_throttle_events_total` | Счетчик | `reason` (`quota`, `concurrency`, `byte_rate`) | Намерения получить ранго с удушением политических агрессоров. |
| `torii_sorafs_range_fetch_concurrency_current` | Калибр | — | Активные потоки защищают то, что потребляется в результате предположения о параллельном соперничестве. |

Примеры PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Используйте контроллер регулирования для подтверждения применения квот перед хабилитаром
Los valores por дефекто дель orquestador multi-origen y alerta cuando la concurrencia se
поднимите максимум предположений о потоках вашего флота.