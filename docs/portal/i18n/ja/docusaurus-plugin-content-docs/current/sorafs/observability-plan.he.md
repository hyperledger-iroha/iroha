---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/observability-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a7b1bf9505b55d6a599406bfce76a3511c7a4972af05215f0abf39335e37bb18
source_last_modified: "2026-01-21T07:37:45+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: observability-plan
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/sorafs_observability_plan.md` で管理されている計画を反映しています。旧 Sphinx セットが完全に移行されるまで、両方のコピーを同期してください。
:::

## 目的
- gateway、ノード、マルチソース・オーケストレーターのメトリクスと構造化イベントを定義する。
- Grafana ダッシュボード、アラート閾値、検証フックを提供する。
- エラーバジェットとカオスドリルの方針とあわせて SLO 目標を定める。

## メトリクス・カタログ

### Gateway サーフェス

| メトリクス | 種類 | ラベル | 備考 |
|-----------|------|--------|------|
| `sorafs_gateway_active` | ゲージ (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` から出力され、endpoint/method の組み合わせ別に進行中の HTTP 操作を追跡する。 |
| `sorafs_gateway_responses_total` | カウンタ | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | 完了した gateway リクエストごとに 1 回インクリメントされる。`result` ∈ {`success`,`error`,`dropped`}。 |
| `sorafs_gateway_ttfb_ms_bucket` | ヒストグラム | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | gateway 応答の time-to-first-byte レイテンシ。Prometheus の `_bucket/_sum/_count` としてエクスポートされる。 |
| `sorafs_gateway_proof_verifications_total` | カウンタ | `profile_version`, `result`, `error_code` | リクエスト時に収集される証明検証結果 (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | ヒストグラム | `profile_version`, `result`, `error_code` | PoR レシートの検証レイテンシ分布。 |
| `telemetry::sorafs.gateway.request` | 構造化イベント | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | リクエスト完了ごとに Loki/Tempo 相関のために発行される構造化ログ。 |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | カウンタ | 既存ラベルセット | 既存ダッシュボード用に保持される Prometheus メトリクス。新しい OTLP シリーズと併せて出力される。 |

`telemetry::sorafs.gateway.request` イベントは OTEL カウンタを構造化ペイロードで反映し、Loki/Tempo の相関に向けて `endpoint`, `method`, `variant`, `status`, `error_code`, `duration_ms` を露出する。一方でダッシュボードは SLO 追跡のため OTLP シリーズを消費する。

### Proof-health テレメトリ

| メトリクス | 種類 | ラベル | 備考 |
|-----------|------|--------|------|
| `torii_sorafs_proof_health_alerts_total` | カウンタ | `provider_id`, `trigger`, `penalty` | `RecordCapacityTelemetry` が `SorafsProofHealthAlert` を発行するたびに増加する。`trigger` は PDP/PoTR/Both の失敗を区別し、`penalty` は担保が実際にスラッシュされたかクールダウンで抑止されたかを示す。 |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | ゲージ | `provider_id` | 問題のテレメトリウィンドウ内で報告された最新の PDP/PoTR カウント。プロバイダーが方針をどれだけ超過したかを定量化できる。 |
| `torii_sorafs_proof_health_penalty_nano` | ゲージ | `provider_id` | 直近アラートでスラッシュされた Nano-XOR 量（クールダウンが抑止した場合はゼロ）。 |
| `torii_sorafs_proof_health_cooldown` | ゲージ | `provider_id` | ブール型ゲージ（`1` = アラートがクールダウンで抑止）で、追従アラートが一時的にミュートされていることを示す。 |
| `torii_sorafs_proof_health_window_end_epoch` | ゲージ | `provider_id` | アラートに紐づくテレメトリウィンドウのエポック。Norito アーティファクトと突合できるようにする。 |

これらのフィードは Taikai viewer ダッシュボードの proof-health 行を駆動し
(`dashboards/grafana/taikai_viewer.json`)、CDN オペレーターにアラート量、PDP/PoTR
トリガー比率、ペナルティ、プロバイダー別クールダウン状態をリアルタイムで
可視化する。

同じメトリクスで Taikai viewer の 2 つのアラートルールを支える。
`SorafsProofHealthPenalty` は
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` が
直近 15 分で増加したときに発火し、`SorafsProofHealthCooldown` は
プロバイダーが 5 分間クールダウン状態のままの場合に警告する。両アラートは
`dashboards/alerts/taikai_viewer_rules.yml` にあり、PoR/PoTR の執行が強まるたびに
SRE が即座に状況を把握できる。

### オーケストレーター・サーフェス

| メトリクス / イベント | 種類 | ラベル | 生成元 | 備考 |
|----------------------|------|--------|----------|------|
| `sorafs_orchestrator_active_fetches` | ゲージ | `manifest_id`, `region` | `FetchMetricsCtx` | 進行中のセッション数。 |
| `sorafs_orchestrator_fetch_duration_ms` | ヒストグラム | `manifest_id`, `region` | `FetchMetricsCtx` | ミリ秒単位の期間ヒストグラム。1 ms から 30 s のバケット。 |
| `sorafs_orchestrator_fetch_failures_total` | カウンタ | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | 理由: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | カウンタ | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | リトライ理由を区別 (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | カウンタ | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | セッション単位の無効化/失敗カウントを記録する。 |
| `sorafs_orchestrator_chunk_latency_ms` | ヒストグラム | `manifest_id`, `provider_id` | `FetchMetricsCtx` | chunk ごとの fetch レイテンシ分布（ms）。throughput/SLO 分析向け。 |
| `sorafs_orchestrator_bytes_total` | カウンタ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | manifest/provider ごとの配信バイト数。PromQL の `rate()` で throughput を導出。 |
| `sorafs_orchestrator_stalls_total` | カウンタ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` を超える chunk 数をカウント。 |
| `telemetry::sorafs.fetch.lifecycle` | 構造化イベント | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | ジョブのライフサイクル（start/complete）を Norito JSON ペイロードで表現。 |
| `telemetry::sorafs.fetch.retry` | 構造化イベント | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | プロバイダーのリトライ連続に対して発行。`attempts` はインクリメンタルな再試行回数（≥ 1）。 |
| `telemetry::sorafs.fetch.provider_failure` | 構造化イベント | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | プロバイダーが失敗閾値を超えたときに発行。 |
| `telemetry::sorafs.fetch.error` | 構造化イベント | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | 失敗終端レコード。Loki/Splunk 取り込みに適した形式。 |
| `telemetry::sorafs.fetch.stall` | 構造化イベント | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | chunk レイテンシが設定上限を超えたときに発行（stall カウンタのミラー）。 |

### ノード / レプリケーション・サーフェス

| メトリクス | 種類 | ラベル | 備考 |
|-----------|------|--------|------|
| `sorafs_node_capacity_utilisation_pct` | ヒストグラム | `provider_id` | ストレージ利用率（%）の OTEL ヒストグラム（`_bucket/_sum/_count` としてエクスポート）。 |
| `sorafs_node_por_success_total` | カウンタ | `provider_id` | スケジューラのスナップショットから導出した PoR 成功サンプルの単調カウンタ。 |
| `sorafs_node_por_failure_total` | カウンタ | `provider_id` | PoR 失敗サンプルの単調カウンタ。 |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | ゲージ | `provider` | 既存の Prometheus ゲージ（使用バイト数、キュー深度、PoR in-flight 数）。 |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | ゲージ | `provider` | 容量/uptime 成功データを容量ダッシュボードに反映。 |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | ゲージ | `provider`, `manifest` | `/v2/sorafs/por/ingestion/{manifest}` のポーリング時にバックログ深度と累積失敗カウンタをエクスポートし、"PoR Stalls" パネル/アラートに供給する。 |

### 修復 & SLA

| メトリクス | 種類 | ラベル | 備考 |
|-----------|------|--------|------|
| `sorafs_repair_tasks_total` | Counter | `status` | 修復タスクの状態遷移に関する OTEL カウンタ。 |
| `sorafs_repair_latency_minutes` | Histogram | `outcome` | 修復ライフサイクルのレイテンシに関する OTEL ヒストグラム。 |
| `sorafs_repair_queue_depth` | Histogram | `provider` | プロバイダー別のキュー待ちタスク数の OTEL ヒストグラム（スナップショット式）。 |
| `sorafs_repair_backlog_oldest_age_seconds` | Histogram | — | 最古のキュー待ちタスクの経過時間（秒）を示す OTEL ヒストグラム。 |
| `sorafs_repair_lease_expired_total` | Counter | `outcome` | リース期限切れ（`requeued`/`escalated`）の OTEL カウンタ。 |
| `sorafs_repair_slash_proposals_total` | Counter | `outcome` | スラッシュ提案の遷移に関する OTEL カウンタ。 |
| `torii_sorafs_repair_tasks_total` | Counter | `status` | タスク遷移の Prometheus カウンタ。 |
| `torii_sorafs_repair_latency_minutes_bucket` | Histogram | `outcome` | 修復ライフサイクルのレイテンシに関する Prometheus ヒストグラム。 |
| `torii_sorafs_repair_queue_depth` | Gauge | `provider` | プロバイダー別のキュー待ちタスク数の Prometheus ゲージ。 |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Gauge | — | 最古のキュー待ちタスクの経過時間（秒）の Prometheus ゲージ。 |
| `torii_sorafs_repair_lease_expired_total` | Counter | `outcome` | リース期限切れの Prometheus カウンタ。 |
| `torii_sorafs_slash_proposals_total` | Counter | `outcome` | スラッシュ提案遷移の Prometheus カウンタ。 |

ガバナンス監査 JSON メタデータは修復テレメトリのラベル（修復イベントでは `status`, `ticket_id`, `manifest`, `provider`、スラッシュ提案では `outcome`）を反映し、メトリクスと監査アーティファクトの決定的な相関を維持する。

### Proof of Timely Retrieval (PoTR) と chunk SLA

| メトリクス | 種類 | ラベル | 生成元 | 備考 |
|-----------|------|--------|----------|------|
| `sorafs_potr_deadline_ms` | ヒストグラム | `tier`, `provider` | PoTR coordinator | デッドライン余裕のミリ秒（正 = 達成）。 |
| `sorafs_potr_failures_total` | カウンタ | `tier`, `provider`, `reason` | PoTR coordinator | 理由: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | カウンタ | `provider`, `manifest_id`, `reason` | SLA monitor | chunk 配信が SLO を満たさない場合に発火（レイテンシ、成功率）。 |
| `sorafs_chunk_sla_violation_active` | ゲージ | `provider`, `manifest_id` | SLA monitor | アクティブな違反ウィンドウ中に切り替わるブールゲージ（0/1）。 |

## SLO 目標

- gateway の trustless availability: **99.9%**（HTTP 2xx/304 応答）。
- trustless TTFB P95: hot tier ≤ 120 ms, warm tier ≤ 300 ms.
- 証明の成功率: 1 日あたり ≥ 99.5%.
- オーケストレーター成功（chunk 完了）: ≥ 99%.

## ダッシュボードとアラート

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — trustless availability、TTFB P95、拒否内訳、PoR/PoTR 失敗を OTEL メトリクスで追跡。
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — multi-source 負荷、リトライ、プロバイダー障害、stall バーストをカバー。
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — 匿名化 relay バケット、抑制ウィンドウ、collector 健全性を `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`, `soranet_privacy_poll_errors_total{provider}` で可視化。

アラートバンドル:

- `dashboards/alerts/sorafs_gateway_rules.yml` — gateway 可用性、TTFB、証明失敗スパイク。
- `dashboards/alerts/sorafs_fetch_rules.yml` — オーケストレーターの失敗/リトライ/stall。`scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, `dashboards/alerts/tests/soranet_policy_rules.test.yml` で検証。
- `dashboards/alerts/soranet_privacy_rules.yml` — プライバシー劣化スパイク、抑制アラーム、collector idle 検出、collector 無効化アラート (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — `sorafs_orchestrator_brownouts_total` に接続された匿名性 brownout アラーム。
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai viewer の drift/ingest/CEK lag アラームと、`torii_sorafs_proof_health_*` に基づく SoraFS proof-health の penalty/cooldown アラート。

## トレーシング戦略

- OpenTelemetry をエンドツーエンドで採用:
  - gateway はリクエスト ID、manifest digest、token hash を付与した OTLP spans（HTTP）を出力する。
  - オーケストレーターは `tracing` + `opentelemetry` を使って fetch 試行の spans をエクスポートする。
  - 組み込み SoraFS ノードは PoR チャレンジとストレージ操作の spans をエクスポートする。全コンポーネントで共通の trace ID を `x-sorafs-trace` 経由で伝播する。
- `SorafsFetchOtel` はオーケストレーターのメトリクスを OTLP ヒストグラムに橋渡しし、`telemetry::sorafs.fetch.*` イベントはログ中心のバックエンド向けに軽量 JSON ペイロードを提供する。
- Collectors: Prometheus/Loki/Tempo（Tempo 推奨）と並行して OTEL collectors を実行する。Jaeger 互換エクスポータは任意。
- 高カーディナリティの操作はサンプリングする（成功パス 10%、失敗 100%）。

## TLS テレメトリ調整 (SF-5b)

- メトリクスの整合:
  - TLS 自動化は `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, `sorafs_gateway_tls_ech_enabled` を出力する。
  - これらのゲージを Gateway Overview ダッシュボードの TLS/Certificates パネルに追加する。
- アラート連携:
  - TLS 期限アラート（残り ≤ 14 日）が発火したら trustless availability SLO と相関させる。
  - ECH 無効化は TLS と availability 両方のパネルを参照する二次アラートを発行する。
- Pipeline: TLS 自動化ジョブは gateway メトリクスと同じ Prometheus スタックへ出力し、SF-5b 連携で重複計装を排除する。

## メトリクス命名とラベル規約

- メトリクス名は Torii と gateway で使われる既存の `torii_sorafs_*` または `sorafs_*` プレフィックスに従う。
- ラベルセットを標準化:
  - `result` → HTTP outcome (`success`, `refused`, `failed`).
  - `reason` → 拒否/エラーコード (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → 16 進エンコードの provider 識別子。
  - `manifest` → 正規 manifest digest（高カーディナリティ時はトリム）。
  - `tier` → 宣言的な tier ラベル (`hot`, `warm`, `archive`).
- テレメトリ出力ポイント:
  - gateway メトリクスは `torii_sorafs_*` 配下にあり、`crates/iroha_core/src/telemetry.rs` の規約を再利用する。
  - オーケストレーターは `sorafs_orchestrator_*` メトリクスと `telemetry::sorafs.fetch.*` イベント（lifecycle, retry, provider failure, error, stall）を発行し、manifest digest、job ID、region、provider 識別子を付与する。
  - ノードは `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, `torii_sorafs_por_*` を出力する。
- Observability と連携して Prometheus 命名ドキュメントにメトリクス・カタログを登録し、ラベル・カーディナリティの期待値（provider/manifests 上限）を含める。

## データパイプライン

- Collectors は各コンポーネントと同居し、OTLP を Prometheus（メトリクス）と Loki/Tempo（ログ/トレース）へエクスポートする。
- オプションの eBPF（Tetragon）が gateway/ノードの低レベルトレースを拡張する。
- Torii と組み込みノードには `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` を使用し、オーケストレーターは `install_sorafs_fetch_otlp_exporter` を呼び続ける。

## 検証フック

- CI で `scripts/telemetry/test_sorafs_fetch_alerts.sh` を実行し、Prometheus アラートルールが stall メトリクスとプライバシー抑制チェックに同期していることを確認する。
- Grafana ダッシュボードはバージョン管理（`dashboards/grafana/`）に置き、パネル変更時にスクリーンショット/リンクを更新する。
- カオスドリルは `scripts/telemetry/log_sorafs_drill.sh` で結果を記録し、`scripts/telemetry/validate_drill_log.sh` で検証する（[運用プレイブック](operations-playbook.md) 参照）。
