---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/transition-notes.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
タイトル: Nexus の移行に関する注意事項
説明: `docs/source/nexus_transition_notes.md` の検査、フェーズ B の移行証拠、聴覚カレンダーの確認、および制限。
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus の移行に関する注意事項

**フェーズ B - Nexus 移行基盤** は、マルチレーンのチェックリストの最終段階にあります。 `roadmap.md` のマイルストーンと B1 ～ B4 の証拠参照、SRE と SDK の比較を補完します。

## アルカンセとカデンシア

- ルーティングされたトレースとテレメトリアのガードレール (B1/B2)、および複数のレーン (B4) での安全な設定の調整 (B3)。
- Reemplaza la nota temporal de Cadencia que antes vivia aqui; 2026 年第 1 四半期の聴衆の報告書は、`docs/source/nexus_routed_trace_audit_report_2026q1.md` に保存されており、ページのマンティエンのカレンダーと管理上の記録が含まれています。
- ルートトレース、投票、ランサヨ デ ランザミエントのイベントのタブラスを実行します。あらゆる成果物を保存し、新しいページを参照して、事後ドキュメント (ステータス、ダッシュボード、ポータル SDK) を確認し、安定した状態に保つことができます。

## 証拠のスナップショット (2026 年第 1 四半期～第 2 四半期)

|ワークストリーム |証拠 |所有者 |エスタード |メモ |
|-----------|----------|----------|----------|----------|
| **B1 - ルーティングトレース監査** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops、@governance |完了 (2026 年第 1 四半期) | Tres ventanas de Auditoria registradas; `TRACE-CONFIG-DELTA` の TLS を再実行して、Q2 を再実行します。 |
| **B2 - テレメトリとガードレールの修復** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core、@telemetry-ops |完了 |アラート パック、差分ボットとタマノ OTLP (`nexus.scheduler.headroom` ログ + Grafana ヘッドルームのパネル) をエンビアドします。罪はアビエルトスを放棄する。 |
| **B3 - デルタ構成のプロパシオン** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance |完了 | GOV-2026-03-19 登録に投票します。エルバンドルファームドアリメンタエルパックデテレメトリアシタドアバホ。 |
| **B4 - Ensayo de lanzamiento マルチレーン** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core、@sre-core |コンプリート (2026 年第 2 四半期) |第 2 四半期の TLS のセキュリティを再実行するためのカナリア。バリデータ マニフェスト + `.sha256` スロット 912 ～ 936 のキャプチャ、ワークロード シード `NEXUS-REH-2026Q2` およびハッシュ デル パーフィル TLS レジストラードと再実行。 |

## Calendario trimestal de audiotorias ルートトレース

|トレースID |ベンタナ (UTC) |結果 |メモ |
|----------|--------------|----------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |アプロバド |キュー受付 P95 は、オブジェクトを取得するために必要な時間 <=750 ミリ秒です。アクションは必要ありません。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 |アプロバド | OTLP リプレイ ハッシュは `status.md` に付加されます。 SDK の差分ボットの確認、Cero ドリフトの確認。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 |レスエルト |第 2 四半期の再実行期間中に TLS を再実行します。 `NEXUS-REH-2026Q2` レジストラのテレメトリア パック、TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (バージョン `artifacts/nexus/tls_profile_rollout_2026q2/`) およびセキュリティ セキュリティ。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 |アプロバド |ワークロード シード `NEXUS-REH-2026Q2`; `artifacts/nexus/rehearsals/2026q1/` のテレメトリア + マニフェスト/ダイジェスト (スロット範囲 912 ～ 936) と `artifacts/nexus/rehearsals/2026q2/` のアジェンダをパックします。 |

ロストリメストレスは、実際のトリメストレのデベンアグリガーヌエバスフィラスとムーバーラスエントラダスを完了し、追加の追加を行います。参照情報セクションは、ルーティングされたトレースを報告します。政府機関 `#quarterly-routed-trace-audit-schedule` を参照します。

