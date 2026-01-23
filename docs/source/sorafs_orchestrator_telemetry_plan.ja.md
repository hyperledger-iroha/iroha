---
lang: ja
direction: ltr
source: docs/source/sorafs_orchestrator_telemetry_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f697d58faa5c634516a58566671cae33538fff8de31422bc34e4e860c0e8dc13
source_last_modified: "2025-11-02T18:25:19.142204+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sorafs_orchestrator_telemetry_plan.md -->

# SoraFS オーケストレーターのテレメトリ／アラート計画

## メトリクス

オーケストレーターは `iroha_telemetry::metrics::Metrics` を通じて Prometheus メトリクスを
出力し、`iroha_telemetry/otel-exporter` 機能が有効な場合は同じシグナルを
`sorafs.fetch.*` プレフィックスの OpenTelemetry 計測としても出力する。

**Prometheus / スクレイプパス**

- `sorafs_orchestrator_active_fetches{manifest_id,region}` — アクティブなフェッチセッション数。
- `sorafs_orchestrator_fetch_duration_ms_bucket{manifest_id,region,le}` — フェッチ時間のヒストグラム。
- `sorafs_orchestrator_fetch_failures_total{manifest_id,region,failure_reason}` — オーケストレーター側の失敗件数。
- `sorafs_orchestrator_retries_total{manifest_id,provider_id,retry_reason}` — プロバイダ別の再試行回数。
- `sorafs_orchestrator_provider_failures_total{manifest_id,provider_id,failure_reason}` — プロバイダ障害マトリクス。

**OpenTelemetry / OTLP プッシュ**

- `sorafs.fetch.active{manifest_id,region,job_id}` — アクティブなフェッチ（up/down カウンタ）。
- `sorafs.fetch.duration_ms{manifest_id,region,job_id}` — フェッチ時間ヒストグラム（ミリ秒）。
- `sorafs.fetch.failures_total{manifest_id,region,failure_reason}` — オーケストレーター全体の失敗。
- `sorafs.fetch.retries_total{manifest_id,region,job_id,provider_id,retry_reason}` — 再試行回数。
- `sorafs.fetch.provider_failures_total{manifest_id,region,job_id,provider_id,failure_reason}` — プロバイダ障害マトリクス。

## イベント／ログ

- プロバイダの BAN/解除（理由付き）。
- トークン枯渇イベント。
- 証明検証失敗の詳細。

## ダッシュボード

- **Overview**: スループット、成功率、アクティブフェッチ。
- **Provider Health**: 失敗件数、レイテンシ、停止率。
- **Retries**: 理由別のヒストグラムと累積件数。
- **Per-Manifest**: チャンク進捗、未消化トークン。

これらのメトリクスに接続された Grafana ダッシュボードのベースラインは
`docs/examples/sorafs_fetch_dashboard.json` を参照（上記パネルに対応）。

## アラート

- 成功率 < 99% が 5 分以上継続。
- プロバイダ失敗率 > 5% が 10 分ウィンドウで継続。
- チャンクレイテンシ P95 > 250 ms が 10 分以上継続。
- トークン枯渇イベントが 1 分あたりの閾値超過。
- 証明検証失敗の観測（重要度付き）。

アラートルールは `docs/examples/sorafs_fetch_alerts.yaml` に定義されており、Prometheus
Alertmanager や Mimir ruler での取り込みに適合する。

## 統合

- OpenTelemetry でオーケストレーターのメトリクスを取り込み、Prometheus/Mimir へ連携。
- ゲートウェイのテレメトリとダッシュボードを連携（共通ラベル `manifest`, `provider`）。
- `sorafs_observability_plan.md` の SLO 定義にアラートを揃える。

## 実装状況

- Prometheus のゲージ／カウンタは `crates/sorafs_orchestrator/src/lib.rs` の
  `FetchMetricsCtx` から出力され、スコアボード駆動のフェッチでも成功／失敗の
  テレメトリが必ず更新される。
- OpenTelemetry メトリクスは `iroha_telemetry::metrics::SorafsFetchOtel` が提供し、
  `install_sorafs_fetch_otlp_exporter` が OTLP プッシュのパイプラインを構成する
  （`crates/iroha_telemetry/src/metrics.rs` 参照）。
- CLI/SDK は `Orchestrator::fetch_*` 実行前に OTLP のエンドポイントとリージョン属性を
  指定してヘルパを呼び出すことで、Prometheus スクレイプと OTLP ストリーミングが
  同期したまま動作する。

## ロールアウト & チューニングガイド

- **設定。** フェッチ開始前に
  `install_sorafs_fetch_otlp_exporter("http://127.0.0.1:4317", "sorafs-orchestrator", &[("deployment.region", region)], Duration::from_secs(5))`
  を呼び出し、OTLP の送信間隔をローカルコレクタに合わせる。OTLP ストリームと
  Prometheus テキストエンドポイントが同じ計測値を出力する状態を保つ。
- **サンプリング間隔。** OTLP ヘルパは 2 秒ごとに送信する。コレクタのバッチ間隔は
  5 秒に保ち、`rate()`/`histogram_quantile()` を使う Grafana パネルが過剰サンプリング
  にならず、準リアルタイムの挙動を反映できるようにする。
