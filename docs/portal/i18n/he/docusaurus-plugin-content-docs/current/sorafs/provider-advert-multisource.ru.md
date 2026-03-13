---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Мульти-источниковые объявления провайдеров и планирование

Эта страница сводит каноническую спецификацию в
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Используйте этот документ для дословных схем Norito ו-changelog; версия портала
держит рядом операционные инструкции, заметки SDK ו-ссылки на телеметрию для остального
набора runbooks SoraFS.

## Дополнения к схеме Norito

### Диапазонная возможность (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – максимальный непрерывный интервал (байты) על ספרוס, `>= 1`.
- `min_granularity` – разрешение seek, `1 <= значение <= max_chunk_span`.
- `supports_sparse_offsets` – קיזוזים של אופציונליים חדשים.
- `requires_alignment` – если true, offsets должны выравниваться по `min_granularity`.
- `supports_merkle_proof` – указывает поддержку свидетельств PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` обеспечивают каноническое
кодирование, чтобы מטענים רכילות оставались детерминированными.

### `StreamBudgetV1`
- תמונה: `max_in_flight`, `max_bytes_per_sec`, אופציונלי `burst_bytes`.
- Правила валидации (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, если задан, должен быть `> 0` ו-`<= max_bytes_per_sec`.

### `TransportHintV1`
- פוליא: `protocol: TransportProtocol`, `priority: u8` (אוקנו 0-15 контролируется
  `TransportHintV1::validate`).
- פרוטוקולים: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Дублирующиеся записи протоколов на провайдера отклоняются.

### Дополнения к `ProviderAdvertBodyV1`
- Опциональный `stream_budget: Option<StreamBudgetV1>`.
- Опциональный `transport_hints: Option<Vec<TransportHintV1>>`.
- Оба поля проходят через `ProviderAdmissionProposalV1`, מעטפות ממשל,
  גופי CLI ו-JSON טלאים.

## Валидация и привязка לממשל

`ProviderAdvertBodyV1::validate` ו-`ProviderAdmissionProposalV1::validate`
отклоняют поврежденные метаданные:

- Диапазонные возможности должны корректно декодироваться и соблюдать лимиты
  диапазона/гранулярности.
- זרם תקציבים / רמזים לתחבורה требуют TLV `CapabilityType::ChunkRangeFetch`
  и непустого списка רמזים.
- Дублирующиеся транспортные протоколы и некорректные приоритеты вызывают ошибки
  валидации до רכילות рассылки פרסומות.
- מעטפות כניסה להצעה/מודעות по диапазонным метаданным через
  `compare_core_fields`, чтобы несовпадающие מטעני רכילות отклонялись заранее.

Регрессионное покрытие находится в
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## מכשירים וגופים

- מטענים объявлений провайдеров должны включать `range_capability`, `stream_budget`
  ו `transport_hints`. Проверяйте через ответы `/v2/sorafs/providers` и מתקני כניסה;
  JSON-резюме должны включать разобранную, תקציב זרימה ורמזים למחשבים
  для телеметрического ingest.
- `cargo xtask sorafs-admission-fixtures` מאפשר תקציבי זרם ורמזים לתחבורה
  в своих חפצי JSON, чтобы לוחות מחוונים отслеживали внедрение функций.
- מתקנים ב-`fixtures/sorafs_manifest/provider_admission/` теперь включают:
  - מודעות канонические мульти-источниковые,
  - `multi_fetch_plan.json`, чтобы SDK наборы могли воспроизводить детерминированный
    רב-עמית אחזור план.

## Интеграция с оркестратором и Torii- Torii `/v2/sorafs/providers` возвращает разобранные метаданные диапазонных возможностей
  вместе с `stream_budget` ו-`transport_hints`. דירוג לאחור של Предупреждения срабатывают, когда
  провайдеры пропускают новые метаданные, а טווח נקודות קצה шлюза применяют те же ограничения
  для прямых клиентов.
- Мульти-источниковый оркестратор (`sorafs_car::multi_fetch`) теперь применяет лимиты диапазона,
  выравнивание возможностей и תקציבי זרם при распределении работы. Unit-тесты покрывают
  случаи слишком больших chunk, разреженного seek и throttling.
- `sorafs_car::multi_fetch` передает сигналы שדרוג לאחור (ошибки выравнивания,
  גזירות מצערות), чтобы операторы могли понимать, почему конкретные провайдеры
  были пропущены при планировании.

## Справочник телеметрии

אחזור טווח הפעלה ב-Torii питает לוח המחוונים של Grafana **SoraFS יכולת התצפית של אחזור**
(`dashboards/grafana/sorafs_fetch_observability.json`) и соответствующие правила алертов
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| Метрика | טיפ | Метки | Описание |
|--------|-----|-------|--------|
| `torii_sorafs_provider_range_capability_total` | מד | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Провайдеры, объявляющие функции диапазонной возможности. |
| `torii_sorafs_range_fetch_throttle_events_total` | מונה | `reason` (`quota`, `concurrency`, `byte_rate`) | טווח טווחי מצערת, сгруппированные по политике. |
| `torii_sorafs_range_fetch_concurrency_current` | מד | — | Активные защищенные потоки, потребляющие общий бюджет конкуренции. |

דגמי PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Используйте счетчик מצערת, чтобы подтвердить применение квот перед включением
дефолтов мульти-источникового оркестратора, и поднимайте алерты, когда конкуренция
приближается к максимальным значениям תקציב זרם по вашей флотилии.