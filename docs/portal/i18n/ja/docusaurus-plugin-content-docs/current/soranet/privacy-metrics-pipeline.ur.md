---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プライバシー-メトリクス-パイプライン
タイトル: SoraNet پرائیویسی میٹرکس پائپ لائن (SNNet-8)
サイドバーラベル: پرائیویسی میٹرکس پائپ لائن
説明: SoraNet はオーケストレーターをリレーし、テレメトリーはテレメトリーを制御します。
---

:::note メモ
`docs/source/soranet/privacy_metrics_pipeline.md` ٩ی عکاسی کرتا ہے۔ پرانے docs کے ختم ہونے تک دونوں نقول ہم وقت رکھیں۔
:::

#SoraNet پرائیویسی میٹرکس پائپ لائن

SNNet-8 リレー ランタイムの実行とテレメトリの実行リレー ハンドシェイク 回路 اقعات کو ایک منٹ کے バケット میں جمع کرتا ہے اور صرف موٹے Prometheus カウンター برآمد کرتا回路がリンク不能です ہے، جس سے انفرادی 回路がリンクできません رہتے ہیں جبکہ آپریٹرز کو قابلِ عمل بصیرت ملتی ہے۔

## アグリゲーター خلاصہ

- ランタイム実装 `tools/soranet-relay/src/privacy.rs` میں `PrivacyAggregator` کے طور پر موجود ہے۔
- バケツの壁時計、キー、キー (`bucket_secs`、デフォルトは 60 秒) リング (`max_completed_buckets`、デフォルトは 60 秒) 120) 아니다 다니다.コレクターの共有バックログ (`max_share_lag_buckets`、デフォルト 12) の Prio Windows 抑制されたバケット フラッシュ フラッシュ メモリ リーク コレクターのスタック スタック コレクターऔर देखें
- `RelayConfig::privacy` マップ `PrivacyConfig` マップ マップ チューニング ノブ (`bucket_secs`、`min_handshakes`、`flush_delay_buckets`、 `force_flush_buckets`、`max_completed_buckets`、`max_share_lag_buckets`、`expected_shares`)本番環境のランタイムのデフォルト値 SNNet-8a の安全な集計しきい値
- ランタイムの型付きヘルパー イベント イベント: `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、`record_throttle_cooldown`、 `record_capacity_reject`、`record_active_sample`、`record_verified_bytes`、`record_gar_category`۔

## リレー管理エンドポイント

آپریٹرز `GET /privacy/events` ذریعے リレー کے 管理者リスナー 投票 投票 生の観察結果 سکتے ہیں۔エンドポイント改行区切り JSON (`application/x-ndjson`) واپس کرتا ہے جس میں `SoranetPrivacyEventV1` ペイロード ہوتے ہیں جو اندرونی `PrivacyEventBuffer`鏡 ہوتے ہیں۔バッファーのイベント `privacy.event_buffer_capacity` エントリーの数 (デフォルトは 4096) 読み取り、ドレインの数、スクレーパー、ギャップ数の数。投票する 投票する 投票するイベント、ハンドシェイク、スロットル、検証済み帯域幅、アクティブ回路、GAR 信号、Prometheus カウンター、ダウンストリーム コレクター安全な集計ワークフロー フィード パンくずリスト 安全な集計ワークフロー

## リレー構成

リレー構成 `privacy` ステータス テレメトリ ケイデンス ステータス:

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

フィールドのデフォルトは SNNet-8 仕様です:

|フィールド |説明 |デフォルト |
|----------|---------------|----------|
| `bucket_secs` |集計ウィンドウ ٩ی چوڑائی (秒)۔ | `60` |
| `min_handshakes` |貢献者数をカウントする バケット カウンターが排出する ہے۔ | `12` |
| `flush_delay_buckets` |フラッシュ کی کوشش سے پہلے مکمل バケツ کی تعداد۔ | `1` |
| `force_flush_buckets` |抑制されたバケットが放出する سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` |バケットのバックログ (無制限のメモリ) (無制限のメモリ) | `120` |
| `max_share_lag_buckets` |抑制、コレクター株式、保持期間、 | `12` |
| `expected_shares` |プリオコレクター株を結合する| `2` |
| `event_buffer_capacity` |管理ストリーム NDJSON イベント バックログ| `4096` |

