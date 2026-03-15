<!-- Japanese translation of docs/source/references/ios_metrics.md -->

---
lang: ja
direction: ltr
source: docs/source/references/ios_metrics.md
status: complete
translator: manual
---

<!--
Swift ダッシュボードメトリクスのロードマップ参照。翻訳と同期すること:
docs/source/references/ios_metrics.ja.md
docs/source/references/ios_metrics.he.md
-->

# Swift ダッシュボードのメトリクス（パリティ & CI）

本メモでは Swift SDK ダッシュボードで使用される JSON スキーマとツール群を説明します。ロードマップおよび `status.md` で参照されるパリティ／CI ビューにデータを供給するテレメトリエクスポータの契約として機能します。併せて `docs/source/swift_xcframework_device_matrix.md`（レーン構成）、パリティトリアージ手順、CI 運用ガイドを参照し、`device_tag` メタデータと SLA しきい値が Buildkite・ダッシュボード・当番ワークフローへどのように伝搬するかを把握してください。

## フィード

- `dashboards/data/mobile_parity.sample.json`
- `dashboards/data/mobile_ci.sample.json`
- `dashboards/data/swift_schema.sample.json`
- `dashboards/data/swift_salt_status.sample.json`
- `dashboards/data/mobile_pipeline_metadata.sample.json`

サンプルファイルは期待される形を文書化するためリポジトリに同梱されています。実運用のフィードはテレメトリエクスポータによって生成し、同じディレクトリに配置してください（リポジトリは `.sample.json` 以外を無視します）。

## `mobile_parity` スキーマ

| フィールド | 型 | 説明 |
|------------|----|------|
| `generated_at` | 文字列（ISO 8601） | スナップショット生成時刻。 |
| `fixtures` | オブジェクト | Norito パリティ差分の保留状況と最古エイジ。 |
| `pipeline` | オブジェクト | `/v2/pipeline` 連携の状態（最終実行、失敗情報）。 |
| `pipeline.metadata` | オブジェクト（任意） | `MOBILE_PARITY_PIPELINE_METADATA` で共有されるジョブ／テストの所要時間 (`job_name`, `duration_seconds`, `tests[]`)。`metadata_source` は監査用のパス／URL を記録します。 |
| `regen_sla` | オブジェクト | フィクスチャ再生成 SLA の連続達成状況。 |
| `alerts` | 配列 | 任意のアラートエントリ。 |
| `acceleration` | オブジェクト（任意） | Metal／NEON／StrongBox のパリティ＆性能メタデータ（`enabled`, `parity`, `perf_delta_pct`）。 |
| `telemetry` | オブジェクト（任意） | レダクション状況のサマリ（`salt_epoch`, 回転経過時間, `overrides_open`, `device_profile_alignment`, `schema_version`, `notes`）。 |

## `mobile_ci` スキーマ

| フィールド | 型 | 説明 |
|------------|----|------|
| `generated_at` | 文字列（ISO 8601） | スナップショット生成時刻。 |
| `buildkite` | オブジェクト | レーン成功率、キュー深度、実行時間。 |
| `devices` | オブジェクト | エミュレータ vs StrongBox プールの成功／失敗数。 |
| `alert_state` | オブジェクト | 連続失敗カウンタと未解決インシデント。 |
| `acceleration_bench` | オブジェクト（任意） | Metal と CPU または NEON スループットの比較ベンチマーク。 |

### `buildkite.lanes[]`

| フィールド | 型 | 説明 |
|------------|----|------|
| `name` | 文字列 | レーン識別子（例: `ci/xcode-swift-parity`, `ci/xcframework-smoke:iphone-sim`）。 |
| `success_rate_14_runs` | 数値 | 直近 14 回の成功率（0〜1）。 |
| `last_failure` | 文字列（ISO 8601） または `null` | 最新失敗時刻。 |
| `flake_count` | 整数 | 既知のフレーク件数（手動入力）。 |
| `mttr_hours` | 数値 | 平均復旧時間（時間）。 |
| `device_tag` | 文字列（任意） | ダッシュボード用の短いデバイスタグ（`iphone-sim`, `strongbox`, `mac-fallback` など）。 |

## `connect.error` イベントスキーマ

Swift の `ConnectError` ラッパー（`docs/source/connect_error_taxonomy.md`）は
`telemetryAttributes(fatal:httpStatus:)` を公開しており、エクスポータは属性を
そのまま OTEL イベントとして送信します。各イベントに含める属性は次の通りです。

