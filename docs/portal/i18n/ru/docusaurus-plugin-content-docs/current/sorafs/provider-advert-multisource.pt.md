---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Рекламные объявления различного происхождения и повестки дня

Эта страница возобновляет конкретную канонику в
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Используйте этот документ для схем Norito дословно и журналы изменений; портал копирования
ориентация для операторов, примечания к SDK и ссылки на телеметрию до востребования
dos runbooks SoraFS.

## Adicoes ao esquema Norito

### Диапазон возможностей (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` — основной диапазон (в байтах) по требованию, `>= 1`.
- `min_granularity` - разрешение поиска, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - разрешается смещать смежные области в требуемом месте.
- `requires_alignment` - когда true, компенсирует devem alinhar com `min_granularity`.
- `supports_merkle_proof` - индикатор поддержки тестового PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` кодировка aplicam canonico
для того, чтобы сплетни были постоянными.

### `StreamBudgetV1`
- Кампос: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` опционально.
- Проверка подлинности (`StreamBudgetV1::validate`):
  - И18НИ00000027Х, И18НИ00000028Х.
  - `burst_bytes`, когда представлено, было создано `> 0` и `<= max_bytes_per_sec`.

### `TransportHintV1`
- Кампос: `protocol: TransportProtocol`, `priority: u8` (приложение janela 0-15 для
  `TransportHintV1::validate`).
- Протоколы согласования: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Дубликаты протоколов для проверки безопасности.

### Adicoes a `ProviderAdvertBodyV1`
- `stream_budget` дополнительно: `Option<StreamBudgetV1>`.
- `transport_hints` дополнительно: `Option<Vec<TransportHintV1>>`.
- Ambos os Campos Agora Passam por `ProviderAdmissionProposalV1`, конверты правительства,
  средства CLI и JSON телеметрии.

## Валидасао и винкулакао с правительством

`ProviderAdvertBodyV1::validate` и `ProviderAdmissionProposalV1::validate`
неправильная форма метаданных rejeitam:

- Возможности диапазона могут быть декодированы и установлены ограничения диапазона/детализации.
- Бюджеты потоков / подсказки по транспортировке exigem um TLV `CapabilityType::ChunkRangeFetch`
  Корреспондент и список подсказок по всему миру.
- Протоколы транспортировки дубликатов и недействительные приоритеты в случае ошибок валидации.
  Antes de Adverts Серем сплетничали.
- Конверты приема сравнивают предложения/рекламные объявления с метаданными диапазона через
  `compare_core_fields` для того, чтобы полезные данные о сплетнях расходились сегодня, и это было сделано.

A cobertura de regressao vive em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Инструменты и приспособления

- Полезные данные рекламных объявлений, разработанные с метаданными `range_capability`,
  `stream_budget` и `transport_hints`. Действует через ответы `/v2/sorafs/providers`
  электронное входное оборудование; Резюме JSON разработаны с возможностью анализа или потоковой передачи
  бюджет и массивы подсказок для приема телеметрии.
- `cargo xtask sorafs-admission-fixtures` бюджеты большинства потоков и подсказки по транспортировке.
  свои артефакты JSON для информационных панелей, сопровождающих добавленную функцию.
- Светильники sob `fixtures/sorafs_manifest/provider_admission/` агоры включают:
  - реклама канонических мультиоригемов,
  - `multi_fetch_plan.json` для воспроизведения наборов SDK в плане выборки.
    многоранговый детерминированный.

## Интеграция с оркестратором и Torii- Torii `/v2/sorafs/providers` возврат метаданных диапазона анализируемого соединения com
  `stream_budget` и `transport_hints`. Предупреждение о переходе на более раннюю версию в любое время
  если опустить новые метаданные, а также конечные точки диапазона шлюза aplicam, как
  имеются ограничения для прямых клиентов.
- O оркестратор multi-orige (`sorafs_car::multi_fetch`) с ограниченными возможностями применения
  диапазон, сочетание возможностей и потокового бюджета, а также атрибуты труда. Единица
  тесты включают в себя множество сценариев, разреженный поиск и регулирование.
- `sorafs_car::multi_fetch` передает данные о понижении версии (falhas de alinhamento,
  Реквизиты ограничены) для того, чтобы операторы растрейем для того, чтобы обеспечить конкретные условия
  для невежд во время полета или полета.

## Справочная информация по телеметрии

Прибор для получения питания Torii или приборной панели Grafana
**SoraFS Получить наблюдаемость** (`dashboards/grafana/sorafs_fetch_observability.json`) e
как regras de alerta associadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Метрика | Типо | Этикетки | Описание |
|---------|------|--------|-----------|
| `torii_sorafs_provider_range_capability_total` | Калибр | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Предоставляет информацию о возможностях дальности действия. |
| `torii_sorafs_range_fetch_throttle_events_total` | Счетчик | `reason` (`quota`, `concurrency`, `byte_rate`) | Тентативы де-диапазона могут привести к ограничению политических интересов. |
| `torii_sorafs_range_fetch_concurrency_current` | Калибр | - | Потоки, защищающие потребление или бюджетные совместные расходы. |

Примеры PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Используйте кнопку регулирования для подтверждения применения квот перед активацией.
значения по умолчанию делают оркестратор многоуровневым и предупреждают, когда согласование происходит приблизительно
dos maximos делают потоковый бюджет из вашего бюджета.