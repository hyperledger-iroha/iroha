---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس پرووائیڈر реклама اور شیڈولنگ

В качестве примера можно привести каноническую спецификацию, которая может быть использована в следующих случаях:
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Схемы Norito в журналах изменений, которые могут быть изменены в соответствии с вашими требованиями. портал کی کاپی
Используйте SDK для создания резервных копий Runbook SoraFS. کے قریب رکھتی ہے۔

## Norito схема میں اضافے

### Диапазон возможностей (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – в зависимости от диапазона (байтов) `>= 1`.
- `min_granularity` – найдите ریزولوشن, `1 <= قدر <= max_chunk_span`.
- `supports_sparse_offsets` – Если вы хотите использовать компенсаторы, которые вы хотите использовать,
- `requires_alignment` – если true и смещения `min_granularity` можно выровнять по выравниванию.
- `supports_merkle_proof` – свидетель PoR کی سپورٹ ظاہر کرتا ہے۔

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` каноническая кодировка
Детерминированные полезные данные для сплетен رہیں۔

### `StreamBudgetV1`
- Доступны: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes`.
- Правила валидации (`StreamBudgetV1::validate`):
  - И18НИ00000027Х, И18НИ00000028Х.
  - `burst_bytes` موجود ہو تو `> 0` اور `<= max_bytes_per_sec` ہونا چاہیے۔

### `TransportHintV1`
- Коды: `protocol: TransportProtocol`, `priority: u8` (0-15 дней).
  `TransportHintV1::validate` здесь.
- Внутренние протоколы: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Доступ к записям протокола поставщика услуг

### `ProviderAdvertBodyV1` میں اضافے
- اختیاری `stream_budget: Option<StreamBudgetV1>`.
- اختیاری `transport_hints: Option<Vec<TransportHintV1>>`.
- Используйте `ProviderAdmissionProposalV1`, конверты управления, приспособления CLI, телеметрический JSON и другие функции.

## Проверка привязки управления

`ProviderAdvertBodyV1::validate` или `ProviderAdmissionProposalV1::validate`
Метаданные, которые можно использовать:

- Возможности диапазона декодирования и ограничения диапазона/детализации.
- Бюджеты потоков / подсказки по транспортировке کے لیے `CapabilityType::ChunkRangeFetch` TLV اور непустой список подсказок لازم ہے۔
- Транспортные протоколы اور غیر درست приоритеты сплетни سے پہلے ошибки проверки
- Приемные конверты `compare_core_fields` کے ذریعے предложение/рекламные объявления کے диапазон метаданных کو сравнить کرتے ہیں تاکہ несоответствие полезных данных сплетен جلدی مسترد ہوں۔

Покрытие регрессии в следующих случаях:
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Инструменты и приспособления

- Полезные данные для рекламы поставщика: `range_capability`, `stream_budget`, `transport_hints` в режиме реального времени.
  `/v1/sorafs/providers` ответы на входные приспособления کے ذریعے проверить کریں؛ Сводки JSON, возможности синтаксического анализа, бюджет потока, массивы подсказок, а также возможность приема данных телеметрии.
- `cargo xtask sorafs-admission-fixtures` Артефакты JSON, потоковые бюджеты и подсказки по транспорту, а также информационные панели с функцией отслеживания внедрения.
- `fixtures/sorafs_manifest/provider_admission/` Какие светильники есть в наличии:
  - каноническая реклама из нескольких источников,
  - `multi_fetch_plan.json` Наборы SDK, воспроизведение плана детерминированной многоранговой выборки.

## Оркестратор Torii انضمام- Torii `/v1/sorafs/providers` анализируемые метаданные о возможностях диапазона.
  провайдеры и метаданные, а также предупреждения о переходе на более раннюю версию и диапазон шлюзов, конечные точки и ограничения клиентов. کرتے ہیں۔
- Оркестратор с несколькими источниками (`sorafs_car::multi_fetch`) с ограничениями диапазона, выравниванием возможностей, потоковыми бюджетами и рабочими назначениями, обеспечивающими соблюдение требований. Модульные тесты, слишком большой фрагмент, разреженный поиск и сценарии регулирования.
- Сигналы понижения версии `sorafs_car::multi_fetch` (ошибки выравнивания, регулируемые запросы) потоковые операторы и поставщики услуг Пропустить ہوئے۔

## Справочник по телеметрии

Torii Инструментарий выборки диапазона **SoraFS Выборка наблюдаемости** Grafana Панель мониторинга
(`dashboards/grafana/sorafs_fetch_observability.json`) Дополнительные правила оповещений
(`dashboards/alerts/sorafs_fetch_rules.yml`) Как кормить کرتی ہے۔

| Метрическая | Тип | Этикетки | Описание |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Калибр | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Функции диапазона возможностей рекламируют лучших поставщиков услуг. |
| `torii_sorafs_range_fetch_throttle_events_total` | Счетчик | `reason` (`quota`, `concurrency`, `byte_rate`) | Политика ограничения попыток выборки диапазона. |
| `torii_sorafs_range_fetch_concurrency_current` | Калибр | — | Общий бюджет параллелизма и активные защищенные потоки. |

Пример фрагментов PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Применение квот и счетчик регулирования آپ کے флот کے потоковый бюджет максимум کے قریب ہو تو alert کریں۔