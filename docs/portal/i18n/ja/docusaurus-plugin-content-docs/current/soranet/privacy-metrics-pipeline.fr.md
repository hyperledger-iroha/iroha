---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プライバシー-メトリクス-パイプライン
タイトル: 機密性の高いパイプライン SoraNet (SNNet-8)
Sidebar_label: 機密性の高いパイプライン
説明: SoraNet のリレーとオーケストレーターの機密情報を収集します。
---

:::note ソースカノニク
リフレット`docs/source/soranet/privacy_metrics_pipeline.md`。 Gardez les deux は、ドキュメントの保存と保存の同期をコピーします。
:::

# SoraNet の機密性に関するパイプライン

SNNet-8 は、リレーの実行時における機密性の高い表面的な機能を提供します。 Prometheus の粗悪なハンドシェイクと回路の監視と、監視された回路の個別の監視は、悪用可能であると相関関係がありません。

## アグレガトゥールの管理

- `tools/soranet-relay/src/privacy.rs` および `PrivacyAggregator` によるランタイムの実装。
- 時計のインデックスを取得するためのバケット (`bucket_secs`、デフォルト 60 秒) とアノー ボーンのストック (`max_completed_buckets`、デフォルト 120)。収集品の株式は、未処理の保守的なルールを保持し、収集品の保管庫 (`max_share_lag_buckets`、標準規格 12) で、優先順位の高いビデオとバケットの詳細を確認し、収集品のブロックを隠します。
- `RelayConfig::privacy` `PrivacyConfig` に関する説明、説明者 (`bucket_secs`、`min_handshakes`、`flush_delay_buckets`、`force_flush_buckets`、`max_completed_buckets`、 `max_share_lag_buckets`、`expected_shares`)。本番環境のランタイムは、SNNet-8a の安全性を保証するための初期設定の価値を維持します。
- ヘルパー タイプ経由でモジュール ランタイム登録ファイル: `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、`record_throttle_cooldown`、`record_capacity_reject`、`record_active_sample`、 `record_verified_bytes` および `record_gar_category`。

## 中継時のエンドポイント管理

`GET /privacy/events` を介して、操作者は尋問者を監視し、リスナー管理者はリレーを監視し、野蛮な観察を行います。 L'endpoint renvoie du JSON délimité par des nouvelles lignes (`application/x-ndjson`) contenant des payloads `SoranetPrivacyEventV1` reflétés depuis le `PrivacyEventBuffer` interne。バッファーは、最新のイベント `privacy.event_buffer_capacity` エントリー (デフォルト 4096) と講義中のビデオを保存し、講義中のビデオを保存します。ハンドシェイク、スロットル、検証済み帯域幅、アクティブ回路および GAR qui alimentent les compteurs Prometheus の記録、ワークフローの機密情報を収集するパンくずリストの永続的な補助収集装置d'agrégation sécurisée。

## リレーの構成

セクション `privacy` によるリレー設定の機密性管理の操作手順:

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

SNNet-8 の仕様に関する規定の特派員と料金の有効性:

|チャンピオン |説明 |デフォー |
|----------|---------------|----------|
| `bucket_secs` | Largeur de Chaque fenêtre d'agrégation (秒)。 | `60` |
| `min_handshakes` |コンペティションの管理に必要な最低限の貢献はありません。 | `12` |
| `flush_delay_buckets` | Nombre de Bucks は、フラッシュのない前衛的なエッセイに参加するための準備を完了します。 | `1` |
| `force_flush_buckets` |バケットの至高性を最大限に高めます。 | `6` |
| `max_completed_buckets` | Backlog debucks conservés (évite une memoire non Bornée)。 | `120` |
| `max_share_lag_buckets` |株式収集家の前衛抑制を保持するフェネートル。 | `12` |
| `expected_shares` |株式プリオ・デ・コレクターには前衛的な組み合わせが必要です。 | `2` |
| `event_buffer_capacity` | NDJSON がフラックス管理者にバックログを提供します。 | `4096` |

`force_flush_buckets` と基本的な `flush_delay_buckets` を定義し、リレー パー リレーでの安全性と保持期間の検証を監視します。

制限 `event_buffer_capacity` はオーストラリアの `/admin/privacy/events` に影響を及ぼし、不確実性を保証します。

## シェア・ド・コレクターズ・プリオSNNet-8a は、収集品の配備を二重化し、機密情報を収集します。 L'orchestrator は、流れ NDJSON `/privacy/events` を分析し、メイン `SoranetPrivacyEventV1` と共有 `SoranetPrivacyPrioShareV1`、`SoranetSecureAggregator::ingest_prio_share` のトランスメタントを分析します。 Lesbucks émettent une fois que `PrivacyBucketConfig::expected_shares` の貢献は到着し、リレーのコンポートメントを反映します。これは、`SoranetPrivacyBucketMetricsV1` のバケットの配列と前衛的な組み合わせのヒストグラムの形式を共有するものです。 `min_contributors` のハンドシェイクの組み合わせを指定し、`suppressed` を使用して輸出用のバケツを作成し、リレーのコンポートを反射します。 Les fenêtres supprimées émettent désormais un label `suppression_reason` afin que les opérateurs puissent distinguer `insufficient_contributors`、`collector_suppressed`、`collector_window_elapsed` et `forced_flush_window_elapsed`テレメトリの診断テスト。 La raison `collector_window_elapsed` は、`max_share_lag_buckets` で米国の株式を取得し、記憶を蓄積することなく収集されたブロックを可視化します。

## エンドポイントの取り込み Torii

Torii は、中継器や収集装置などの監視対象の HTTP プロトコルの詳細なエンドポイントを公開し、輸送時の監視を行わずに監視します。

- `POST /v1/soranet/privacy/event` ペイロード `RecordSoranetPrivacyEventDto` を受け入れます。 Le corps エンベロープ un `SoranetPrivacyEventV1` と un label `source` オプション。 Torii 有効なプロファイル プロファイルの要求、有効なアクセスの登録、HTTP `202 Accepted` エンベロープの付属 Norito JSON コンテンツ ラ フェネットリレーのモードの計算 (`bucket_start_unix`、`bucket_duration_secs`)。
- `POST /v1/soranet/privacy/share` はペイロード `RecordSoranetPrivacyShareDto` を受け入れます。 Le corps Transporte un `SoranetPrivacyPrioShareV1` et un indice `forwarded_by` optionnel afin que les opérateurs puissent Auditer les flux de Collecteurs. Les soumissions réussies renvoient HTTP `202 Accepted` avec une enveloppe Norito JSON Resumant le Collecteur、la fenêtre de Bucket et l'indication de抑制。 `Conversion` は、収集者に対する証拠の検証を担当します。 La boucle d'événements de l'orchestrator émet désormais ces は、lorsqu'elle interroge les リレー、gardant l'accumulateur Prio de Torii synchronisé avec lesbucks côté リレーを共有します。

プロファイル デ テレメトリに関するエンドポイントの詳細: `503 Service Unavailable` のメトリックに関するテスト。顧客の特​​使 Norito binaires (`application/x.norito`) または Norito JSON (`application/x.norito+json`) ; les extracteurs Torii 標準によるサーバー自動自動ファイル形式。

## メトリクス Prometheus

チャック バケット エクスポート ポート レ ラベル `mode` (`entry`、`middle`、`exit`) および `bucket_start`。将来の家族の特徴 :

|メトリック |説明 |
|----------|---------------|
| `soranet_privacy_circuit_events_total{kind}` |ハンドシェイクの分類法は `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` です。 |
| `soranet_privacy_throttles_total{scope}` |スロットルの平均値 `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` をコンピューティングします。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |クールダウン期間は、ハンドシェイク スロットルの承認に必要な期間を設定します。 |
| `soranet_privacy_verified_bytes_total` | Bande passante vérifiée issue de preuves de mesure aveugles。 |
| `soranet_privacy_active_circuits_{avg,max}` |モエンヌとバケットごとのサーキット活動。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT パーセンタイルの推定 (`p50`、`p90`、`p99`)。 |
| `soranet_privacy_gar_reports_total{category_hash}` | Compteurs de Governance Action Report は、カテゴリのダイジェストをまとめたインデックスです。 |
| `soranet_privacy_bucket_suppressed` |バケットは、貢献者にとって重要な役割を果たします。 |
| `soranet_privacy_pending_collectors{mode}` |組み合わせて蓄積し、リレーモードでグループを収集します。 |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` は、ダッシュボードの属性を機密情報として管理します。 |
| `soranet_privacy_snapshot_suppression_ratio` |最高/ビデオ ドレインの比率 (0-1)、予算に応じたユーティリティを提供します。 |
| `soranet_privacy_last_poll_unixtime` | Horodatage UNIX du dernier vote réussi (alimente l'alerte コレクターアイドル)。 |
| `soranet_privacy_collector_enabled` | Gauge qui passe à `0` lorsque le Collecteur de confidentialité est désactivé ou ne démarre pas (コレクターが無効になっている場合)。 |
| `soranet_privacy_poll_errors_total{provider}` |リレーのエイリアスによるポーリング グループのセキュリティ (エラーのエラーの増加、ステータスの監視のための HTTP のコードのセキュリティ)。 |観察のないバケツは沈黙を保ち、ダッシュボードの保守的な設備はファブリケのないものであり、ゼロに応答します。

## ガイダンス操作

1. **ダッシュボード** - `mode` および `window_start` によるトレーサー レポート メトリクス ci-dessus グループ。収集者とリレーの問題に関する公正な報告を監視します。使用者 `soranet_privacy_suppression_total{reason}` は、収集器官の安全性の低下を判断し、抑圧のパイロットを監視します。 L'asset Grafana inclut désormais un panneau dédié **「抑制理由 (5m)」** alimenté par ces compteurs, plus un stat **「Suppressed Bucket %」** qui calcule `sum(soranet_privacy_bucket_suppressed) / count(...)` par selection afin que les opérateurs予算とクーデターの期限を過ぎます。重要な **「コレクター共有バックログ」** (`soranet_privacy_pending_collectors`) および統計 **「スナップショット抑制率」** は、コレクター ブロケおよび予算ペンダントの実行自動化を監視します。
2. **アラート** - パイロットはコンピューティングの一部の警告を表示し、機密情報を提供します: PoW の拒否の写真、クールダウンの頻度、RTT と容量の拒否の画像。モノトーンのコンプチュールは、バケツのインテリア、シンプルなデザインの機能を備えています。
3. **インシデント対応** - s'appuyer d'abord sur les donnees agrégées。 Lorsqu'un débogage と深い必要性、要求者は、バケットのスナップショットとスナップショットを監視し、トラフィックの検査を監視します。
4. **保持** - スクレーパー suffisamment souvent pour éviter de dépasser `max_completed_buckets`。輸出業者は、出撃する Prometheus のソース、カノニクとサプライヤーのバケツをロコーで送信します。

## 抑制と自動実行の分析

SNNet-8 の承認は、収集装置の収集および停止を自動的に停止し、政治の制限を制限します (30 分間のリレーでバケットの 10 % 以下)。 Les outils necessaires pour satisfaire cette exigence Sont maintenant livrés avec le dépôt; les opérateurs doivent les intégrer à leurs rituels hebdomadaires。抑制の新しい機能 Grafana reflètent les extraits PromQL ci-dessous、donnant aux équipes d'astreinte une visibilité en direct avant de recouir à des requêtes manuelles。

### PromQL によるレビューの抑制を再確認する

メインのポートに対する PromQL 訴訟のヘルパーの操作。ダッシュボード Grafana 部分 (`dashboards/grafana/soranet_privacy_metrics.json`) および警告マネージャーの参照:

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

統計 **「抑制されたバケット %」** を使用して、政治的な予算を確認します。アラートマネージャーは、写真を検出し、迅速な監視を提供し、マニエールの監視を強化します。

### CLI デ・ラポール・デ・バケット・オー・リーニュ

L'espace de travail公開`cargo xtask soranet-privacy-report`はNDJSON ponctuellesをキャプチャします。 Pointez-le sur unou plusieurs 輸出管理者リレー:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

`SoranetSecureAggregator` をキャプチャする既成のパスを取得し、標準出力などの抑制を開始し、オプションで `--json-out <path|->` を介して JSON 構造を信頼します。 Il respecte les mêmes réglages que lecollecteur live (`--bucket-secs`、`--min-contributors`、`--expected-shares` など)、永続的な aux opérateurs de rejouer des は、歴史を捉え、異なる事件を記録します。 Joignez le JSON は Grafana をキャプチャし、抑制された SNNet-8 の分析を監査可能にします。

### 自動実行のプレミア実行チェックリスト

抑制のための予算を尊重するために、最高権力者による実行の自動化が必要です。 L'outil accepte désormais `--max-suppression-ratio <0-1>` afin que la CI ou les opérateurs puissent échouer Rapidement lorsque lesbucks supprimés dépassent la fenêtre autorisée (10 % par défaut) ou lorsqu'aucun Bucket n'est encore présent。推奨フラックス:

1. NDJSON のエンドポイント管理者であるエクスポータ、リレーとル フラックス `/v1/soranet/privacy/event|share` およびオーケストレータ バージョン `artifacts/sorafs_privacy/<relay>.ndjson`。
2. 政治予算の執行者:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```命令は、優先順位を観察し、コードを非ヌルに設定し、予算を最小限に抑えるために、バケットを実行する前に実行する必要があります。 Les métriques live doivent montr `soranet_privacy_pending_collectors` se vidant vers zéro et `soranet_privacy_snapshot_suppression_ratio` restant sous le même Budget ペンダント l'execution。
3. アーカイバーは、JSON とジャーナル CLI を実行して、SNNet-8 の事前文書を作成し、輸送機関の安全性を確認し、正確な成果物を確認します。

## Prochaines étapes (SNNet-8a)

- コレクタの統合は、ペイロード `SoranetPrivacyBucketMetricsV1` コヒーレントのリレーとコレクタの実行時に共有される接続ルールの取り込みを二重化します。 *(事実 — `ingest_privacy_payload` と `crates/sorafs_orchestrator/src/lib.rs` およびテスト関連のテスト。)*
- ダッシュボード Prometheus の発行者は、抑圧、収集者および匿名の規制に関する情報を提供します。 *(`dashboards/grafana/soranet_privacy_metrics.json`、`dashboards/alerts/soranet_privacy_rules.yml`、`dashboards/alerts/soranet_policy_rules.yml` およびフィクスチャの検証が完了しました。)*
- `privacy_metrics_dp.md` による機密性の高い校正の作成、およびノートブックの複製および管理のダイジェストの作成。 *(Fait — ノートブック + アーティファクトの一般的な `scripts/telemetry/run_privacy_dp.py`; ラッパー CI `scripts/telemetry/run_privacy_dp_notebook.sh` ワークフロー `.github/workflows/release-pipeline.yml` 経由でノートブックを実行; `docs/source/status/soranet_privacy_dp_digest.md` による管理のダイジェスト。)*

SNNet-8 のバージョンは、スクレーパー Prometheus およびダッシュボードの機密情報を管理するための情報です。機密性の異なる校正の成果物、リリース保守のパイプラインのワークフロー、定期的なノートブックの実行、自動実行の自動化と抑制の監視の拡張機能の集中監視などの作業を行います。