---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Объявления о четырех источниках и планировании

Эта страница с кратким описанием канонической спецификации
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Используйте этот документ для схем Norito дословно и журналов изменений; копия порта
сохранять операторов-грузоотправителей, заметки SDK и ссылки телеметрии рядом с остальными
des runbooks SoraFS.

## Настройка схемы Norito

### Пляжная емкость (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – плюс большой непрерывный пляж (октеты) по запросу, `>= 1`.
- `min_granularity` – разрешение поиска, `1 <= valeur <= max_chunk_span`.
- `supports_sparse_offsets` – разрешено несмежное смещение в одном запросе.
- `requires_alignment` – верно, смещения будут выравниваться по `min_granularity`.
- `supports_merkle_proof` – Индикация приза, отвечающего за темпы PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` применение канонической кодировки
для того, чтобы сплетни оставались детерминированными.

### `StreamBudgetV1`
- Чемпионы: `max_in_flight`, `max_bytes_per_sec`, опция `burst_bytes`.
- Правила проверки (`StreamBudgetV1::validate`):
  - И18НИ00000027Х, И18НИ00000028Х.
  - `burst_bytes`, сейчас присутствует, doit etre `> 0` и `<= max_bytes_per_sec`.

### `TransportHintV1`
Чемпионы: `protocol: TransportProtocol`, `priority: u8` (окна 0–15, номинал аппликации).
  `TransportHintV1::validate`).
- Протоколы подключения: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Двойные протокольные входы для посетителей.

### Представляет собой `ProviderAdvertBodyV1`
- Опция `stream_budget`: `Option<StreamBudgetV1>`.
- Опция `transport_hints`: `Option<Vec<TransportHintV1>>`.
- Les deux champs transient desormais через `ProviderAdmissionProposalV1`, les enveloppes
  управления, приборов CLI и телеметрии JSON.

## Валидация и взаимодействие в сфере управления

`ProviderAdvertBodyV1::validate` и `ProviderAdmissionProposalV1::validate`
отвергающие метадоннеи плохих форм:

- Les capacites de plage doivent является декодером и соблюдает ограничения на пляж/гранулярит.
- Бюджеты потоков / подсказки по транспортировке, необходимые для TLV `CapabilityType::ChunkRangeFetch`
  корреспондент и список подсказок, не видимых.
- Двойные протоколы транспортировки и недействительные приоритеты, возникающие из-за ошибок
  проверка перед распространением рекламы.
- Сравнительное предложение о приеме на конверты/рекламные объявления для метадоннеес де пляж через
  `compare_core_fields` afin que les payloads de сплетни не согласны, поэтому все отвергается.

La couverture de repression se trouve dans
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Уборка и сантехника- Les payloads d'annonces de fournisseurs doivent включают метадоннеи `range_capability`,
  `stream_budget` и `transport_hints`. Подтвердите через ответы `/v2/sorafs/providers` и др.
  приспособления для приема; Резюме JSON doivent включают анализ емкости, бюджет потока
  и таблицы подсказок для телеметрии приема пищи.
- `cargo xtask sorafs-admission-fixtures` раскрывает бюджеты потоков и подсказки по транспортировке в них.
  Эти артефакты JSON нужны для того, чтобы панели мониторинга соответствовали функциональному принятию.
- Светильники с `fixtures/sorafs_manifest/provider_admission/` включают в себя:
  - канонические объявления с несколькими источниками,
  - `multi_fetch_plan.json` для того, чтобы пакеты SDK могли обновить план выборки.
    многоранговый детерминированный.

## Интеграция с оркестратором и Torii

- Torii `/v2/sorafs/providers` отправка метадонов в емкость пляжа с парами
  `stream_budget` и `transport_hints`. Предупреждения о переходе на более раннюю версию исчезают, когда
  les fournisseurs omettent la nouvelle метадонне и конечные точки пляжа дю шлюза
  применение мемов против клиентов.
- L'orchestrateur multi-source (`sorafs_car::multi_fetch`) аппликация desormais les limites de
  пляж, выравнивание мощностей и потоки бюджетов для облегчения труда.
  Унитарные тесты сочетают в себе сценарии больших кусков, поиск рассеивает и т. д.
  дросселирование.
- `sorafs_car::multi_fetch` диффузные сигналы понижения версии (echecs d'alignement,
  требует дросселирования) для того, чтобы операторы могли использовать трассировщик для определенных действий
  ont ete игнорирует подвеску la planification.

## Справочник по телеметрии

Инструмент для извлечения пляжа Torii обеспечивает питание приборной панели Grafana
**SoraFS Получить наблюдаемость** (`dashboards/grafana/sorafs_fetch_observability.json`) и др.
правила оповещения ассоциации (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Метрика | Тип | Этикет | Описание |
|----------|------|------------|-------------|
| `torii_sorafs_provider_range_capability_total` | Калибр | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Обратите внимание на функциональные возможности пляжа. |
| `torii_sorafs_range_fetch_throttle_events_total` | Счетчик | `reason` (`quota`, `concurrency`, `byte_rate`) | Политические соображения по выбору невесты на пляже. |
| `torii_sorafs_range_fetch_concurrency_current` | Калибр | — | Потоки действуют в рамках согласованного бюджета. |

Примеры PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Использование счетчика регулирования для подтверждения предварительного применения квот
les valeurs par defaut de l'orchestrateur с несколькими источниками и оповещения о совпадении
сближение максимальных бюджетных потоков для вашего флота.