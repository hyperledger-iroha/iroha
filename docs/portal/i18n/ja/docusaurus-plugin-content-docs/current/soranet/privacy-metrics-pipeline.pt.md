---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プライバシー-メトリクス-パイプライン
タイトル: SoraNet のプライバシー指標パイプライン (SNNet-8)
Sidebar_label: プライバシーの指標のパイプライン
説明: SoraNet のリレーとオーケストレーターのプライバシーを保護するためのテレメトリ情報。
---

:::note フォンテ カノニカ
リフレテ`docs/source/soranet/privacy_metrics_pipeline.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

# SoraNet のプライバシーに関するメトリクスのパイプライン

SNNet-8 は、中継を実行するランタイムのプライバシーに関する監視機能を紹介します。 O 中継アゴラ アグレガ イベント デ ハンドシェイク 電子サーキット エム バケツ デ ウム 分 e エクスポート アペナス コンタドール Prometheus グロッソス、マンテンド サーキットは個別のデスビンキュラドス エンクアント フォルネセ ビジビリダーデ アシオナベル AOS オペラドールです。

## ヴィサオ・ジェラル・ド・アグレガドール

- `tools/soranet-relay/src/privacy.rs` と `PrivacyAggregator` のランタイムを実装します。
- 時間ごとのバケット数 (`bucket_secs`、デフォルト 60 秒) とリング制限数 (`max_completed_buckets`、デフォルト 120)。コレクターは、固有のバックログ制限 (`max_share_lag_buckets`、デフォルト 12) を管理して、ジャネラス プリオ アンチガス セジャム エスヴァジアダ コモ バケット スプリーミドス エム ベズ デ バザール メモリア オイ マスカラー コレクター プレソスを共有します。
- `RelayConfig::privacy` `PrivacyConfig` のマペイアディレト、調整ノブの拡張 (`bucket_secs`、`min_handshakes`、`flush_delay_buckets`、`force_flush_buckets`、`max_completed_buckets`、 `max_share_lag_buckets`、`expected_shares`)。ランタイムの製造管理 OS のデフォルトは、SNNet-8a の統合しきい値を設定します。
- ヘルパー情報によるランタイム レジストラム イベントのモジュール: `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、`record_throttle_cooldown`、`record_capacity_reject`、`record_active_sample`、 `record_verified_bytes`、e `record_gar_category`。

## エンドポイント管理者はリレーを実行します

OS オペラドール ポデム コンサルタントまたはリスナー管理者は、`GET /privacy/events` 経由で観測ブルータスを中継します。 O エンドポイント レポートは、novas linhas (`application/x-ndjson`) コンテンツ ペイロード `SoranetPrivacyEventV1` espelhados do `PrivacyEventBuffer` interno の JSON 区切りです。 O バッファーは、`privacy.event_buffer_capacity` エントリ (デフォルト 4096) を保護するイベント (デフォルトは 4096) を記録し、セキュリティ レベルに応じて十分な頻度で情報を収集し、スクレーパーを開発します。ハンドシェイクのイベント コブレム、スロットル、検証済み帯域幅、アクティブ回路、GAR クエリ アリメンタム os contadores Prometheus、コレクタのダウンストリーム アーカイブのパンくずリストのプライバシーとアリメンタ ワークフローの管理を許可します。

## リレーを設定します

OS オペラドールは、セカンダリ `privacy` 経由で中継を行うための設定を行わず、テレメトリアのプライバシーを確立します。

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

OS のデフォルトは、特定の SNNet-8 に対応しており、有効な有効性はありません:

|カンポ |説明 |パドラオ |
|------|-----------|----------|
| `bucket_secs` | Largura de cada janela de agregacao (セグンドス)。 | `60` |
| `min_handshakes` |バケット ポーダーの送信元の数を数えます。 | `12` |
| `flush_delay_buckets` |バケットは、事前のフラッシュを完了します。 | `1` |
| `force_flush_buckets` |バケツを最大限に活用してください。 | `6` |
| `max_completed_buckets` |バケットのバックログ (メモリの制限を妨げます)。 | `120` |
| `max_share_lag_buckets` | Janela de retencao パラコレクターは、最高の商品を共有しています。 | `12` |
| `expected_shares` |プリオコレクターは、コンビナールでの取引を共有します。 | `2` |
| `event_buffer_capacity` |ストリーム管理者による NDJSON イベントのバックログ。 | `4096` |

