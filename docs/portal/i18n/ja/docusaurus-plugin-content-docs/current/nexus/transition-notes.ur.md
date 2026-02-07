---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
title: Nexus ٹرانزیشن نوٹس
説明: `docs/source/nexus_transition_notes.md` フェーズ B のフェーズ B のテスト。 ❁❁❁❁
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

#Nexus ٹرانزیشنوٹس

**フェーズ B - Nexus 移行基盤** 開発中のマルチレーンの開発سٹ مکمل نہ ہو جائے۔ یہ `roadmap.md` میں マイルストーン اندراجات کی تکمیل کرتا ہے اور B1-B4 میں حوالہ دی گئی 証拠 کو ایک جگہガバナンス SRE SDK 真実のソース 真実のソース

## リズム

- ルーテッド トレース テレメトリ ガードレール (B1/B2) ガバナンス 設定デルタ (B3) マルチレーン打ち上げリハーサル フォローアップ (B4)
- یہ عارضی cadence نوٹ کی جگہ لیتا ہے جو پہلے یہاں تھا؛ 2026 年第 1 四半期のセキュリティ対策جسٹر رکھتا ہے۔
- ルートトレース ガバナンス 打ち上げリハーサル 打ち上げリハーサルアーティファクトの管理 ダウンストリーム ドキュメント (ステータス、ダッシュボード、SDK ポータル) 管理アンカー سے لنک کر سکیں۔

## 証拠のスナップショット (2026 年第 1 四半期～第 2 四半期)

| और देखें और देखें意味 | और देखेंにゅう |
|-----------|----------|----------|----------|----------|
| **B1 - ルーティングトレース監査** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops、@governance |重要 (2026 年第 1 四半期) | آڈٹ ونڈوز ریکارڈ ہوئیں؛ `TRACE-CONFIG-DELTA` TLS ラグ Q2 再実行 بند ہوا۔ |
| **B2 - テレメトリ修復とガードレール** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core、@telemetry-ops | |アラート パック、差分ボット ポリシー、OTLP バッチ サイズ (`nexus.scheduler.headroom` ログ + Grafana ヘッドルーム パネル) ٩وئی واورز اوپن نہیں۔ |
| **B3 - 構成デルタの承認** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance | | GOV-2026-03-19 ووٹ ریکارڈ ہے؛署名済みバンドル テレメトリー パック フィード ہے۔ |
| **B4 - マルチレーン発射リハーサル** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core、@sre-core |重要 (2026 年第 2 四半期) |第 2 四半期の Canary 再実行による TLS ラグの軽減バリデータ マニフェスト + `.sha256` スロット 912-936 ワークロード シード `NEXUS-REH-2026Q2` 再実行 TLS プロファイル ハッシュ キャプチャ キャプチャ|

## ルーテッドトレース شیڈول

|トレースID | (UTC) |ナタリー |にゅう |
|----------|--------------|----------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |やあ |キュー入場 P95 ہدف <=750 ms سے کافی نیچے رہا۔ ٩وئی ایکشن درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 |やあ | OTLP リプレイ ハッシュ `status.md` 説明SDK の差分ボット パリティ ゼロ ドリフト テスト|
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | और देखें TLS ラグ Q2 再実行`NEXUS-REH-2026Q2` テレメトリ パック میں TLS プロファイル ハッシュ `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ریکارڈ ہے (دیکھیں `artifacts/nexus/tls_profile_rollout_2026q2/`) اور صفر stragglers ہیں۔ |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 |やあ |ワークロード シード `NEXUS-REH-2026Q2`;テレメトリ パック + マニフェスト/ダイジェスト `artifacts/nexus/rehearsals/2026q1/` میں (スロット範囲 912-936) اور 議題 `artifacts/nexus/rehearsals/2026q2/` میں ہے۔ |

آنے الے سہ ماہیوں میں نئی قطاریں شامل کریں اور جب ٹیبل موجودہ سہ ماہی سے بڑا ہو جائے تو مکمل اندراجات کو 付録 میں منتقل کریں۔ Routed-trace ガバナンス議事録 `#quarterly-routed-trace-audit-schedule` アンカー ذریعے ریفرنس کریں۔

