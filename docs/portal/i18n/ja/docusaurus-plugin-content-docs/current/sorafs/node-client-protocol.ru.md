---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Протокол SoraFS узел ↔ клиент

Этот гид резюмирует каноническое определение протокола в
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)。
アップストリーム-最低のバージョン Norito レイアウトと変更ログ'ов;
Runbook SoraFS を実行してください。

## Объявления провайдера и валидация

Провайдеры SoraFS ペイロード `ProviderAdvertV1` (см.
`crates/sorafs_manifest::provider_advert`) を確認してください。
Объявления фиксируют метаданные обнаружения и ガードレール、которые
мульти-источниковый оркестратор применяет на рантайме.

- **Срок действия** — `issued_at < expires_at ≤ issued_at + 86 400 s`。 Провайдеры
  12 月 12 日。
- **能力 TLV** — TLV-список рекламирует транспортные возможности (Torii,
  QUIC+Noise、SoraNet リレー、ベンダー拡張機能）。 Неизвестные коды можно
  `allow_unknown_capabilities = true`、今日の天気
  グリース。
- **QoS ヒント** — 層 `availability` (ホット/ウォーム/コールド)、максимальная задержка
  ストリームの予算を確認します。 QoS の定義
  入場料が必要です。
- **エンドポイントとランデブー トピック** — サービス URL と TLS/ALPN
  発見トピックス、トピックス、トピックス、トピックスなど
  ガードセット。
- **Политика разнообразия путей** — `min_guard_weight`、ファンアウト デバイス
  AS/пулов и `provider_failure_threshold` обеспечивают детерминированные
  マルチピアフェッチ。
- **Идентификаторы профилей** — провайдеры обязаны публиковать канонический
  ハンドル (`sorafs.sf1@1.0.0`); опциональные `profile_aliases` помогают
  миграции старых клиентов.

ステーク、機能/エンドポイント/トピックの詳細、
QoS ターゲットを確認します。入学封筒
(`compare_core_fields`) を確認してください。
распространением обновлений。

### 範囲フェッチ

範囲 - 最小値:

| Поле | Назначение |
|------|-----------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`、`min_granularity` と、флаги выравнивания/доказательств。 |
| `StreamBudgetV1` |エンベロープ конкурентности/スループット (`max_in_flight`、`max_bytes_per_sec`、опциональный `burst`)。射程能力が優れています。 |
| `TransportHintV1` | Упорядоченные предпочтения транспорта (например、`torii_http_range`、`quic_stream`、`soranet_relay`)。 Приоритеты `0–15`, дубли отклоняются. |

ツールの例:

- プロバイダーの広告範囲、ストリーム バジェット
  および輸送のヒント、ペイロードに関する情報。
- `cargo xtask sorafs-admission-fixtures` はマルチソースをサポートします
  広告「ダウングレードの備品」
  `fixtures/sorafs_manifest/provider_admission/`。
- 範囲広告 без `stream_budget` または `transport_hints` отклоняются загрузчиками
  CLI/SDK は、マルチソース ハーネスをサポートします。
  ожиданиями 入場 Torii。

## 範囲エンドポイント ゲートウェイ

ゲートウェイは、HTTP 接続、отражающие метаданные объявлений をサポートします。

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Требование |ださい |
|-----------|----------|
| **ヘッダー** | `Range` (チャンク オフセット、チャンク オフセット)、`dag-scope: block`、`X-SoraFS-Chunker`、опциональный `X-SoraFS-Nonce` およびBase64 `X-SoraFS-Stream-Token`。 |
| **回答** | `206` 、 `Content-Type: application/vnd.ipld.car`、`Content-Range`、 описывающим выданный интервал、метаданными `X-Sora-Chunk-Range` および эхо ヘッダーチャンカー/トークン。 |
| **障害モード** | `416` 日/日、`401` 日/日、 `429` ストリーム/バイト バジェット。 |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

ヘッダーとダイジェスト チャンクを取得します。
フォレンジック ダウンロード、CAR スライスの作成も可能です。

## ワークフロー мульти-источникового оркестратора

SF-6 マルチソースフェッチ (Rust CLI через `sorafs_fetch`,
SDK `sorafs_orchestrator`):1. **Собрать входные данные** — декодировать план chunk'ов マニフェスト、получить
   広告と、テレメトリのスナップショットを表示
   (`--telemetry-json` または `TelemetrySnapshot`)。
2. **スコアボード** — `Orchestrator::build_scoreboard` оценивает
   пригодность и записывает причины отказа; `sorafs_fetch --scoreboard-out`
   JSON を使用します。
3. **チャンク** — `fetch_with_scoreboard` (`--plan`) の値
   範囲、ストリーム バジェット、再試行/ピア (`--retry-budget`、
   `--max-peers`) とストリーム トークンとスコープ マニフェストの組み合わせ。
4. **領収書** — результаты включают `chunk_receipts` и
   `provider_reports`; CLI の概要 `provider_reports`、`chunk_receipts`
   `ineligible_providers` 証拠バンドル。

Распространенные обки、возвращаемые операторам/SDK:

| Осибка | Описание |
|----------|----------|
| `no providers were supplied` | Нет подходящих записей после фильтрации. |
| `no compatible providers available for chunk {index}` |チャンクを確認してください。 |
| `retry budget exhausted after {attempts}` | `--retry-budget` はピアです。 |
| `no healthy providers remaining` | Все провайдеры отключены после повторных отказов. |
| `streaming observer failed` |ダウンストリーム CAR ライター заверсился с олибкой。 |
| `orchestrator invariant violated` |マニフェスト、スコアボード、テレメトリ スナップショット、および CLI JSON トリアージをサポートします。 |

## Телеметрия и доказательства

- Метрики、эмитируемые оркестратором:  
  `sorafs_orchestrator_active_fetches`、`sorafs_orchestrator_fetch_duration_ms`、
  `sorafs_orchestrator_retries_total`、`sorafs_orchestrator_provider_failures_total`
  (マニフェスト/リージョン/プロバイダー)。 `telemetry_region` の構成
  через CLI флаги、чтобы далаборды разделяли по флоту。
- CLI/SDK フェッチ概要、スコアボード JSON、チャンク レシート、
  プロバイダーのレポート、SF-6/SF-7 のロールアウト バンドル、ゲートのレポート。
- ゲートウェイ ハンドラー `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`、
  SRE ダッシュボードが表示されます。

## CLI と REST の組み合わせ

- `iroha app sorafs pin list|show`、`alias list`、`replication list` оборачивают
  pin-registry REST エンドポイントと認証 Norito JSON 認証
  для аудиторских доказательств。
- `iroha app sorafs storage pin` および `torii /v1/sorafs/pin/register` および Norito
  JSON マニフェストは、別名証明と後継者を表します。不正な証明
  `400`、古いプルーフ、`503` と `Warning: 110`、完全に期限切れのプルーフ
  `412`。
- REST エンドポイント (`/v1/sorafs/pin`、`/v1/sorafs/aliases`、
  `/v1/sorafs/replication`) 認証、認証、認証、認証
  ブロック ヘッダーを確認できます。

## Ссылки

- 最低料金:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito メッセージ: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI の例: `crates/iroha_cli/src/commands/sorafs.rs`、
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- 木箱: `crates/sorafs_orchestrator`
- バージョン: `dashboards/grafana/sorafs_fetch_observability.json`