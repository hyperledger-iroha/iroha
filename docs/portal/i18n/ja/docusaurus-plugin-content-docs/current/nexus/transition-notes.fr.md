---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
title: Nexus の移行ノート
説明: Miroir de `docs/source/nexus_transition_notes.md`、移行フェーズ B のクーブラント、オーディットのカレンダーと緩和策。
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus の移行に関するメモ

**フェーズ B - Nexus 移行財団** のジャーナル スーツは、マルチレーンの最終チェックリストを確認します。 `roadmap.md` および `roadmap.md` のマイルストーンの完全なレファレンスは、B1 ～ B4 の詳細なエンドロイトを参照し、SRE および SDK がミーム ソースの真の部分をリードします。

## ポーティーとリズム

- ルーテッド トレースとテレメトリのガードレールを監査 (B1/B2)、管理者によるデルタ構成の承認 (B3)、およびマルチ レーンの繰り返しの監視 (B4)。
- 生き生きとリズムを刻む音を置き換えます。 2026 年第 1 四半期監査、`docs/source/nexus_routed_trace_audit_report_2026q1.md` に関する詳細な報告書、カレンダー管理および軽減措置の登録ページの管理。
- メッテズ・ア・ジュール・レ・テーブル・アプレ・チャク・フェネトル・ルーテッド・トレース、投票による統治、または反復による実行。アーティファクトのブーゲントを取得し、安定した状態でドキュメントのページを作成し、ページを作成します (ステータス、ダッシュボード、Portails SDK)。

## スナップショット デ プリューブ (2026 年第 1 四半期～第 2 四半期)

|ワークストリーム |プルーヴ |所有者 |法令 |メモ |
|-----------|----------|----------|----------|----------|
| **B1 - ルーティングトレース監査** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops、@governance |完了 (2026 年第 1 四半期) | Trois fenetres d'audit の登録者。ル・リタード TLS デ `TRACE-CONFIG-DELTA` s'est clos ペンダント ル・リラン Q2。 |
| **B2 - テレメトリとガードレールの修復** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core、@telemetry-ops |完了 |アラート パック、差分ボットと詳細 OTLP (`nexus.scheduler.headroom` ログ + パネル Grafana ヘッドルーム) のポリシー。オークン免除オーバーバート。 |
| **B3 - デルタ構成の承認** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance |完了 | GOV-2026-03-19 登録に投票します。バンドル署名、テレメトリのパック、およびベース。 |
| **B4 - マルチレーンの繰り返し** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core、@sre-core |完了 (2026 年第 2 四半期) | TLS の遅延を緩和するために Q2 のカナリアを再実行します。ファイル バリデーター マニフェスト + `.sha256` スロット 912 ～ 936 のプラージュ、ワークロード シード `NEXUS-REH-2026Q2` をキャプチャし、プロファイルのハッシュ TLS 登録ペンダント ファイルを再実行します。 |

## ルーティングトレース監査のカランドリエ トリメストリエル

|トレースID |フェネトレ (UTC) |結果 |メモ |
|----------|--------------|----------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |ロイッシ |キュー許可 P95 の最大遅延時間は 750 ミリ秒以下です。オーキューンアクションの要求。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 |ロイッシ | OTLP リプレイ ハッシュには `status.md` が付加されます。差分ボット SDK のパリテはゼロ ドリフトを確認します。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 |リソル | TLS の最も長い時間をかけて Q2 を再実行します。 `NEXUS-REH-2026Q2` のテレメトリー パックは、TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/`) のハッシュ デュ プロファイルを登録し、遅延をゼロにします。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 |ロイッシ |ワークロード シード `NEXUS-REH-2026Q2`; `artifacts/nexus/rehearsals/2026q1/` によるテレメトリ + マニフェスト/ダイジェストのパック (スロット範囲 912 ～ 936) `artifacts/nexus/rehearsals/2026q2/` によるアベック アジェンダ。 |

トリメストルの将来は、新しいリニュスとデプレイサの終端駅と別館とテーブルのデパスル トリメストル クーラントを管理します。参照セクションのルートトレース報告書は、`#quarterly-routed-trace-audit-schedule` を使用した統治の議事録です。