`force_flush_buckets` `flush_delay_buckets` しきい値のしきい値 リテンション ガードの検証の失敗導入の監視、リレーごとのテレメトリのリークの監視

`event_buffer_capacity` کی حد `/admin/privacy/events` کو بھی バウンド کرتی ہے، تاکہ スクレイパー غیر معینہ مدت تک پیچھے نہ رہああ

## Prio コレクター株SNNet-8a デュアル コレクターは、秘密共有 Prio バケットからの出力を実行します。オーケストレーター `/privacy/events` NDJSON ストリーム `SoranetPrivacyEventV1` エントリ `SoranetPrivacyPrioShareV1` シェア `SoranetSecureAggregator::ingest_prio_share` 解析 `SoranetSecureAggregator::ingest_prio_share`先に進みます ہے۔バケツの出力 ہوتے ہیں جب `PrivacyBucketConfig::expected_shares` の貢献数 آ جائیں، جو リレー動作 کی عکاسی ہے۔株式数、バケットの調整、ヒストグラムの形状、検証、評価、評価、`SoranetPrivacyBucketMetricsV1`、結合、評価、評価、評価、検証結合ハンドシェイク数 `min_contributors` ハンドシェイク数 `min_contributors` バケット数 `suppressed` エクスポート数 リレー内アグリゲーターの動作やあ抑制されたウィンドウ `suppression_reason` ラベルが発行される `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, اور `forced_flush_window_elapsed`テレメトリのギャップ کی تشخیص کر رہے ہوں۔スタックコレクター古いアキュムレータのメモリ میں نہیں رہتے۔

## Torii 取り込みエンドポイント

Torii テレメトリ ゲート HTTP エンドポイント、リレー、コレクター、オーダーメイドのトランスポート、観測転送、次のとおりです。

- `POST /v2/soranet/privacy/event` ایک `RecordSoranetPrivacyEventDto` ペイロード قبول کرتا ہے۔ボディ میں `SoranetPrivacyEventV1` کے ساتھ ایک オプション `source` ラベル شامل ہوتا ہے۔ Torii リクエスト テレメトリ プロファイル کے مطابق 検証 کرتا ہے، イベント ریکارڈ کرتا ہے، اور HTTP `202 Accepted` کے ساتھ Norito JSON エンベロープ 計算バケット ウィンドウ (`bucket_start_unix`、`bucket_duration_secs`) リレー モード
- `POST /v2/soranet/privacy/share` ایک `RecordSoranetPrivacyShareDto` ペイロード قبول کرتا ہے۔本文 `SoranetPrivacyPrioShareV1` オプション `forwarded_by` ヒント ہوتا ہے تاکہ آپریٹرز コレクタ フロー 監査 監査 評価送信 HTTP `202 Accepted` ساتھ Norito JSON エンベロープ واپس کرتی ہیں جو コレクター، バケット ウィンドウ ، اور 抑制ヒント کو 要約 کرتا ہے؛検証失敗テレメトリー `Conversion` 応答 マップ ہیں コレクター コレクター 決定論的エラー処理 決定論的エラー処理オーケストレーター イベント ループ リレー ポーリング 株式の発行 リレー バケット Torii Prio アキュムレータ オンリレー バケット 同期 同期

エンドポイント テレメトリ プロファイル پابندی کرتے ہیں: メトリクスが無効です ہونے پر `503 Service Unavailable` واپس کرتے ہیں۔クライアント Norito バイナリ (`application/x.norito`) Norito JSON (`application/x.norito+json`) ボディサーバー標準 Torii エクストラクター フォーマット ネゴシエート ہے۔

## Prometheus メトリクス

エクスポートされたバケット `mode` (`entry`, `middle`, `exit`) `bucket_start` ラベル ہوتے ہیں۔メトリック ファミリは次のメッセージを出力します。