`force_flush_buckets` の基準 `flush_delay_buckets` を定義し、OS のしきい値を定義し、リレーのバザリアム テレメトリアでのセキュリティの監視を開始します。

`event_buffer_capacity` の制限 `/admin/privacy/events` を制限し、スクレイパーは、ポッサム フィカル アトラサドスを定義できません。

## Prio コレクター株SNNet-8a インプラント コレクター デュプロス キュー エミテム バケット Prio com compartilhamento Secreto。オーケストレーター アゴラ アナリサ ストリーム NDJSON `/privacy/events` エントラダス `SoranetPrivacyEventV1` は、`SoranetPrivacyPrioShareV1` を共有し、`SoranetSecureAggregator::ingest_prio_share` としてエンカミンハンドを共有します。バケットは、リレーを行うためのコンポルタメント、エスペルハンド、`PrivacyBucketConfig::expected_shares` を提供します。バケットの有効性を共有するため、`SoranetPrivacyBucketMetricsV1` の組み合わせでヒストグラムを実行します。 `min_contributors` でハンドシェイクを組み合わせて、`suppressed` でバケットとエクスポートを行い、中継なしでコンポルタメントを行います。 Janelas suprimidas は、ラベル `suppression_reason` パラケ オペラドールの所有者識別エントリ `insufficient_contributors`、`collector_suppressed`、`collector_window_elapsed`、および `forced_flush_window_elapsed` テレメトリの診断能力を備えています。 O motivo `collector_window_elapsed` は、`max_share_lag_buckets` で、プリオが `max_share_lag_buckets` の情報を共有し、トルナンド コレクターが、訪問者を訪問し、記憶を蓄積します。

## Torii を実行するエンドポイントの取り込み

Torii アゴラは、エンドポイントの HTTP 通信ゲートウェイ テレメトリア パラケ リレー、コレクター、ポッサム エンカミンハール オブザーバコエスエム エンブティルム トランスポーテ ビスポーク:

- `POST /v1/soranet/privacy/event` ペイロード `RecordSoranetPrivacyEventDto` をサポートします。 O corpo envolve um `SoranetPrivacyEventV1` は、オプションのラベル `source` を作成します。 Torii テレメトリーの要求事項を検証、登録イベント、電子応答 com HTTP `202 Accepted` junto com um envelope Norito JSON contendo a janela calculada (`bucket_start_unix`, `bucket_duration_secs`) e o modo do リレー。
- `POST /v1/soranet/privacy/share` ペイロード `RecordSoranetPrivacyShareDto` をサポートします。 O corpo carrega um `SoranetPrivacyPrioShareV1` e uma dica `forwarded_by` opcional para que operadores possam Auditar fluxos de collections. bem-sucedidas retornam HTTP `202 Accepted` com um envelope Norito JSON resumindo o コレクター、ジャネラ デ バケット、および a dica de supressao を送信します。ファルハス デ バリダカオ マペイアム パラ ウマ レスポスタ デ テレメトリア `Conversion` パラ保存トラタメント決定論的エラー エントレ コレクター。 O ループ デ イベントは、オーケストレーター アゴラのエミット essas 共有、fazer ポーリング、dos リレー、mantendo o acumulador Prio do Torii sincronizado com os バケット、リレーなし。

OS エンドポイントは、テレメトリの実行を許可します。`503 Service Unavailable` をメトリクスとして送信します。クライアントのポデム環境情報 Norito バイナリ (`application/x.norito`) または Norito JSON (`application/x.norito+json`)。 o servidor negocia autoamente o formato via extractors Padrao do Torii.

## メトリカス Prometheus

Cada バケットエクスポート、カレガラベル `mode` (`entry`、`middle`、`exit`) e `bucket_start`。家族の安全保障として、次のように述べています。

