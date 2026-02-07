---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Мульти-источниковые объявления провайдеров и планирование

Эта страница сводит каноническую спецификацию в
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
Используйте этот документ для дословных схем Norito и Changelog; और देखें
SDK を使用して、SDK を使用して、必要な情報を取得します。
ランブック SoraFS。

## Дополнения к схеме Norito

### Диапазонная возможность (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – максимальный непрерывный интервал (байты) на запрос、`>= 1`。
- `min_granularity` – シーク、`1 <= значение <= max_chunk_span`。
- `supports_sparse_offsets` – オフセットを指定します。
- `requires_alignment` – true、オフセットは `min_granularity` です。
- `supports_merkle_proof` – указывает поддержку свидетельств PoR。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` обеспечивают каноническое
кодирование、чтобы ペイロードのゴシップ оставались детерминированными。

### `StreamBudgetV1`
- Поля: `max_in_flight`、`max_bytes_per_sec`、опциональный `burst_bytes`。
- Правила валидации (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`、`max_bytes_per_sec > 0`。
  - `burst_bytes`、`> 0` および `<= max_bytes_per_sec`。

### `TransportHintV1`
- Поля: `protocol: TransportProtocol`、`priority: u8` (окно 0-15 контролируется)
  `TransportHintV1::validate`)。
- バージョン: `torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- Дублирующиеся записи протоколов на провайдера отклоняются.

### Дополнения к `ProviderAdvertBodyV1`
- Опциональный `stream_budget: Option<StreamBudgetV1>`。
- Опциональный `transport_hints: Option<Vec<TransportHintV1>>`。
- Оба поля проходят через `ProviderAdmissionProposalV1`、ガバナンス エンベロープ、
  CLI フィクスチャと телеметрический JSON。

## Валидация и привязка к ガバナンス

`ProviderAdvertBodyV1::validate` および `ProviderAdmissionProposalV1::validate`
отклоняют поврежденные метаданные:

- Диапазонные возможности должны корректно декодироваться и соблюдать лимиты
  диапазона/гранулярности。
- ストリームの予算/トランスポートのヒント TLV `CapabilityType::ChunkRangeFetch`
  и непустого списка ヒント。
- Дублирующиеся транспортные протоколы и некорректные приоритеты вызывают обки
  ゴシップ広告。
- 入場用封筒、提案書/広告、 метаданным через
  `compare_core_fields`、ゴシップ ペイロードが表示されます。

Регрессионное покрытие находится в
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## Инструменты および 備品

- ペイロード объявлений провайдеров должны включать `range_capability`、`stream_budget`
  `transport_hints`。 Проверяйте через ответы `/v1/sorafs/providers` および入場設備。
  JSON-резюме должны включать разобранную 能力、ストリームバジェット、およびヒント
  取り込みます。
- `cargo xtask sorafs-admission-fixtures` ストリーム バジェットとトランスポート ヒント
  JSON アーティファクト、ダッシュボードの機能を備えています。
- `fixtures/sorafs_manifest/provider_admission/` の備品:
  - канонические мульти-источниковые 広告、
  - `multi_fetch_plan.json`、SDK のバージョンが更新されました。
    マルチピアフェッチ機能。

## Интеграция с оркестратором и Torii

- Torii `/v1/sorafs/providers` を表示します。
  `stream_budget` または `transport_hints` です。 Предупреждения ダウングレード срабатывают, когда
  範囲エンドポイントの範囲のエンドポイントを取得します。
  для прямых клиентов.
- Мульти-источниковый оркестратор (`sorafs_car::multi_fetch`) теперь применяет лимиты диапазона,
  ストリーム予算が表示されます。 Unit-тесты покрывают
  チャンク、スロットリングをシークします。
- `sorafs_car::multi_fetch` передает сигналы ダウングレード (овибки выравнивания,
  スロットル запросы)、чтобы операторы могли понимать、почему конкретные провайдеры
  問題はそれです。

## Справочник телеметрии

Torii による範囲フェッチ Grafana ダッシュボード **SoraFS フェッチ オブザーバビリティ**
(`dashboards/grafana/sorafs_fetch_observability.json`) と表示されます。
(`dashboards/alerts/sorafs_fetch_rules.yml`)。

| Метрика | Тип | Метки | Описание |
|----------|-----|----------|----------|
| `torii_sorafs_provider_range_capability_total` |ゲージ | `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`) Провайдеры、объявляющие функции диапазонной возможности。 |
| `torii_sorafs_range_fetch_throttle_events_total` |カウンター | `reason` (`quota`、`concurrency`、`byte_rate`) |範囲フェッチとスロットリングを実行し、スロットルを実行します。 |
| `torii_sorafs_range_fetch_concurrency_current` |ゲージ | — | Активные защищенные потоки, потребляющие общий бюджет конкуренции. |

PromQL の例:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```スロットリング、スロットリング、スロットリング、スロットリングなどの機能を備えています。
дефолтов мульти-источникового оркестратора, и поднимайте алерты, когда конкуренция
ストリームの予算を確認できます。