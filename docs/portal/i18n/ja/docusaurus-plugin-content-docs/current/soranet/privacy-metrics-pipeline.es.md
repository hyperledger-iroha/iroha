---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プライバシー-メトリクス-パイプライン
タイトル: SoraNet のプライバシー メトリクス パイプライン (SNNet-8)
Sidebar_label: プライバシーのメトリクスのパイプライン
説明: SoraNet のリレーとオーケストレーターのプライバシー保護のためのテレメトリア。
---

:::ノート フエンテ カノニカ
リフレジャ`docs/source/soranet/privacy_metrics_pipeline.md`。マンテンガンは、安全な医療を提供するために、引退したことを報告します。
:::

# SoraNet のプライバシーに関するメトリクスのパイプライン

SNNet-8 は、プライバシーに関するテレメトリ管理上の権限を導入します
ランタイムデルリレー。アホラ エル リレー アグリガ イベント デ ハンドシェイク サーキット en
バケット デ ウン ミント y エクスポート ソロ コンタドール Prometheus グルーソス、マンテニエンド
ロスサーキットは個人のデスビンキュラドスミエントラスオブレセビジビリダッドアクセナブル
パラ・ロス・オペラドーレス。

## 総合的な履歴書

- `tools/soranet-relay/src/privacy.rs` コミュニティでのランタイムの実装
  `PrivacyAggregator`。
- ロスバケットは、リロイの分ごとにインデックスされます (`bucket_secs`、デフォルトは 60 秒)
  リング acotado を使用してください (`max_completed_buckets`、デフォルトは 120)。損失株式
  デ・コレクター・マンティエン・ス・プロピオ・バックログ・アコタド (`max_share_lag_buckets`、デフォルトは 12)
  古いものから古いものまで、バケツのスプリミドスをバケツに入れてください。
  filtrar Memoria o enmascarar コレクター アタスカドス。
- `RelayConfig::privacy` Mapea directo、`PrivacyConfig`、exponiendo ノブの調整
  (`bucket_secs`、`min_handshakes`、`flush_delay_buckets`、`force_flush_buckets`、
  `max_completed_buckets`、`max_share_lag_buckets`、`expected_shares`)。エルランタイム
  デフォルトのミエントラ SNNet-8a の製造にしきい値が導入されました
  アグリガシオンセグラ。