|メトリック |説明 |
|----------|---------------|
| `soranet_privacy_circuit_events_total{kind}` |ハンドシェイク分類法 `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` ハンドシェイク分類法|
| `soranet_privacy_throttles_total{scope}` |スロットル カウンター `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` ہے۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |スロットルハンドシェイク سے آنے والی クールダウン مدتوں کا مجموعہ۔ |
| `soranet_privacy_verified_bytes_total` |ブラインド測定の証明、帯域幅の検証|
| `soranet_privacy_active_circuits_{avg,max}` |バケット アクティブ回路 اوسط اور زیادہ سے زیادہ۔ |
| `soranet_privacy_rtt_millis{percentile}` | RTT パーセンタイル推定値 (`p50`、`p90`、`p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` |ハッシュ化されたガバナンス アクション レポート カウンター カテゴリー ダイジェスト キー ہوتے ہیں۔ |
| `soranet_privacy_bucket_suppressed` | وہ バケット数 جو 投稿者のしきい値 پورا نہ ہونے کی وجہ سے روکے گئے۔ |
| `soranet_privacy_pending_collectors{mode}` |コレクターシェアアキュムレータ 結合 ہونے کے منتظر ہیں، リレーモード کے حساب سے گروپ کیے گئے۔ |
| `soranet_privacy_suppression_total{reason}` |抑制されたバケットカウンター セキュリティ `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` セキュリティ ダッシュボード プライバシー ギャップ セキュリティ ギャップ|
| `soranet_privacy_snapshot_suppression_ratio` |ドレイン 抑制/ドレイン比 (0-1) アラート予算 مفید۔ |
| `soranet_privacy_last_poll_unixtime` | UNIX タイムスタンプ (コレクター アイドル アラート) (コレクター アイドル アラート) |
| `soranet_privacy_collector_enabled` |ゲージ جو اس وقت `0` ہو جاتا ہے جب Privacy Collector بند ہو یا start ہونے میں ناکام ہو (コレクター無効アラート کیلئے)۔ |
| `soranet_privacy_poll_errors_total{provider}` |ポーリングの失敗、リレー エイリアス کے حساب سے گروپ ہوتے ہیں (デコード エラー、HTTP 失敗、، یا غیر、متوقع ステータス コード پر بڑھتے ہیں)۔ |

バケット 監視 監視 監視 監視 監視 監視 監視 監視 ダッシュボード 監視 ゼロ埋めウィンドウ 監視やあ

## 操作ガイド1. **ダッシュボード** - メトリクス `mode` `window_start` حساب سے گروپ کر چارٹ کریں۔見つからないウィンドウ アイコン ハイライト アイコン コレクター リレー メッセージ メッセージ`soranet_privacy_suppression_total{reason}` ギャップ ギャップ トリアージ 貢献者不足 コレクター主導の抑圧 درمیان فرق ہو سکے۔ Grafana アセット اب ایک 専用 **「抑制理由 (5 分)」** パネル فراہم کرتا ہے جو ان カウンター سے چلتا ہے، ساتھ ہی ایک **「抑制されたバケット %」**ステータス جو `sum(soranet_privacy_bucket_suppressed) / count(...)` فی 選択 حساب کرتا ہے تاکہ آپریٹرز 予算違反 ایک نظر میں دیکھ سکیں۔ **「コレクター共有バックログ」** シリーズ (`soranet_privacy_pending_collectors`) **「スナップショット抑制率」** 統計スタックコレクター、自動化された実行、予算のドリフト、
2. **アラート** - プライバシー保護カウンター、アラーム、PoW 拒否スパイク、クールダウン頻度、RTT ドリフト、容量拒否単調なカウンター ہونکہ ہر バケット کے اندر カウンター単調 ہوتے ہیں، سادہ レートベースのルール اچھا کام کرتے ہیں۔
3. **インシデント対応** - 集計データの収集デバッグ リレー バケット スナップショット リプレイ ブラインド測定の証明 生のトラフィック ログ トラフィック ログ
4. **保持** - `max_completed_buckets` سے تجاوز سے بچنے کیلئے کافی بار scrape کریں۔エクスポータ Prometheus 出力 正規ソース フォワード フォワード ローカル バケット ローカル バケット

## 抑制分析と自動実行

SNNet-8 のセキュリティ セキュリティ セキュリティ 自動コレクター セキュリティ抑制 セキュリティ セキュリティ(リレー کیلئے کسی بھی 30 منٹ کی window میں ≤10% バケット)۔ゲートゲートの管理 ツーリングの管理 リポジトリ管理 ツールの管理آپریٹرز کو اسے اپنی ہفتہ وار 儀式 میں شامل کرنا ہوگا۔ Grafana サプレッション パネル دیے PromQL スニペット کو منعکس کرتے ہیں، جس سے オンコール ٹیموں کو ライブ可視性 ملتی ہے اس سے پہلے کہ انہیں 手動クエリ کی ضرورت پڑے۔

### 抑制のレビュー PromQL レシピ

PromQL ヘルパーのサポート共有 Grafana ダッシュボード (`dashboards/grafana/soranet_privacy_metrics.json`) アラートマネージャー ルールが参照されました:

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

比率出力 **「抑制されたバケット %」** 統計情報 予算情報スパイク検出器 アラートマネージャー 担当者 担当者 担当者 担当者 担当者 担当者 担当者 担当者 担当者 担当者 担当者 担当者

### オフライン バケット レポート CLI

ワークスペース میں `cargo xtask soranet-privacy-report` ایک وقتی NDJSON キャプチャ کیلئے دستیاب ہے۔リレー管理者のエクスポートのポイント:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

ヘルパー キャプチャ `SoranetSecureAggregator` ストリーム ストリーム 標準出力抑制の概要 サポート サポート サポート`--json-out <path|->` 構造化 JSON レポートライブ コレクター ノブ (`--bucket-secs`、`--min-contributors`、`--expected-shares`、وغیرہ) 名誉 کرتا ہے، جس سے آپریٹرز 問題 トリアージしきい値を設定する 歴史的なキャプチャをリプレイする ہیں۔ SNNet-8 抑制分析ゲートの監査、JSON、Grafana のスクリーンショット、添付、添付

### 自動実行チェックリスト

ガバナンス 管理 自動化 抑制予算 管理ヘルパー `--max-suppression-ratio <0-1>` قبول کرتا ہے تاکہ CI یا آپریٹرز اس وقت フェイルファスト کر سکیں 抑制されたバケットの許可ウィンドウ سے بڑھ جائیں (デフォルト 10%) یا جب ابھی کوئی バケット موجود نہ ہوں۔流れ:

1. リレー管理エンドポイントとオーケストレーター `/v2/soranet/privacy/event|share` ストリーム NDJSON エクスポート `artifacts/sorafs_privacy/<relay>.ndjson` セキュリティ
2. 政策予算の補助者:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   観測された比率 پرنٹ کرتی ہے اور ゼロ以外の出口 کرتی ہے 予算が超過 ہو **یا** جب バケット ابھی تیار نہ ہوں، جس سے اشارہ遠隔測定 実行 پیدا نہیں ہوئی۔ライブメトリクス `soranet_privacy_pending_collectors` 排水量 `soranet_privacy_snapshot_suppression_ratio` 予算رہتا ہے جب run چل رہا ہو۔
3. トランスポートのデフォルト設定 JSON 出力 CLI ログ SNNet-8 証拠バンドル アーカイブ レビュー担当者 アーティファクト 保存

## 次のステップ (SNNet-8a)- デュアル Prio コレクターによる統合、共有取り込み、ランタイム、リレー、コレクターの一貫性のある `SoranetPrivacyBucketMetricsV1` ペイロードの放出*(完了 — `crates/sorafs_orchestrator/src/lib.rs` میں `ingest_privacy_payload` اور متعلقہ テスト دیکھیں۔)*
- 共有 Prometheus ダッシュボード JSON アラート ルール、抑制ギャップ、コレクターの健全性、匿名性の停電、カバー*(完了 — `dashboards/grafana/soranet_privacy_metrics.json`、`dashboards/alerts/soranet_privacy_rules.yml`、`dashboards/alerts/soranet_policy_rules.yml` 検証フィクスチャ دیکھیں۔)*
- `privacy_metrics_dp.md` 差分プライバシー キャリブレーション アーティファクト 再現可能なノートブック ガバナンス ダイジェスト*(完了 — ノートブック アーティファクト `scripts/telemetry/run_privacy_dp.py` セキュリティ ラッパー `scripts/telemetry/run_privacy_dp_notebook.sh` ノートブック `.github/workflows/release-pipeline.yml` ワークフロー ガバナンス ダイジェスト`docs/source/status/soranet_privacy_dp_digest.md` میں فائل کیا گیا ہے۔)*

SNNet-8 のリリース SNNet-8 のリリース 決定性、プライバシーに安全なテレメトリ、Prometheus スクレーパー、ダッシュボードفٹ ہوتی ہے۔差分プライバシー キャリブレーション アーティファクト リリース パイプライン ワークフロー ノートブック出力 リリース パイプライン ワークフロー ノートブック出力 自動実行 モニタリング 抑制アラート分析 モニタリングبڑھانے پر مرکوز ہے۔