| 属性 | 型 | 説明 |
|------|----|------|
| `category` | 文字列 | `transport`/`codec`/`authorization`/`timeout`/`queueOverflow`/`internal` のいずれか。 |
| `code` | 文字列 | 安定した識別子（例: `client.closed`, `network.timeout`）。 |
| `fatal` | 文字列 (`"true"`/`"false"`) | エラーがセッションを終了させたかどうか。 |
| `http_status` | 文字列（任意） | 対応する HTTP / WebSocket ステータス。 |
| `underlying` | 文字列（任意） | ラップされた基礎エラーのデバッグ表現。 |
| `sid` | 文字列（任意） | 必要に応じたセッション ID（ハッシュ済み推奨）。 |
| `queue_depth` | 整数（任意） | 発生時のキュー深度（オーバーフロー分析向け）。 |

エラーをアプリ層へ伝える際に 1 度だけイベントを発火し、過剰なノイズを避けてください。
ダッシュボード側では `connect.queue_depth` や再接続ヒストグラムと突き合わせて傾向を把握します。

## ツール

- ダッシュボードのレンダリング: `make swift-dashboards`（スキーマ検証＋Swift レンダラ実行）。
  `SWIFT_PARITY_FEED` / `SWIFT_CI_FEED` 環境変数を指定すれば、サンプルではなく実際のフィードを
  検証・レンダリングできます（未設定の場合は `.sample` を利用）。
- CI エントリポイント: `ci/check_swift_dashboards.sh`。
- 手動レンダリング: `scripts/render_swift_dashboards.sh [/path/to/parity.json [/path/to/ci.json]]`。
- スキーマ検証: `scripts/check_swift_dashboard_data.py <files…>`。
- テレメトリ拡張: `scripts/swift_enrich_parity_feed.py --input parity.json --salt-epoch …` を用いて
  salt 回転情報や override 数をフィードへ注入し、`ci/swift_status_export.sh` から環境変数で呼び出します。
- テレメトリ収集: `scripts/swift_collect_redaction_status.py --salt-config dashboards/data/swift_salt_status.sample.json --overrides-store artifacts/swift_telemetry_overrides.json`
  が salt/override/アラインメント情報から `telemetry` ブロックを自動生成します。
- Override 管理: `python3 scripts/swift_status_export.py telemetry-override {list,create,revoke}`
  （または従来の `scripts/swift_telemetry_override.py`）で `artifacts/swift_telemetry_overrides.json`
  を更新し、マスクされたロールと期限付き overrides を記録します。
- JSON スキーマ: `docs/source/references/ios_metrics.schema.json`（`make swift-dashboards` 内で `python3 -m jsonschema …` を実行）。
- デバイスカバレッジ参照: `docs/source/swift_xcframework_device_matrix.md` に Buildkite レーンマトリクスを定義。

## Prometheus export (`swift_parity_success_total`)

- `swift_status_export.py` は `--metrics-path /path/to/swift_status.prom` を指定すると Prometheus テキストファイルメトリクスを出力できます。
  エクスポータは `swift_parity_success_total` および `swift_parity_failure_total` カウンタを JSON 状態ファイル（既定は
  `artifacts/swift_status_metrics_state.json`、`--metrics-state …` で上書き可能）で管理します。
- 直近のパリティスナップショットに未解決差分が無く、`regen_sla.breach` が偽で、`/v2/pipeline` の失敗テストも無い場合に
  `swift_parity_success_total` がインクリメントされます。
- テキストファイルには現在のステータス（`swift_parity_status`）、未解決差分数、最古差分時間、最後の再生成からの経過時間のゲージも含まれ、
  textfile コレクタやオンコールダッシュボードがフィクスチャ鮮度の健全性を監視できます。
- Telemetry ブロックが含まれている場合は次のゲージも出力されます:
  - `swift_telemetry_overrides_open` — 最新スナップショットでアクティブなレダクション override 件数を示すゲージ。
  - `swift_telemetry_salt_rotation_age_hours` — 直近のソルトローテーションからの経過時間（時間単位）を記録するゲージ。

## エクスポータガイドライン

1. 上記スキーマと一致する有効な UTF-8 JSON を必ず出力すること。
2. 任意ブロック（`acceleration`, `acceleration_bench`）はデータが無い場合に省略すること。
3. タイムスタンプは UTC（`Z` 付き ISO 8601）で統一すること。
4. ベンチマーク値は生の数値（ミリ秒や MB/s など）で出力し、Swift レンダラ側で可読文字列に変換できるようにする。
5. フィードは安全な場所（S3 やビルド成果物など）へアップロードし、レポートジョブで `dashboards/data/` に同期させること。リポジトリはサンプル以外を無視するため、エクスポータ側で保管期間を管理する。
6. CI 上で実行する場合は、JSON に加えて Buildkite メタデータ `ci/xcframework-smoke:<lane>:device_tag` を出力し、ダッシュボードがデバイスレーンを解釈できるようにすること。

## 今後の作業

- エクスポータを Buildkite と連携させ、ライブフィードを公開する。
- 時系列データが揃い次第、ダッシュボードへ履歴スパークラインを追加する。
- Merkle／CRC64 の実行結果に対する Metal vs CPU のパリティチェックを自動化する。