- **障害シナリオ。**
  - `sorafs.fetch.failures_total` の増加は機能不一致や再試行枯渇が原因であることが多い。
    `failure_reason` ラベルを確認し、オーケストレーター側のゲートを調整する。
  - `sorafs.fetch.provider_failures_total` のスパイクは不調なプロバイダを示す。`Retries per Provider`
    パネルと相関させて一時的なブラックリストを判断する。
  - `sorafs.fetch.duration_ms` の p95 が 250 ms を超えて継続するとバンドル済みアラートが発火する。
    ページング前に `FetchOptions::global_parallel_limit` と再試行予算を調整する。
- **検証。** `docs/examples/sorafs_fetch_dashboard.json` の Grafana パネルからロールアウトを開始し、
  ベースラインのレイテンシと失敗率が安定したら `docs/examples/sorafs_fetch_alerts.yaml` の
  アラートセットを有効化する。

## ラベルタクソノミ

オーケストレーターのメトリクスは、ゲートウェイ／ノード計画と同じラベル規則と
セマンティクスを採用し、サービス横断の結合が決定論的に行えるようにする。

| ラベル | 適用先 | 説明 |
|--------|--------|------|
| `manifest_id` | ゲージ／ヒストグラム／カウンタ | フェッチ対象マニフェストの安定 Norito CID。グローバル指標（例: 総再試行回数）では省略。 |
| `provider_id` | プロバイダ指標 | ガバナンス発行のプロバイダ識別子（`prov_xxx`）。マルチホップでは最終プロバイダを指す。 |
| `job_id` | オーケストレーションジョブ | 単一フェッチ要求と再試行を束ねる UUID。トレース相関のためレイテンシ／再試行に含める。 |
| `region` | 全メトリクス | オーケストレーターのデプロイリージョン（`us-east-1`, `eu-central-1`）。リージョナル表示とアラート振り分けを可能にする。 |
| `failure_reason` | 失敗カウンタ | 列挙理由（`timeout`, `digest_mismatch`, `http_5xx`, `token_exhausted`）。 |
| `retry_reason` | 再試行メトリクス | 再試行分類（`retry`, `session_failure`, `length_mismatch` など）。 |

ラベルの健全性ルール:
- `manifest_id` と `provider_id` の組を結合キーにする。どちらかが欠ける場合、下流ダッシュボードは
  集約指標として扱う。
- カーディナリティは、`job_id` をエグザンプラ専用にして抑制する（Prometheus のヒストグラムでは
  通常ラベルではなくエグザンプラ・タグとして出力）。
- OpenTelemetry スパンを出す場合は、これらのラベルをトレース属性（`sora.manifest_id`,
  `sora.provider_id`）としても複製し、トレーシングとメトリクスで語彙を一致させる。

## メトリクス配信アーキテクチャ

- **エクスポート機構。** `iroha_telemetry::metrics::install_sorafs_fetch_otlp_exporter` を呼び出して
  OTLP パイプラインを初期化する。デプロイではローカルの OpenTelemetry Collector サイドカーに
  接続し、スクレイプだけに頼らず 2 秒間隔でストリーミングする。
- **コレクタパイプライン。**
  1. オーケストレーターがサイドカーへ OTLP メトリクスを送信（Tokio ランタイム + OTLP gRPC exporter）。
  2. コレクタは 5 秒ウィンドウでバッチし、リソース属性（`service.name=sorafs-orchestrator`,
     `deployment.region`）を付与して中央の Prometheus remote-write ゲートウェイへ転送。
  3. Remote-write ゲートウェイが Mimir/Prometheus に書き込み。ストリーミングにより
     チャンクレイテンシのヒストグラムにジョブ単位のエグザンプラを付与できる。
- **フェイルセーフのポーリング。** Prometheus レジストリは既存の `/metrics` で引き続き利用可能。
  OTLP プッシュが 30 秒以上失敗した場合、コレクタはスクレイプにフォールバックして
  監視ダッシュボードを維持できる。
- **トレース相関。** トレースは同じサイドカー経由で流れる。各スパンに `manifest_id`, `provider_id`,
  `job_id` を含め、Grafana Tempo/Jaeger がメトリクスのデータポイントからトレースへ
  ピボットできるようにする。

## Grafana 連携

- **テンプレートライブラリ。** Observability チームと協調し、共有 Grafana ライブラリ
  （`grafana/provisioning/dashboards/sorafs.jsonnet`）にオーケストレーターのパネルを追加する。
  KPI 概要、プロバイダ詳細、再試行マトリクス、マニフェスト進捗のボードを含める。
- **パネルオーナー。** ダッシュボードの担当（`Storage On-Call`）を定義し、アラートが
  直ちに当番へ届くよう連絡先メタデータを追加する。
- **パネル検証。** ステージングでオーケストレーターのメトリクスとゲートウェイのダッシュボードを
  組み合わせ、ラベル結合（`manifest_id`, `provider_id`, `region`）が一致することを確認する。
  パネルが安定したら `sorafs_observability_plan.md` にスクリーンショット／リンクを追加する。
- **アラート統合。** Observability チームが Alertmanager ルートを設定し、オーケストレーターの
  アラートを Slack/PagerDuty へ振り分ける。各アラートルールは共有 SLO 定義を参照し、
  一貫した注釈（`summary`, `runbook_url`）で同じ対応ガイドに誘導する。