## バックログの項目を軽減する|アイテム |説明 |オーナー |オブジェクト |エスタド / ノタス |
|------|---------------|----------|----------|-----|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` の期間にわたる TLS の情報伝達ターミナル、再実行および軽減登録のキャプチャ証拠。 | @release-eng、@sre-core | 2026 年第 2 四半期の Ventana ルーテッド トレース | Cerrado - ハッシュ デ パーフィル TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` キャプチャ `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`;再実行確認してください、ヘイ レザガドスはありません。 |
| `TRACE-MULTILANE-CANARY` 準備 | Q2 のプログラマ、補助フィクスチャ、テレメトリ パック、および SDK ハーネスの再利用とヘルパーの検証をサポートします。 | @telemetry-ops、SDK プログラム |プラナシオンのラマダ 2026-04-30 | Completado - スロット/ワークロードのメタデータに関する `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` の議題。トラッカーのハーネスを再利用します。 |
|テレメトリ パック ダイジェスト ローテーション | Ejecutar `scripts/telemetry/validate_nexus_telemetry_pack.py` は、構成デルタのトラッカーを詳細に分析/リリースします。 | @telemetry-ops |リリース候補ごと | Completado - `telemetry_manifest.json` + `.sha256` は `artifacts/nexus/rehearsals/2026q1/` で出力されます (スロット範囲 `912-936`、シード `NEXUS-REH-2026Q2`)。コピアドとトラッカーと証拠のインデックスをダイジェストします。 |

## バンドルと構成デルタの統合

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 差分ファイルを再開します。クアンド・イレグエン・ヌエボス `defaults/nexus/*.toml` は、創世記のカンビオス、実際のトラッカー プリメロとルエゴ リフレハ ロス プントス クラーベ アキです。
- テレメトリア デル パックの署名付き設定バンドルが失われています。 El パック、`scripts/telemetry/validate_nexus_telemetry_pack.py` の検証、設定デルタ パラケ ロス オペラドールのプエダン再現ロス アーティファクト 正確な米国デュランテ B4 の公開情報。
- Iroha 2 つのレーンのバンドルのロス: `nexus.enabled = false` での構成のロス: `nexus.enabled = false` アホラ レチャザンは、レーン/データスペース/ルーティングのメノス クエリ パーフィル Nexus エステ ハビリタド (`--sora`)、同様のエリミナをオーバーライドします。 las secciones `nexus.*` de las plantillas シングルレーン。
- Manten el registro del voto de gobernanza (GOV-2026-03-19) は、不正行為の儀式の記録を保持するために、追跡者とその追跡者を追跡します。

## セギミエントス デル エンサヨ デ ランザミエント

- `docs/source/runbooks/nexus_multilane_rehearsal.md` カナリア計画のキャプチャ、参加者のリストとロールバックの記録。実際のランブックは、レーンやテレメトリの輸出業者のトポロジ情報を提供します。
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 4 月 9 日の最新情報リストには、Q2 の準備に関する注記/議題が含まれています。証拠となるモノトナを管理するための追跡情報を収集します。
- Grafana (バージョン `docs/source/telemetry.md`) のバッチ処理デル エクスポータで、OTLP およびエクスポートの収集者からスニペットを公開します。第 1 四半期の実際のバッチ サイズは 256 件のヘッドルーム アラートです。
- `integration_tests/tests/nexus/multilane_pipeline.rs` での CI/テスト マルチレーン アホラ ライブ、ワークフロー `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)、参照 `pytests/nexus/test_multilane_pipeline.py`; `defaults/nexus/config.toml` (`nexus.enabled = true`、blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) のハッシュ パラメータと同期コントロール トラッカー、および参照バンドルを保持します。

## 実行時にレーンの動きを確認する

- ランタイムでの車線の損失、バリダン、データスペースのバインディング、クラ/アルマセナミエントのニベレスでの和解、カンビオスのデジャンドとの和解。ロスヘルパーは、レーンとキャッシュパラレーンのレティラダをリレーし、モードケラシンテシスのマージ台帳を再利用して証明を廃止します。
- アプリケーション プレーンは、Nexus (`State::apply_lane_lifecycle`、`Queue::apply_lane_lifecycle`) パラアグリガー/リティア レーンの設定/ライフサイクル ヘルパーを実行します。ルーティング、スナップショット TEU およびレジストリのマニフェストは、計画終了時に自動的に再収集されます。
- ギア パラ オペラドール: 計画フォールヤの作成、データスペースのファルタンテスの改訂、ストレージ ルートのプエダン クレアス (階層化されたコールド ルート/ディレクター クラ ポー レーン)。基本的な再意図を確認してください。飛行機の出口は、レーン/データスペース パラケロス ダッシュボード リフレジェン ラ ヌエバ トポロジアでテレメトリの差分を再送信します。

## Telemetria NPoS とバックプレッシャーの証拠フェーズ B のレトロなデータ キャプチャとテレメトリの決定機能、ペースメーカーの NPoS とゴシップのラス キャップ、背圧の限界を確認します。 `integration_tests/tests/sumeragi_npos_performance.rs` を統合して、シナリオを実行し、履歴書 JSON (`sumeragi_baseline_summary::<scenario>::...`) を確認して、新しいメトリクスを生成します。エヘクタロ・ローカルメント・コン:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`、`SUMERAGI_NPOS_STRESS_COLLECTORS_K`、`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` の市長のトポロジを探索するための設定。ロス バロレス ポル ディフェクト リフレジャン エル パーフィル デ レコレクタレス 1 s/`k=3` usado en B4。