- ヘルパーの情報に関するランタイム レジストラン イベントの損失モジュール:
  `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、
  `record_throttle_cooldown`、`record_capacity_reject`、`record_active_sample`、
  `record_verified_bytes`、y `record_gar_category`。

## エンドポイント管理者リレー

ロス オペラドーレス プエデン コンサルタント エル リスナー 管理者 デル リレー パラ観測
`GET /privacy/events`経由のクルーダ。 El エンドポイントの JSON デリミタの取得
nuevas lineas (`application/x-ndjson`) ペイロード `SoranetPrivacyEventV1`
リフレハドス デスデ エル `PrivacyEventBuffer` インテルノ。エルバッファレティエンロスイベントス
mas nuevos hasta `privacy.event_buffer_capacity` entradas (デフォルト 4096) y se
Vacía en la lectura、por lo que los scrapers deben sondear lo suficiente para
デジャール・ウエコスはありません。握手、スロットル、ミスマスセナーレスのイベント、
検証済み帯域幅、アクティブ回路 y GAR QUE ALIMENTANT LOS CONTADORES Prometheus、
コレクターのダウンストリーム アーカイブ ブレッドクラム セグロ パラ プライバシーを許可します。
o 確実に集約された食品ワークフロー。

## リレーの構成

テレメトリアのプライベート アーカイブのオペラドール アジャストン
セクション `privacy` によるリレーの構成:

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

SNNet-8 の有効性と一致してデフォルトが失われる
カーガー:

|カンポ |説明 |デフォルト |
|----------|---------------|----------|
| `bucket_secs` |アンチョ・デ・カダ・ベンタナ・デ・アグレガシオン（セグンドス）。 | `60` |
| `min_handshakes` |ミニモはバケットに投稿する前に投稿します。 | `12` |
| `flush_delay_buckets` |バケットはフラッシュのエスペラントを完了します。 | `1` |
| `force_flush_buckets` | Edad は、最高のバケットを提供するために最大の準備を整えています。 | `6` |
| `max_completed_buckets` |バケットのバックログ (エビタ メモリア イリミタダ)。 | `120` |
| `max_share_lag_buckets` |ベンタナは、コレクターと最高の資産を共有します。 | `12` |
| `expected_shares` |事前にコンビナールを共有します。 | `2` |
| `event_buffer_capacity` |管理ストリームに関するイベントのバックログ NDJSON。 | `4096` |

`flush_delay_buckets` による `force_flush_buckets` の設定、ポーナー
脱走兵の閾値が失われ、保持期間が過ぎて、アホラ、フォールララ
中継用テレメトリアの検証。

エル リミテ `event_buffer_capacity` タンビエン アコタ `/admin/privacy/events`、
アセグランド ケ ロス スクレーパーズ ノー ケデン アトラス インデフィニダメンテ。

## シェアデルコレクタープリオSNNet-8a のデスプリーガ コレクターは、デュアル キュー エミテン バケットのプリコン秘密共有を行います。
アホラ エル オーケストレーター パーセア エル ストリーム NDJSON `/privacy/events` タント パラ
entradas `SoranetPrivacyEventV1` コモパラ株式 `SoranetPrivacyPrioShareV1`、
`SoranetSecureAggregator::ingest_prio_share` を参照してください。ロスバケットSE
エミテン・クアンド・レガン `PrivacyBucketConfig::expected_shares` の貢献者、
リフレハンド・エル・コンポルタミエント・デル・アグレガドール・エン・エル・リレー。損失は有効です
バケットと組み合わせのヒストグラムを作成するためのラインアクション
`SoranetPrivacyBucketMetricsV1`。握手の組み合わせについて
`min_contributors` のバホ、`suppressed` のバケットをエクスポート、reflejando
エル・コンポルタミエント・デル・アグレガドール・エン・エル・リレー。ラス ベンタナス スプリミダス アホラ
Emiten una etiqueta `suppression_reason` para que los operadores distingan entre
`insufficient_contributors`、`collector_suppressed`、`collector_window_elapsed`
y `forced_flush_window_elapsed` テレメトリアの診断機能。ラ・ラゾン
`collector_window_elapsed` tambien dispara cuando las 株式を優先的に取得します
アラ デ `max_share_lag_buckets`、ハシエンド ビジブル ロス コレクター アタスカドス シン
記憶が古くなってしまった管理の蓄積。

## エンドポイントの取り込み Torii

Torii ahora expone dos エンドポイント HTTP コン テレメトリア ゲートダ パラ ケ リレー y
コレクターは、ビスポークの輸送機関を観察します:

- `POST /v2/soranet/privacy/event` ペイロードの受け入れ
  `RecordSoranetPrivacyEventDto`。 El ボディ envuelve un `SoranetPrivacyEventV1`
  マス ウナ エチケット オプション `source`。 Torii 対案の有効性
  テレメトリ アクティビティの実行、HTTP に応答するレジストラ イベント
  `202 Accepted` ジュントコンアンエンベロープ Norito JSON que contiene la ventana
  バケットの計算 (`bucket_start_unix`、`bucket_duration_secs`) y el
  モドデルリレー。
- `POST /v2/soranet/privacy/share` ペイロード `RecordSoranetPrivacyShareDto` を受け入れます。
  ボディ lleva un `SoranetPrivacyPrioShareV1` y unヒント オプション `forwarded_by`
  パラ・ケ・ロス・オペラドールはコレクターのフルホスを監査します。ラス・エントレガス・イグトーサス
  デブエルベン HTTP `202 Accepted` コンアンエンベロープ Norito JSON キュー履歴書
  コレクター、ラ・ベンタナ・デル・バケット・エル抑制のヒント。ラス・ファラス・デ
  テレメトリアの検証マップ `Conversion` パラ プリザーバー
  コレクターによる間違いの決定的なエラー。オーケストレーターのイベントループ
  アホラ エミテ エスタス シェア ミエントラ ソンデア リレー、マンテニエンド エル アキュムラドール
  Prio de Torii は、オンリレーのバケットを同期します。

Ambos エンドポイントのテレメトリアに関する情報: `503 サービスの発行
利用できません` cuando las metricas estan deshabilitadas.ロスクライアントプエデンエンビア
cuerpos Norito バイナリ (`application/x.norito`) o Norito JSON (`application/x.norito+json`);
エル サービス ネゴシア エル フォーマット自動化 (ロス エクストラクター標準デ経由)
Torii。

## メトリカス Prometheus

Cada バケットのエクスポートおよびエチケット `mode` (`entry`、`middle`、`exit`) y
`bucket_start`。家族の情報を参照してください:

|メトリカ |説明 |
|----------|---------------|
| `soranet_privacy_circuit_events_total{kind}` |ハンドシェイクに関する分類法 `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`。 |
| `soranet_privacy_throttles_total{scope}` |スロットルコンタドール `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |ハンドシェイク時のクールダウン時間の制限が抑制されました。 |
| `soranet_privacy_verified_bytes_total` |医療セガダの証明を確認します。 |
| `soranet_privacy_active_circuits_{avg,max}` |メディアとピコデ回路はバケットごとに活動します。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT のパーセント値の推定 (`p50`、`p90`、`p99`)。 |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores GAR は、カテゴリーのダイジェストをハヘッドスします。 |
| `soranet_privacy_bucket_suppressed` |バケツは、貢献できるすべての情報を収集します。 |
| `soranet_privacy_pending_collectors{mode}` |蓄積されたデータは、組み合わせのペンディエンテス、リレーの実行方法を共有します。 |
| `soranet_privacy_suppression_total{reason}` |プライバシー ギャップに関する `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` のバケツ管理。 |
| `soranet_privacy_snapshot_suppression_ratio` |スーププリミド/究極のデスカルガドの比率 (0-1)、有効なプレスプエストス デ アラート。 |
| `soranet_privacy_last_poll_unixtime` | UNIX の最終的なポーリング終了のタイムスタンプ (コレクタ アイドル状態のアラート)。 |
| `soranet_privacy_collector_enabled` |ゲージ que cae a `0` cuando el Privacy Collector Esta deshabilitado o falla al iniciar (alimenta laalerta Collector-disabled)。 |
| `soranet_privacy_poll_errors_total{provider}` |リレーのエイリアスによるポーリングのフォールス (デコード エラーの増加、エラーの発生による HTTP エラー)。 |