|メトリック |説明 |
|----------|---------------|
| `soranet_privacy_circuit_events_total{kind}` |ハンドシェイク分類法 com `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`。 |
| `soranet_privacy_throttles_total{scope}` |コンタドール デ スロットル コム `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |ハンドシェイクのクールダウン期間が調整されました。 |
| `soranet_privacy_verified_bytes_total` |医療機関の帯域幅検証。 |
| `soranet_privacy_active_circuits_{avg,max}` |メディアとバケットの回路。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT パーセントの推定値 (`p50`、`p90`、`p99`)。 |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores de Governance Action Report com のハッシュまたはダイジェスト カテゴリ。 |
| `soranet_privacy_bucket_suppressed` |バケツは、投稿を投稿したり、投稿したりすることができます。 |
| `soranet_privacy_pending_collectors{mode}` |コレクターの蓄積は、組み合わせのペンダント、リレーの方法を共有します。 |
| `soranet_privacy_suppression_total{reason}` |バケット管理者は `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` パラケ ダッシュボードにアクセスし、プライバシーを確​​保します。 |
| `soranet_privacy_snapshot_suppression_ratio` | Razao suprimida/drenada は究極のドレイン (0-1)、予算を大幅に削減します。 |
| `soranet_privacy_last_poll_unixtime` |タイムスタンプ UNIX は、究極のポーリング bem-sucedido (コレクターアイドルのアラート) を実行します。 |
| `soranet_privacy_collector_enabled` |ゲージ QUE VIRA `0` は、コレクターからプライバシーを保護されており、ファルハからの開始を要求しています (コレクターが無効になっている警告)。 |
| `soranet_privacy_poll_errors_total{provider}` |リレーエイリアスによるポーリングアグループの実行 (デコードエラーの増加、HTTP の実行、ステータスコードの増加)。 |

バケットは永続的に監視され、沈黙し、mantendo のダッシュボードは製造されたジャネラス ゼラダを監視します。

## オリエンタカオ オペラシオナル1. **ダッシュボード** - `mode` e `window_start` のメトリクスとしてトレースします。コレクターとリレーの明らかな問題を解決します。 `soranet_privacy_suppression_total{reason}` para distinguir falta de contribuidores de supressoes orientadas por collections ao triar lacunas を使用してください。 O 資産 Grafana は、**「抑制の理由 (5 分)」** 統計を取得し、**「抑制されたバケット %」** 計算式 `sum(soranet_privacy_bucket_suppressed) / count(...)` を予算のオペランドから選択します。急速です。一連の **「コレクター共有バックログ」** (`soranet_privacy_pending_collectors`) 統計 **「スナップショット抑制率」** は、予算期間中のコレクターの事前実行とスケジュールの自動実行を自動化します。
2. **アラート** - conduza は、プライベート制御の一部を警告します: PoW 拒否、クールダウン頻度、RTT 容量拒否のドリフト。コモ・オス・コンタドールは、モノトーンのデントロ・デ・カダ・バケット、単純なベースアダおよび分類機能ベムを確認します。
3. **インシデント対応** - 初期の状況を確認します。必要なデバッグは広範囲に行われ、バケットのスナップショットの再現や検査、医療機関の検査、交通事故のログの収集などを依頼します。
4. **保持** - 顔のスクレイピング コム 周波数で十分なエビタール エクシーダー `max_completed_buckets`。輸出業者は、Prometheus フォント カノニカとダウンロード バケットの場所を指定して、トラタールを開発します。

## 自動制御と実行の分析

SNNet-8 は、デモンストレーション コレクターの自動化と永続的な監視に依存し、政治的制限を抑制します (30 分間のリレーでの実行バケット数が 10% 未満)。 O ツールは、リポジトリのアゴラ ヴェム ゲート ゲートに必要なツールです。オペラドールは、すべてが統合されたものを開発し、儀式を行います。 Grafana は、PromQL を参照して、植物の可視性を維持し、生体内で正確な調査を行って、マニュアルを確認します。

### PromQL の改訂を確認する

オペラドールは、PromQL と Mao のヘルパーを開発します。ダッシュボード Grafana compartilhado (`dashboards/grafana/soranet_privacy_metrics.json`) と Alertmanager の参照:

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

**「抑制されたバケット %」** の統計を確認するには、政治的な予算を永続的に維持するための比率を使用します。スパイク検出器とアラートマネージャーのフィードバックを迅速に接続し、安全性を確保します。

### バケットとの関係をオフラインにする CLI

O ワークスペース expo `cargo xtask soranet-privacy-report` は、NDJSON ポントゥアイをキャプチャします。リレーの輸出管理を担当します:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

`SoranetSecureAggregator` をキャプチャして、標準出力の制限なしで、必要に応じて、`--json-out <path|->` 経由で JSON を参照できるようにしてください。エレホンラオスのメスモスノブは、コレクターの生体内（`--bucket-secs`、`--min-contributors`、`--expected-shares`など）を実行し、オペラドールの再現を許可し、歴史をキャプチャし、すすり泣きのしきい値の違いと裁判の事故を許可します。 Grafana ゲートの解析と SNNet-8 の永続的な監査を実行するための JSON のキャプチャの付録。

### 自動実行のチェックリスト

予算を管理するために、事前に自動実行を実行する必要があります。ああ、ヘルパー アゴラ アセイタ `--max-suppression-ratio <0-1>` パラ ケ CI オイ オペラドール ファルヘム ラピダメンテ クアンド バケット スプリーミドスを超えてジャネラ許可 (デフォルト 10%) オイ クアンド アインダ ナオ フーバー バケット。 Fluxo の推奨事項:

1. NDJSON dos エンドポイント管理者、リレー メイン、ストリーム `/v1/soranet/privacy/event|share`、オーケストレーター パラ `artifacts/sorafs_privacy/<relay>.ndjson` をエクスポートします。
2. 政治予算を支援する:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   ああ、私は、ゼロの予算を実行するために、予算と予算を実行する命令を実行します。私は、バケットをすぐに実行し、実行するために、テレメトリを実行する必要があります。生体内で最も重要な指標として `soranet_privacy_pending_collectors` ゼロのゼロを設定するか、実行予算を設定してください。
3. CLI com o pacote de evidencias SNNet-8 antes de trocar odefault do Transporte para que revisores possam reproduzir os artefatos をアーカイブします。

## プロキシモス パソス (SNNet-8a)- インテグラル OS デュアル Prio コレクター、取り込み共有、ランタイム パラケ リレー、コレクターのエミッタム ペイロードの接続 `SoranetPrivacyBucketMetricsV1` は一貫しています。 *(結論 - veja `ingest_privacy_payload` em `crates/sorafs_orchestrator/src/lib.rs` e os testes associados.)*
- ダッシュボード Prometheus の情報を確認し、管理者が管理するコブリンドの情報を確認し、コレクターと匿名の情報を確認します。 *(結論 - 検証済みの `dashboards/grafana/soranet_privacy_metrics.json`、`dashboards/alerts/soranet_privacy_rules.yml`、`dashboards/alerts/soranet_policy_rules.yml` フィクスチャ)*
- `privacy_metrics_dp.md` の異なる詳細な記述を作成し、ノートブックの複製と統治のダイジェストを含みます。 *(結論 - `scripts/telemetry/run_privacy_dp.py` によるノートブックとアーティファトのジェラドス。ラッパー CI `scripts/telemetry/run_privacy_dp_notebook.sh` ワークフロー経由でノートブックを実行 `.github/workflows/release-pipeline.yml`; 統治アルキバードのダイジェスト `docs/source/status/soranet_privacy_dp_digest.md`。)*

SNNet-8 を実行するための実際のリリース: テレメトリアの決定性とプライバシーを保護するためのスクレーパーとダッシュボード Prometheus が存在します。 OS は、管理の異なるプライベート スタオの技術、ワークフローのリリース パイプライン管理、OS 出力のノートブックの自動化、監視なしでの初期実行の自動化、および監視の拡張機能の分析を行います。