## バックログの緩和策と項目|アイテム |説明 |オーナー |聖書 |法令・注意事項 |
|------|---------------|----------|----------|-----|
| `NEXUS-421` |ターミネーターはプロファイル TLS の伝播を制御し、ペンダント `TRACE-CONFIG-DELTA`、キャプチャー les preuves du rerun et fermer le registre de mitigation を提供します。 | @release-eng、@sre-core | Fenetre ルーテッド トレース 2026 年第 2 四半期 | Clos - プロファイル TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` キャプチャと `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` のハッシュ。遅延ゼロの確認を再実行します。 |
| `TRACE-MULTILANE-CANARY` 準備 |プログラマは Q2 を繰り返し、テレメトリ パックなどのフィクスチャに参加し、SDK ハーネスを再利用してヘルパーを有効にします。 | @telemetry-ops、SDK プログラム |計画に対する異議申し立て 2026-04-30 |完了 - `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` の平均メタデータ スロット/ワークロードのアジェンダ ストック。トラッカーによるハーネスの再利用。 |
|テレメトリ パック ダイジェスト ローテーション | Executer `scripts/telemetry/validate_nexus_telemetry_pack.py` アバント チャクの繰り返し/リリースおよび登録者は、構成デルタのトラッカーをダイジェストします。 | @telemetry-ops |パーリリース候補 |完了 - `telemetry_manifest.json` + `.sha256` および `artifacts/nexus/rehearsals/2026q1/` (スロット範囲 `912-936`、シード `NEXUS-REH-2026Q2`)。トラッカーおよびインデックス・デ・プルーブのダイジェスト・コピー。 |

## バンドル構成デルタの統合

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 差分のカノニクを再開します。 Quand de nouveaux `defaults/nexus/*.toml` の変更点が到着し、定期的なトラッカーを確認して、ポイントを確認します。
- 署名された構成バンドルには、繰り返しのテレメトリ パックが含まれています。 Le Pack、valide par `scripts/telemetry/validate_nexus_telemetry_pack.py`、doit etre publie avec les preuves de config delta afin que les Operators puissent rejouer les artefacts Exacts はペンダント B4 を使用します。
- バンドル Iroha レーンなしの 2 つの保持ファイル: 構成ファイル avec `nexus.enabled = false` 保持保守ファイルは、レーン/データスペース/ルーティングのファイル プロファイルをオーバーライドします。 Nexus est active (`--sora`)、doncシングルレーンのテンプレートセクション `nexus.*` をサポートしています。
- Gardez le log de vote de gouvernance (GOV-2026-03-19) 嘘をつき、追跡者や将来の投票を注ぐメモを作成し、コピー機の形式で再承認する必要はありません。

## 繰り返しのランスメント

- `docs/source/runbooks/nexus_multilane_rehearsal.md` カナリア計画のキャプチャ、参加者のリストとロールバックの記録。 1 時間ごとにランブックを作成し、レーンや輸出業者のテレメトリー変更のトポロジーを確認します。
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` リスト チャク アーティファクトは、9 月 9 日の繰り返しと内容のメンテナンスに関するメモ/議題の準備を確認します。 Q2. Ajoutez les repetitions futures au meme tracker au lieu d'ouvrir des trackers アドホック プール ガーダー les preuves モノトーン。
- 収集者 OTLP の抜粋と輸出 Grafana (`docs/source/telemetry.md`) の輸出者変更のバッチ処理の委託品の発行。 la misse a jour Q1 a porte la taille de Lot a 256 echantillons pour eviter des alertes ヘッドルーム。
- Les preuves CI/tests multi-lane vivent maintenant dans `integration_tests/tests/nexus/multilane_pipeline.rs` et tournent sous le workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)、remplicant la Reference retee `pytests/nexus/test_multilane_pipeline.py`; Gardez le hash pour `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) と繰り返しのバンドルを同期するためのトラッカーです。

## レーンのサイクル・ド・ヴィー・ランタイム

- ランタイムの有効なメンテナンスのサイクルとレーンの計画、データスペースのバインディングおよび中止の調整、および蔵/在庫とパリエのエコー、カタログ変更のレザン。ヘルパーはリレーキャッシュを削除し、レーンを削除し、マージ台帳を合成し、時代遅れになったパスデプルーフを再利用します。
- 構成/ライフサイクルのヘルパー Nexus (`State::apply_lane_lifecycle`、`Queue::apply_lane_lifecycle`) を介して計画をアップリケし、再婚せずにレーンを移動/退職者に提供します。ルーティング、スナップショット TEU およびレジストリのマニフェスト、再充電の自動化計画後の再充電。
- ガイド オペレーター: エコーを計画し、データスペースのマンカントとストレージ ルートを確認します (階層化されたコールド ルート/レパートリー クラ パー レーン)。 Corrigez les chemins de Base et reessayez;レウシスは、テレメトリ レーン/データスペースの差分を再確認して、新しいトポロジーを反映したダッシュボードを計画します。

## Telemetry NPoS とバックプレッシャーの実行フェーズ B では、ペースメーカーの NPoS と、背圧の限界を確認するためのゴシップとソファを確認するために、テレメトリのキャプチャとテレメトリの決定を要求します。 `integration_tests/tests/sumeragi_npos_performance.rs` の演習シナリオと再開 JSON (`sumeragi_baseline_summary::<scenario>::...`) の新しいメトリクスを統合して利用します。ランセズルのロケール平均:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`、`SUMERAGI_NPOS_STRESS_COLLECTORS_K` または `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` は、トポロジの探索者とストレス対象者を定義します。 les valeurs par defaut refletent le profil decollecteurs 1 s/`k=3` は B4 を使用します。

|シナリオ/テスト |クーベルチュール |テレメトリ |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` |ブロック 12 ラウンドのブロック時間は、遅延 EMA を登録するためのブロック時間を繰り返し、ファイルのプロフォンデュールとゲージの冗長送信前シリアライザーのバンドルを実行します。 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |トランザクションのファイルは、承認の延期や承認の決定、および容量/飽和のコンピューティングのファイルのエクスポートを保証します。 | `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` | Echantillonne le jitter du ペースメーカーと les timeouts de vue jusqu'a prouver que labande +/-125 permille est appliquee。 | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` |総ペイロードの RBC jusqu'aux は、ストアでのセッションやバイト数の計算、デパッサー ル ストアなしでの安定性、ソフト/ハードの制限を制限します。 | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |再送信は、冗長性と送信率のゲージを強制的に実行し、コレクター オン ターゲットの収集者を事前に監視し、レトロな分岐先のエンドツーエンドでテレメトリ要求者を監視します。 | `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` |チャンクの間隔を決定して、検証者がバックログの監視を行い、ペイロードの遅延を通知します。 | `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |

Joignez les lignes JSON は、Prometheus のハーネス アベニューのスクレイピングを暗黙的に監視し、ペンダントの実行を監視し、警察の要求を監視し、背圧特派員を繰り返して監視します。

## 1 時間のチェックリスト

1. Ajoutez de nouvelles fenetres ルートトレースとリタイア レ アンシエンヌ ロルスク レ トリメストレス トーナメント。
2. アラートマネージャーによる、監視後の緩和策の計画は、チケットから構成されます。
3. 構成デルタ変更、時間追跡トラッカー、メモおよびテレメトリ パックのダイジェスト リスト、プル リクエストの収集。
4. 将来の計画は、定期的なロードマップの定期的な参照を必要とせず、文書の追加メモとしてアドホックに分散されます。

## インデックス・デ・プルーブ|アクティフ |定置 |メモ |
|------|----------|------|
| Rapport d'audit ルーテッド トレース (2026 年第 1 四半期) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` |ソース カノニク プール レ プルーヴ フェーズ B1。ミロワール プール ル ポルテイル スー `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`。 |
|トラッカーの構成デルタ | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |コンテンツは、TRACE-CONFIG-DELTA の差分の再開、レビュー担当者のイニシャル、および GOV-2026-03-19 の投票ログです。 |
|テレメトリの修復計画 | `docs/source/nexus_telemetry_remediation_plan.md` | B2 には、アラート パック、OTLP および輸出予算のガードレールに関する文書が記載されています。 |
|トラッカー・デ・レペティション・マルチレーン | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` |成果物リストは、4 月 9 日の繰り返し、マニフェスト/ダイジェスト検証ツール、メモ/議題第 2 四半期以降のロールバックの期間を検証します。 |
|テレメトリ パックのマニフェスト/ダイジェスト (最新) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Enregistre la plage 912-936、seed `NEXUS-REH-2026Q2` et les hashes d'artefacts pour les Bundle de gouvernance。 |
| TLS プロファイル マニフェスト | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |ハッシュ デュ プロファイル TLS はキャプチャ ペンダントを承認し、Q2 を再実行します。 citez-le dans les annexes ルートトレース。 |
|アジェンダ TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 の繰り返しによる計画のメモ (フェネトレ、スロット範囲、ワークロード シード、所有者のアクション)。 |
|繰り返しのランブック | `docs/source/runbooks/nexus_multilane_rehearsal.md` |ステージング -> 実行 -> ロールバックのチェックリスト操作。 1 時間ごとにトポロジーのレーンとコンセイユの輸出業者の変更を監視します。 |
|テレメトリ パック バリデータ | `scripts/telemetry/validate_nexus_telemetry_pack.py` |レトロ B4 に関する CLI リファレンス。 archivez les ダイジェスト、avec le tracker、chaque fois que le パックの変更。 |
|マルチレーン回帰 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | `nexus.enabled = true` はマルチレーンの構成を有効にし、カタログのハッシュとプロビジョニングされた化学物質の蔵/マージログパーレーン (`blocks/lane_{id:03}_{slug}`) を `ConfigLaneRouter` 経由で保持し、成果物を要約します。 |