ロスバケットは永続的な監視、管理ダッシュボードを監視します
リンピオスの罪はファブリカル・ベンタナス・コン・セロス。

## ギア オペラティバ1. **ダッシュボード** - `mode` による前段階のグラフ
   `window_start`。コレクターによる明らかな問題の解決策
   ○リレー。米国 `soranet_privacy_suppression_total{reason}` の識別
   コレクターのトリアージに対する抑制の貢献。エルアセット
   Grafana ahora incluye un pane dedicado **「抑制理由 (5 分)」**
   統計上の統計 **「抑制されたバケット %」** クエリ
   計算 `sum(soranet_privacy_bucket_suppressed) / count(...)` による選択
   パラ・ケ・ロス・オペラドーレスは、展望台の展望台を訪れます。ラ・セリエ
   **コレクターシェアバックログ** (`soranet_privacy_pending_collectors`) y el stat
   **スナップショット抑制率** レサルタン コレクターのアタスカドスと予算の変動
   デュランテの排出自動化。
2. **警告** - プライバシー保護に関する警告: picos de rechazo
   PoW、クールダウン頻度、RTT 容量のドリフトが拒否します。コモ・ロス
   コンタドール ソン モノトニック デントロ デ カダ バケット、レグラス バサダス アン タサ
   ファンシナンビアン。
3. **インシデント対応** - 一連の事件を解決します。クアンド・セ・レクイエラ
   深遠な不正行為、バケットの再現スナップショットを中継する要請
   交通機関の記録を収集した医療機関の証拠を検査する
   クルード。
4. **保持** - 頻度を超えたものを十分に除去しません
   `max_completed_buckets`。ロス エクスポーターズ デベン トラタール ラ サリダ Prometheus コモ
   ラ・フェンテ・カノニカとデカール・バケット・ロケール・ウナ・ベス・レエンビアドス。

## 検出と自動処理の分析

SNNet-8 の受け入れは、自動コレクターのデモストラルに依存します
マンティネン・サノスと政策の限界を永久に維持する
(30 分間のリレーのバケット数 <=10%)。エルツーリング
必要な場合は、エル レポが含まれます。ロス オペラドーレス デベン インテグラルロ エン サス
セマナレスの儀式。 Grafana リフレジャン ロスのヌエボス パネルの制限
スニペット PromQL のアバホ、オンコール前の状態での生体内での可視化
マニュアルを繰り返し参照してください。

### Recetas PromQL パラリバイザーの抑制

ロス オペラドーレス デベン テナー、マノ ロス シギエンテス ヘルパー PromQL。アンボス・セ
ダッシュボード比較リファレンス (`dashboards/grafana/soranet_privacy_metrics.json`)
アラートマネージャーの規則:

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