## 緩和策とバックログ項目| और देखें評価 |意味 | | और देखें
|------|---------------|----------|----------|-----|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` TLS プロファイル、伝播、再実行、証拠キャプチャ、軽減ログ、TLS プロファイルありがとう| @release-eng、@sre-core | 2026 年第 2 四半期のルーテッド トレース | - TLS プロファイル ハッシュ `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` キャプチャ キャプチャ再放送 ゼロのはぐれ者 کی تصدیق کی۔ |
| `TRACE-MULTILANE-CANARY` 準備 | Q2 リハーサルの確認 テレメトリ パックの確認 フィクスチャの確認 確認の確認 SDK ハーネスによるヘルパーの再利用の検証ありがとう| @telemetry-ops、SDK プログラム | 2026-04-30 計画電話 | مکمل - アジェンダ `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` میں محفوظ ہے جس میں スロット/ワークロード メタデータ شامل ہے؛ハーネス再利用トラッカー میں نوٹ ہے۔ |
|テレメトリ パック ダイジェスト ローテーション |リハーサル/リリース `scripts/telemetry/validate_nexus_telemetry_pack.py` ダイジェスト 設定 デルタ トラッカー 設定デルタ トラッカー| @telemetry-ops | ⑥ リリース候補 | - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں جاری ہوئے (スロット範囲 `912-936`、シード `NEXUS-REH-2026Q2`);ダイジェスト トラッカー 証拠インデックス میں کاپی کیے گئے۔ |

## 構成デルタバンドルの統合

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 正規の差分概要 ہے۔ جب نئے `defaults/nexus/*.toml` یا ジェネシス変更 آئیں تو پہلے tracker اپ ڈیٹ کریں اور پھر خلاصہ یہاں شامل کریں۔
- 署名付き構成バンドル リハーサル テレメトリー パックパック `scripts/telemetry/validate_nexus_telemetry_pack.py` 検証 構成デルタ証拠 公開 オペレータ B4 演算子 B4アーティファクトをリプレイする
- Iroha 2 バンドル レーンの説明: `nexus.enabled = false` レーン/データスペース/ルーティングのオーバーライド構成、拒否、拒否Nexus プロファイル (`--sora`) は、単一レーン テンプレートを有効にします。 `nexus.*` セクションは、セクションを有効にします。
- ガバナンス投票ログ (GOV-2026-03-19) 評価トラッカー 評価 評価 評価 評価 投票数 評価دوبارہ دریافت کیے بغیر استعمال کر سکیں۔

## 打ち上げリハーサルのフォローアップ

- `docs/source/runbooks/nexus_multilane_rehearsal.md` Canary 参加者名簿とロールバック手順を確認してくださいレーン トポロジ、テレメトリ エクスポータ、ランブック、およびランブック
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 リハーサル、成果物、Q2 準備メモ/議題、Q2 準備メモ/議題リハーサル、トラッカー、証拠、単調な証拠
- OTLP コレクター スニペット Grafana エクスポート (`docs/source/telemetry.md`) およびエクスポート バッチ ガイダンスの公開第 1 四半期更新のヘッドルーム アラート、バッチ サイズ、サンプル数 256 個のヘッドルーム アラート
- マルチレーン CI/テスト証拠 `integration_tests/tests/nexus/multilane_pipeline.rs` ワークフロー `Nexus Multilane Pipeline` ワークフロー (`.github/workflows/integration_tests_multilane.yml`) 評価 `pytests/nexus/test_multilane_pipeline.py` और देखें `defaults/nexus/config.toml` ハッシュ (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) トラッカー、同期、リハーサル バンドル、リハーサル バンドル

## ランタイムレーンのライフサイクル

- ランタイム レーンのライフサイクル プラン、データスペース バインディングの検証、クラ/階層型ストレージ調整の失敗、中止、カタログ、カタログの作成。さいとうヘルパー レーン キャッシュされたリレー プルーン プルーン マージ元帳合成 証明の再利用
- Nexus 構成/ライフサイクル ヘルパー (`State::apply_lane_lifecycle`、`Queue::apply_lane_lifecycle`) プランの適用 レーンの再起動 追加/廃止ああルーティング TEU スナップショット マニフェスト レジストリ 計画 計画 リロード ہوتے ہیں۔
- オペレーターの説明: 計画の失敗、データスペースの欠落、ストレージ ルート、接続、および (階層型コールド ルート/クラ レーン ディレクトリ)ベースパス درست الدوبارہ کوشش کریں؛レーン/データスペース テレメトリ差分計画 レーン/データスペース テレメトリ差分 放出量 ダッシュボード ダッシュボード トポロジ 計画 レーン/データスペース テレメトリ差分

## NPoS テレメトリとバックプレッシャーの証拠

フェーズ B の打ち上げリハーサルのレトロな決定論的テレメトリ キャプチャ セキュリティ セキュリティ NPoS ペースメーカー ゴシップ層 背圧 حدود میں رہتے ہیں۔ `integration_tests/tests/sumeragi_npos_performance.rs` 統合ハーネス、シナリオ、メトリクス、JSON サマリー (`sumeragi_baseline_summary::<scenario>::...`) の出力説明:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`、`SUMERAGI_NPOS_STRESS_COLLECTORS_K` یا `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` سیٹ کریں تاکہ زیادہ ストレス والی トポロジ دیکھ سکیں؛デフォルト値 B4 میں استعمال ہونے والے 1 s/`k=3` コレクタ پروفائل کو 反映 کرتے ہیں۔|シナリオ/テスト |取材範囲 |主要なテレメトリー |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` |リハーサルブロック時間 12 ラウンド 12 ラウンド EMA レイテンシ エンベロープ キュー深さ 冗長送信ゲージ 証拠バンドル シリアル化 6 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |トランザクション キュー 承認 承認の延期 決定的にトリガーされる キューの容量/飽和カウンターのエクスポート 承認の延期| `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` |ペースメーカー ジッター タイムアウト サンプルの表示 ٩رتا ہے جب تک +/-125 permille بینڈ نافذ ہونے کا ثبوت نہ ملے۔ | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` | RBC ペイロード、ストア、ソフト/ハード、プッシュ セッション、バイト カウンター、オーバーフロー、安定化فونا دکھایا جا سکے۔ | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |力の再送信 冗長送信率ゲージ コレクタ オン ターゲット カウンター セキュリティ レトロ テレメトリ エンドツーエンド テレメトリ エンドツーエンド テレメトリ エンドツーエンド テレメトリ| `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` |決定的に間隔をあけたチャンクをドロップする バックログを監視する バックログを監視する 排水する フォルトが発生する| `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |

ガバナンス ガバナンス 背圧アラーム リハーサル トポロジ 一致 ハーネス ハーネス JSON 行Prometheus スクレイピング بھی منسلکریں۔

## チェックリストを更新する

1. Routed-trace のルーティングトレースの実行
2. Alertmanager のフォローアップ、緩和テーブル、アクション チケット、アクション チケット、およびアクション チケット。
3. 設定デルタ、トラッカー、テレメトリ パック ダイジェスト、プル リクエスト、プル リクエスト
4. リハーサル/テレメトリ アーティファクトの確認とロードマップの更新の確認ریفرنس کریں، بکھری ہوئی アドホック نوٹس نہیں۔

## 証拠インデックス

|認証済み |大事 |にゅう |
|------|----------|------|
|ルーティングトレース監査レポート (2026 年第 1 四半期) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` |フェーズ B1 証拠と正規情報源پورٹل پر `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` میں ミラー ہے۔ |
|構成デルタ トラッカー | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA の差分概要、レビュー担当者、イニシャル、GOV-2026-03-19 投票ログ、データ|
|テレメトリ修復計画 | `docs/source/nexus_telemetry_remediation_plan.md` |アラート パック、OTLP バッチ サイズ、B2、輸出予算のガードレール、 ہے۔ |
|マルチレーンリハーサルトラッカー | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 リハーサル成果物、バリデータマニフェスト/ダイジェスト、第 2 四半期メモ/議題、ロールバック証拠|
|テレメトリ パックのマニフェスト/ダイジェスト (最新) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) |スロット範囲 912 ～ 936 シード `NEXUS-REH-2026Q2` ガバナンス バンドル アーティファクト ハッシュ|
| TLS プロファイル マニフェスト | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |第 2 四半期の再実行、キャプチャ、TLS プロファイル ハッシュRouted-Trace の付録を参照|
| TRACE-MULTILANE-CANARY アジェンダ | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` |第 2 四半期リハーサル計画 (スロット範囲、ワークロード シード、アクション オーナー) |
|打ち上げリハーサル ランブック | `docs/source/runbooks/nexus_multilane_rehearsal.md` |ステージング -> 実行 -> ロールバックのチェックリストレーン トポロジー 輸出業者のガイダンス بدلنے پر اپ ڈیٹ کریں۔ |
|テレメトリ パック バリデータ | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 レトロ میں حوالہ دیا گیا CLI؛パック、ダイジェスト、トラッカー、トラック、トラック、トラック、トラック、トラック|
|マルチレーン回帰 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` |マルチレーン構成 `nexus.enabled = true` セキュリティ ソラ カタログ ハッシュ セキュリティ `ConfigLaneRouter` レーンローカルKura/merge-log パス (`blocks/lane_{id:03}_{slug}`) アーティファクト ダイジェストのプロビジョニング|