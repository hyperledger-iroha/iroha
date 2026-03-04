---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c42ab96daf6fcc2344f4b2361c5c91a371a87cf16dd8408448b7c6b69bd45ec
source_last_modified: "2025-11-20T11:52:13.452903+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: privacy-metrics-pipeline
title: SoraNet プライバシーメトリクス・パイプライン (SNNet-8)
sidebar_label: プライバシーメトリクス・パイプライン
description: SoraNet の relay と orchestrator 向けのプライバシー保護テレメトリ収集。
---

:::note 公式ソース
このページは `docs/source/soranet/privacy_metrics_pipeline.md` を反映します。レガシー docs が廃止されるまで両方のコピーを同期してください。
:::

# SoraNet プライバシーメトリクス・パイプライン

SNNet-8 は relay runtime 向けにプライバシーを意識したテレメトリ面を導入します。relay は handshake と circuit のイベントを 1 分単位の bucket に集約し、粗い Prometheus カウンタのみをエクスポートすることで、個々の circuit をリンク不能に保ちつつ、運用者に実用的な可視性を提供します。

## アグリゲータ概要

- runtime 実装は `tools/soranet-relay/src/privacy.rs` の `PrivacyAggregator` にあります。
- bucket は実時間の分単位でキー付けされ (`bucket_secs`、既定 60 秒)、有界リング (`max_completed_buckets`、既定 120) に保存されます。collector の share はそれぞれ有界のバックログ (`max_share_lag_buckets`、既定 12) を持ち、古くなった Prio のウィンドウは抑制 bucket としてフラッシュされ、メモリリークや詰まった collector の隠蔽を防ぎます。
- `RelayConfig::privacy` は `PrivacyConfig` にそのまま対応し、調整ノブ (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`) を公開します。本番 runtime は既定値を維持し、SNNet-8a で安全な集約のしきい値を導入します。
- runtime モジュールは型付きヘルパー `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, `record_gar_category` を通じてイベントを記録します。

## Relay 管理エンドポイント

運用者は `GET /privacy/events` を介して relay の管理リスナーから生の観測値を取得できます。このエンドポイントは改行区切り JSON (`application/x-ndjson`) を返し、内部の `PrivacyEventBuffer` からミラーされた `SoranetPrivacyEventV1` ペイロードが含まれます。バッファは最新のイベントを `privacy.event_buffer_capacity` 件 (既定 4096) まで保持し、読み取り時にドレインされるため、スクレイパーは欠落が出ないよう十分な頻度でポーリングしてください。イベントは Prometheus カウンタを支える handshake、throttle、verified bandwidth、active circuit、GAR の信号を同じくカバーし、下流の collector がプライバシー安全な証跡を保存したり、安全な集約ワークフローに供給できるようにします。

## Relay 設定

運用者は relay の設定ファイルの `privacy` セクションでプライバシー テレメトリの cadence を調整します:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

フィールドの既定値は SNNet-8 仕様に一致し、読み込み時に検証されます:

| フィールド | 説明 | 既定値 |
|-----------|------|--------|
| `bucket_secs` | 各集約ウィンドウの幅 (秒)。 | `60` |
| `min_handshakes` | bucket がカウンタを出力できる最小貢献者数。 | `12` |
| `flush_delay_buckets` | フラッシュを試みる前に待つ完了済み bucket 数。 | `1` |
| `force_flush_buckets` | 抑制 bucket を出すまでの最大経過時間。 | `6` |
| `max_completed_buckets` | 保持される bucket バックログ (無限メモリを防止)。 | `120` |
| `max_share_lag_buckets` | 抑制前に保持する collector share のウィンドウ。 | `12` |
| `expected_shares` | 結合前に必要な Prio collector share 数。 | `2` |
| `event_buffer_capacity` | 管理ストリーム向け NDJSON イベントのバックログ。 | `4096` |

`force_flush_buckets` を `flush_delay_buckets` より低く設定したり、しきい値をゼロにしたり、保持ガードを無効にすると、relay ごとのテレメトリ漏えいを防ぐために検証で失敗します。

`event_buffer_capacity` の上限は `/admin/privacy/events` も制限し、スクレイパーが無制限に遅延しないようにします。

## Prio collector share

SNNet-8a は秘密分散された Prio bucket を出力する二重 collector を展開します。orchestrator は `/privacy/events` NDJSON ストリームから `SoranetPrivacyEventV1` と `SoranetPrivacyPrioShareV1` を解析し、`SoranetSecureAggregator::ingest_prio_share` に渡します。bucket は `PrivacyBucketConfig::expected_shares` 件の貢献が揃うと出力され、relay の挙動を反映します。share は bucket の整合性とヒストグラム形状が検証され、`SoranetPrivacyBucketMetricsV1` に結合されます。結合後の handshake 数が `min_contributors` を下回る場合、bucket は `suppressed` としてエクスポートされ、relay 内アグリゲータの挙動と一致します。抑制ウィンドウは `suppression_reason` ラベルを出力し、運用者が `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, `forced_flush_window_elapsed` を区別してテレメトリの欠落を診断できます。`collector_window_elapsed` は Prio share が `max_share_lag_buckets` を超えて滞留した場合にも発火し、古いアキュムレータをメモリに残さずに詰まった collector を可視化します。

## Torii 取り込みエンドポイント

Torii は relay と collector が専用トランスポートを埋め込まずに観測を転送できるよう、テレメトリ制御の HTTP エンドポイントを 2 つ公開します:

- `POST /v1/soranet/privacy/event` は `RecordSoranetPrivacyEventDto` を受け付けます。ボディは `SoranetPrivacyEventV1` と任意の `source` ラベルを包みます。Torii は有効なテレメトリプロファイルに対して検証し、イベントを記録して HTTP `202 Accepted` とともに、算出したウィンドウ (`bucket_start_unix`, `bucket_duration_secs`) と relay モードを含む Norito JSON エンベロープを返します。
- `POST /v1/soranet/privacy/share` は `RecordSoranetPrivacyShareDto` を受け付けます。ボディは `SoranetPrivacyPrioShareV1` と任意の `forwarded_by` ヒントを運び、運用者が collector のフローを監査できるようにします。成功時は HTTP `202 Accepted` と Norito JSON エンベロープで collector、bucket ウィンドウ、抑制ヒントを要約します。検証失敗はテレメトリの `Conversion` 応答に対応し、collector 間の決定的なエラーハンドリングを維持します。orchestrator のイベントループは relay をポーリングする際にこれらの share を出力し、Torii の Prio アキュムレータを relay 側の bucket と同期させます。

両エンドポイントはテレメトリプロファイルに従い、メトリクスが無効な場合は `503 Service Unavailable` を返します。クライアントは Norito バイナリ (`application/x.norito`) または Norito JSON (`application/x.norito+json`) を送信でき、サーバは標準の Torii extractor で形式を自動交渉します。

## Prometheus メトリクス

エクスポートされる各 bucket には `mode` (`entry`, `middle`, `exit`) と `bucket_start` ラベルが付きます。次のメトリクス ファミリが出力されます:

| Metric | 説明 |
|--------|------|
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` の handshake 分類。 |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` の throttle カウンタ。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | throttle された handshake による cooldown 継続時間の集約値。 |
| `soranet_privacy_verified_bytes_total` | 盲測定証明からの検証済み帯域。 |
| `soranet_privacy_active_circuits_{avg,max}` | bucket ごとのアクティブ circuit の平均と最大値。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT パーセンタイル推定 (`p50`, `p90`, `p99`)。 |
| `soranet_privacy_gar_reports_total{category_hash}` | カテゴリ digest でキー付けされた Governance Action Report カウンタ。 |
| `soranet_privacy_bucket_suppressed` | 貢献者しきい値未達のため保留された bucket。 |
| `soranet_privacy_pending_collectors{mode}` | relay モード別にまとめた、結合待ちの collector share アキュムレータ。 |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` の抑制 bucket カウンタ。ダッシュボードがプライバシー欠落を分類できます。 |
| `soranet_privacy_snapshot_suppression_ratio` | 直近ドレインの抑制/ドレイン比 (0-1)。アラート予算に有用。 |
| `soranet_privacy_last_poll_unixtime` | 直近の成功ポーリングの UNIX タイムスタンプ (collector-idle アラートの元)。 |
| `soranet_privacy_collector_enabled` | プライバシー collector が無効または起動失敗時に `0` になるゲージ (collector-disabled アラートの元)。 |
| `soranet_privacy_poll_errors_total{provider}` | relay 別名ごとのポーリング失敗 (デコードエラー、HTTP 失敗、予期しないステータスコードで増加)。 |

観測のない bucket は沈黙し、ゼロ埋めウィンドウを捏造せずにダッシュボードを整えます。

## 運用ガイダンス

1. **ダッシュボード** - 上記メトリクスを `mode` と `window_start` でグループ化して可視化します。欠落ウィンドウを強調し、collector や relay の問題を露出させます。`soranet_privacy_suppression_total{reason}` を使って、貢献者不足と collector 起因の抑制を切り分けます。Grafana アセットには、これらのカウンタで駆動する **"Suppression Reasons (5m)"** パネルと、`sum(soranet_privacy_bucket_suppressed) / count(...)` を選択範囲ごとに計算する **"Suppressed Bucket %"** スタットが追加され、予算超過を一目で確認できます。**"Collector Share Backlog"** シリーズ (`soranet_privacy_pending_collectors`) と **"Snapshot Suppression Ratio"** スタットは、詰まった collector と自動運転中の予算ドリフトを示します。
2. **アラート** - プライバシー安全なカウンタからアラームを駆動します。PoW 拒否のスパイク、cooldown 頻度、RTT のドリフト、容量拒否が対象です。各 bucket 内でカウンタは単調増加なので、シンプルなレートベースのルールがよく効きます。
3. **インシデント対応** - まずは集約データに頼ります。より深いデバッグが必要な場合は、生のトラフィックログを収集する代わりに、relay に bucket スナップショットの再生や盲測定証明の検査を依頼してください。
4. **保持** - `max_completed_buckets` を超えないよう十分頻繁にスクレイプします。エクスポータは Prometheus の出力を正規ソースとみなし、転送後はローカル bucket を破棄してください。

## 抑制分析と自動実行

SNNet-8 の受け入れは、自動 collector が健全で抑制がポリシー上限 (任意の 30 分ウィンドウで relay あたり 10% 以下) に収まることを示すことに依存します。このゲートを満たすために必要なツールはリポジトリに同梱されており、運用者は週次の儀式に組み込む必要があります。新しい Grafana の抑制パネルは以下の PromQL スニペットを反映し、オンコールが手動クエリに頼る前にライブで可視化できるようにします。

### 抑制レビュー用 PromQL レシピ

運用者は次の PromQL ヘルパーを手元に置いてください。どちらも共有 Grafana ダッシュボード (`dashboards/grafana/soranet_privacy_metrics.json`) と Alertmanager ルールで参照されています:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

この比率の出力で **"Suppressed Bucket %"** スタットがポリシー予算以下に留まっていることを確認し、スパイク検出を Alertmanager に接続して、貢献者数が想定外に落ちた際に素早く気付けるようにしてください。

### オフライン bucket レポート CLI

ワークスペースは単発の NDJSON キャプチャ向けに `cargo xtask soranet-privacy-report` を提供します。1 つ以上の relay 管理エクスポートを指定してください:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

このヘルパーはキャプチャを `SoranetSecureAggregator` に通し、抑制サマリを stdout に出力し、`--json-out <path|->` で構造化 JSON レポートも任意で書き出します。ライブ collector と同じノブ (`--bucket-secs`, `--min-contributors`, `--expected-shares` など) を尊重するため、運用者は問題の切り分け時にしきい値を変えて過去のキャプチャを再生できます。SNNet-8 抑制分析ゲートが監査可能であるよう、JSON を Grafana のスクリーンショットと一緒に添付してください。

### 初回自動実行チェックリスト

ガバナンスは、初回の自動実行が抑制予算を満たしたことの証明を引き続き求めます。ヘルパーは `--max-suppression-ratio <0-1>` を受け付けるようになり、抑制 bucket が許容量 (既定 10%) を超えた場合や、まだ bucket が存在しない場合に CI も運用者も即座に失敗できます。推奨フロー:

1. relay 管理エンドポイントと orchestrator の `/v1/soranet/privacy/event|share` ストリームから NDJSON をエクスポートし、`artifacts/sorafs_privacy/<relay>.ndjson` に保存します。
2. ポリシー予算でヘルパーを実行します:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   コマンドは観測された比率を表示し、予算超過 **または** bucket が未準備の場合に非ゼロで終了し、実行のテレメトリがまだ生成されていないことを示します。ライブメトリクスは `soranet_privacy_pending_collectors` がゼロに向かってドレインし、`soranet_privacy_snapshot_suppression_ratio` が同じ予算以下に留まることを示すはずです。
3. トランスポートのデフォルトを切り替える前に、JSON 出力と CLI ログを SNNet-8 証跡バンドルに保管し、レビュワーが正確な成果物を再生できるようにします。

## 次のステップ (SNNet-8a)

- 二重 Prio collector を統合し、その share 取り込みを runtime に配線して、relay と collector が一貫した `SoranetPrivacyBucketMetricsV1` ペイロードを出力できるようにする。*(完了 — `crates/sorafs_orchestrator/src/lib.rs` の `ingest_privacy_payload` と付随テストを参照。)*
- 抑制ギャップ、collector 健全性、匿名性の劣化をカバーする共有 Prometheus ダッシュボード JSON とアラートルールを公開する。*(完了 — `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` と検証フィクスチャを参照。)*
- `privacy_metrics_dp.md` で説明される差分プライバシーのキャリブレーション成果物を作成し、再現可能なノートブックとガバナンスダイジェストを含める。*(完了 — ノートブックと成果物は `scripts/telemetry/run_privacy_dp.py` により生成。CI ラッパー `scripts/telemetry/run_privacy_dp_notebook.sh` が `.github/workflows/release-pipeline.yml` でノートブックを実行。ガバナンスダイジェストは `docs/source/status/soranet_privacy_dp_digest.md` に保存。)*

現在のリリースは SNNet-8 の基盤を提供します。決定的でプライバシー安全なテレメトリが既存の Prometheus スクレイパとダッシュボードに直接組み込まれ、差分プライバシーのキャリブレーション成果物が整備され、リリースパイプラインのワークフローがノートブック出力を最新に保ちます。残る作業は、初回自動実行の監視と抑制アラート分析の拡張です。