米国比率を確認するための統計 **「抑制されたバケット %」** を確認
政策を事前に決定する。スパイク検出器を接続
Alertmanager によるフィードバックの迅速な提供による貢献
フォルマ・イネスペラダ。

### CLI によるバケットのオフラインレポート

El ワークスペース expone `cargo xtask soranet-privacy-report` キャプチャ NDJSON
プントゥアレス。 Apuntalo a uno o mas エクスポート管理者リレー:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

`SoranetSecureAggregator` のキャプチャ手順をヘルパーが再開し、
標準出力とオプションの制限を使用してレポートを記述し、JSON 構造を作成します
`--json-out <path|->`。生体内でのコレクターのようなミスモスのノブのレスペタ
(`--bucket-secs`、`--min-contributors`、`--expected-shares` など)、許可
歴史上の記録を再現し、問題の閾値を調査します。
Grafana の分析ゲートの追加と JSON のスクリーンショット
SNNet-8 siga siendo 監査可能。

### 自動排出初期チェックリスト

ガバナンスは、最初の段階で自動的に実行されます。
プレスプエスト・デ・スプレション。エル ヘルパー アホラ アセプタ `--max-suppression-ratio <0-1>`
パラ ケ CI u オペラドールズ フォールド ラピド クアンド ロス バケツ スプリミドス エクセデン ラ
ベンタナ許可 (デフォルト 10%) o cuando aun 干し草バケツを提示しません。フルージョ
お勧め:

1. NDJSON のエンドポイント管理、リレー、ストリームのエクスポート
   `/v2/soranet/privacy/event|share` オーケストレーター ハシア
   `artifacts/sorafs_privacy/<relay>.ndjson`。
2. 政策を支援するための政策:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   最高のコマンドと、最高の比率を監視し、安全な販売を実現する
   cuando se excede el presupuesto **o** cuando 干し草バケツ リストス、インディカンド
   テレメトリーを確認すると、排出された情報が生成されます。ラス・メトリックス・エン
   vivo deben mostrar `soranet_privacy_pending_collectors` drenado hacia cero y
   `soranet_privacy_snapshot_suppression_ratio` ケダンドース バホ エル ミスモ
   presupuesto mientras se ejecuta la corrida。
3. SNNet-8 の JSON アーカイブとログと CLI コントロールの証拠バンドル
   カンビアエル輸送のデフォルトパラケロスレビュープエダン
   再現ロス・アーティファクト・エククトス。

## プロキシモス パソス (SNNet-8a)- インテグラル ロス コレクター Prio Duales、コネクタンド ラ インジェスタ デ 共有のランタイム
  パラケリレー、コレクター、エミタンペイロード `SoranetPrivacyBucketMetricsV1`
  一貫しています。 *(完了 - バージョン `ingest_privacy_payload` ja
  `crates/sorafs_orchestrator/src/lib.rs` y はアソシアドをテストします。)*
- パブリック エル ダッシュボード Prometheus compartido y reglas de alerta que cubran
  抑圧のギャップ、コレクターの賞賛、そして匿名性の低下。 *(完了 - バージョン
  `dashboards/grafana/soranet_privacy_metrics.json`、
  `dashboards/alerts/soranet_privacy_rules.yml`、
  `dashboards/alerts/soranet_policy_rules.yml`、y フィクスチャの検証。)*
- 差分プライバシーの校正の作成者
  `privacy_metrics_dp.md`、再現ノートブックとダイジェストが含まれます
  ガバナンス。 *(完了 - ノートブック + 生成されたアーティファクト
  `scripts/telemetry/run_privacy_dp.py`; CIラッパー
  `scripts/telemetry/run_privacy_dp_notebook.sh` ejecuta el ノートブック経由 el
  ワークフロー `.github/workflows/release-pipeline.yml`;ガバナンス ダイジェスト アーカイブ
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

LA バージョンの実際のベース SNNet-8: テレメトリア決定とセグラ
プライバシー ポリシー インテグラ ダイレクト コン スクレーパーとダッシュボード Prometheus
存在者。差分プライバシーの校正のロス アーティファクト リスト、
ノートブック、トラバホなどのリリースのマンティエン・フレカス・ラス・サリダスのワークフロー
Restante se enfoca en Monitorear la primera ejecucion automatizada y extender
抑圧に関する警告の分析。