|シナリオ/テスト |コベルチュラ |テレメトリア・クラーベ |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | EMA の遅延とレジストラのエンベロープ、シリアル化と証拠のバンドルの冗長送信アンテの詳細なゲージのブロック時間の 12 個のブロック。 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |コーラは、容量/飽和状態のコーラを輸出するために、正式に承認され、有効な承認を取得する必要があります。 | `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` |ペースメーカーとタイムアウトのタイムアウトを確認し、デモストラル ケ ラ バンダ +/-125 パーミルとアプリケーションを確認してください。 | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` | Empuja ペイロードは、RBC グランデス ハスタ ロス リミットのソフト/ハード デル ストア パラ最も多くのセッションやコンタドール デ バイトのサブエン、レトロセデン アンド セスタビリザン シン ソブレパサール ストアを保存します。 | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |冗長な比率でゲージを再送信し、コレクタ オン ターゲットのアバンセンを送信し、エンドツーエンドでのレトロな接続をデモストランドで確認できます。 | `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Descarta チャンクは、ペイロードの期間を確認して、バックログの監視期間を決定します。 | `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |

補助的なラインは、Prometheus での Prometheus のインプリメ エル ハーネスの制御と、バックプレッシャーの同時発生による安全性の証拠の取得を要求します。

## 現実化のチェックリスト

1. アグレガ ヌエバス ベンタナス ルートトレースとレティラ ラス アンティグアス クアンド ロテン ロス トリメストレス。
2. アラート マネージャーの安全性を確認し、チケットの確認も含めます。
3. 設定デルタ、実際のトラッカー、テレメトリ パックのダイジェスト リスト、ミスモ プル リクエストの記録。
4. アドホック分散に関する単独の文書と、ロードマップの実際のロードマップに関する、エンサヨ/テレメトリアの最新の成果物を確認します。

## 証拠のインデックス|アクティボ |ユビカシオン |メモ |
|------|----------|------|
|監査ルートトレースレポート (2026 年第 1 四半期) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` |フェーズ B1 での証拠の確定； Reflejado パラ エル ポータル en `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`。 |
|トラッカーの構成デルタ | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA の差分履歴、GOV-2026-03-19 のログの改訂履歴を確認してください。 |
|テレメトリの修復計画 | `docs/source/nexus_telemetry_remediation_plan.md` | B2 の輸出用のガードレールや、OTLP のタマノ ドキュメントなどを記録します。 |
|トラッカー デ エンサヨ マルチレーン | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 4 月 9 日の資料リスト、検証ツールのマニフェスト/ダイジェスト、第 2 四半期のメモ/議題とロールバックの証拠。 |
|テレメトリ パックのマニフェスト/ダイジェスト (最新) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) |レジストラ スロット範囲 912 ～ 936、シード `NEXUS-REH-2026Q2` y ハッシュ、バンドルのアーティファクト。 |
| TLS プロファイル マニフェスト | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |第 2 四半期の再実行時に TLS をキャプチャーするためのハッシュ。 Routed-Trace の付録が付属しています。 |
|アジェンダ TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` |第 2 四半期の計画に関する注意事項 (ベンタナ、スロット範囲、ワークロード シード、所有者のアクション)。 |
|ランザミエントのランブック | `docs/source/runbooks/nexus_multilane_rehearsal.md` |チェックリスト操作パラステージング -> 排出 -> ロールバック;実際のクアンド・カンビー・ラ・トポロジー・デ・レーン・オ・ラ・ギア・デ・エクスポーター。 |
|テレメトリ パック バリデータ | `scripts/telemetry/validate_nexus_telemetry_pack.py` |レトロ B4 の CLI リファレンス。アーカイブは、ジュント アル トラッカー クアンド エル パック カンビアをダイジェストします。 |
|マルチレーン回帰 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prueba `nexus.enabled = true` はマルチレーンを構成し、`ConfigLaneRouter` を介して、公開ダイジェストの成果物を公開する前に、レーン (`blocks/lane_{id:03}_{slug}`) のクラ/マージログをプロビジョニングしてカタログのハッシュを保存します。 |