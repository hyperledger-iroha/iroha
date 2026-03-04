---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/transition-notes.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
title: Notas de transicao do Nexus
説明: Espelho de `docs/source/nexus_transition_notes.md`、フェーズ B の移行証拠、公聴会としてのカレンダー。
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus の転送に関する注意事項

**フェーズ B - Nexus 移行基盤** は、マルチレーン ターミナルのチェックリストに従って登録を行っています。 `roadmap.md` のマイルストーンとしての要素の補完は、B1 ～ B4 の政府機関向けの標準的な証拠参照、SRE の SDK コンパートメントおよび管理上の証拠の管理を管理します。

## エスコポとカデンシア

- テレメトリアのルーテッド トレース、OS ガードレール (B1/B2)、政府管理用のデルタ構成 (B3)、マルチ レーンのサポート (B4) として使用されます。
- 生き生きとした生活を一時的に置き換えます。 2026 年第 1 四半期のオーディオ関連の情報は、`docs/source/nexus_routed_trace_audit_report_2026q1.md` に保存されており、ページ管理、カレンダーの修正、および記録の記録が行われます。
- tabela apos cada janela Routed-Trace、voto de Governmentca ou ensaio de lancamento として実現します。最新のローカル情報 (ステータス、ダッシュボード、ポータル SDK) を参照して、最新のローカル情報を確認できます。

## 証拠のスナップショット (2026 年第 1 四半期～第 2 四半期)

|ワークストリーム |証拠 |所有者 |ステータス |メモ |
|-----------|----------|----------|----------|----------|
| **B1 - ルーティングトレース監査** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops、@governance |完了 (2026 年第 1 四半期) |聴衆登録簿。 `TRACE-CONFIG-DELTA` で TLS を実行し、デュランテを実行して Q2 を再実行します。 |
| **B2 - テレメトリとガードレールの対策** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core、@telemetry-ops |完了 |アラート パック、政治的差異ボットとロット OTLP (`nexus.scheduler.headroom` ログ + ペイン Grafana デ ヘッドルーム) がエンビアドス。 sem は em aberto を放棄します。 |
| **B3 - デルタ設定のアプリケーション** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance |完了 | GOV-2026-03-19 登録に投票します。 o バンドル・アッシナド・アリメンタ o パック・デ・テレメトリア・シタド・アバイショ。 |
| **B4 - Ensaio de lancamento マルチレーン** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core、@sre-core |コンプリート (2026 年第 2 四半期) | O TLS を実行して第 2 四半期のカナリアを再実行します。 o バリデータ マニフェスト + `.sha256` キャプチャ o スロット 912 ～ 936 の間隔、ワークロード シード `NEXUS-REH-2026Q2` e o ハッシュ実行 perfil TLS レジストラード、再実行なし。 |

## Calendario trimestal de audiotorias ルートトレース

|トレースID |ジャネラ (協定世界時) |結果 |メモ |
|----------|--------------|----------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |アプロヴァド |キュー許可 P95 は 750 ミリ秒以下です。ネンフマ・アカオ・ネセサリア。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 |アプロヴァド | OTLP リプレイ ハッシュは `status.md` に属します。 SDK の差分ボットを確認してゼロ ドリフトを確認します。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 |リゾルビド | Q2 で TLS をフェチャド デュランテまたは再実行します。 `NEXUS-REH-2026Q2` レジストラのテレメトリア パック、TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (バージョン `artifacts/nexus/tls_profile_rollout_2026q2/`) のゼロ アトラサードを実行するハッシュ。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 |アプロヴァド |ワークロード シード `NEXUS-REH-2026Q2`;テレメトリア パック + マニフェスト/ダイジェスト em `artifacts/nexus/rehearsals/2026q1/` (スロット範囲 912-936) com アジェンダ em `artifacts/nexus/rehearsals/2026q2/`。 |

オスのトリメストレスは、トリメストレを行うための追加要素としての新しい動きを開発します。関連性のあるパートのルートトレースを参照し、政府機関からアンコラ `#quarterly-routed-trace-audit-schedule` を参照してください。

## バックログのアイテムを軽減する|アイテム |説明 |オーナー |アルボ |ステータス/メモ |
|------|---------------|----------|----------|-----|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` の期間にわたって TLS クエリを実行し、証拠をキャプチャして再実行し、登録を完了します。 | @release-eng、@sre-core | 2026 年第 2 四半期の Janela ルート トレース | Fechado - ハッシュ実行 TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` キャプチャ `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; o rerunco​​nfirmou que nao ha atrasados。 |
| `TRACE-MULTILANE-CANARY` 準備 | Q2 のプログラム、テレメトリのパック、OS の保証、SDK ハーネスのヘルパー検証の再利用。 | @telemetry-ops、SDK プログラム |飛行機の旅 2026-04-30 |完了 - スロット/ワークロードのメタデータに関する `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` の議題。トラッカーを使用せずにハーネスを再利用します。 |
|テレメトリ パック ダイジェスト ローテーション | Executar `scripts/telemetry/validate_nexus_telemetry_pack.py` は、レジストラが構成デルタのトラッカーをダイジェスト/リリースします。 | @telemetry-ops |リリース候補ごと |完全 - `telemetry_manifest.json` + `.sha256` em `artifacts/nexus/rehearsals/2026q1/` (スロット範囲 `912-936`、シード `NEXUS-REH-2026Q2`);コピアドをダイジェストしますが、トラッカーや証拠のインデックスはありません。 |

## Integracao はバンドル構成デルタを実行します

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 差分を共有します。新しい `defaults/nexus/*.toml` を生成し、ESS トラッカー プライムを初期化して、Reflita OS のデスタケスを取得します。
- OS 署名付き設定バンドルには、テレメトリ パックの付属品が含まれています。 O パック、`scripts/telemetry/validate_nexus_telemetry_pack.py` の有効性、開発者は、構成デルタ パラケ OS オペラドール、ポッサム再現 OS 芸術作品、Exatos USados durante B4 の証拠を公開しています。
- Iroha 2 つの永続的な sem レーンの OS バンドル: configs com `nexus.enabled = false` は、レーン/データスペース/ルーティングをオーバーライドし、パフォーマンスを維持します Nexus esteja habilitado (`--sora`)、entao remova secoes `nexus.*` das テンプレートは単一レーンです。
- Mantenha o log de voto de Government (GOV-2026-03-19) linkado Tanto no tracker quantonesta nota para que futuros votos possam copiar or formato sem redescobrir or儀式 de aprovacao。

## アコンパニャメントス ド エンサイオ デ ランカメント

- `docs/source/runbooks/nexus_multilane_rehearsal.md` プラノ カナリアのキャプチャ、ロールバックに参加する参加者のリスト。レーンのトポロジーとテレメトリアの輸出業者のランブックをすべて正確に作成します。
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 4 月 9 日までに、Q2 の準備に関するメモ/議題をリストします。 Adicione ensaios futuros は、監視トラッカーを使用して、アブリル トラッカー イソラドス パラマンターの証拠をモノトナします。
- 公開スニペットはコレクタ OTLP を実行し、エクスポートは Grafana (ver `docs/source/telemetry.md`) でバッチ処理を実行し、エクスポータ ムダルを実行します。第 1 四半期以降のバッチ サイズは 256 時間に達し、ヘッドルーム アラートが表示されます。
- CI/テストのマルチレーン アゴラ ライブ エム `integration_tests/tests/nexus/multilane_pipeline.rs` およびワークフロー `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`) の証拠、`pytests/nexus/test_multilane_pipeline.py` の参照を置き換えます。 Mantenha は、`defaults/nexus/config.toml` (`nexus.enabled = true`、blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) のハッシュを同期し、トラッカーの自動バンドルを提供します。

## ランタイムでのレーンの実行

- ランタイム アゴラの有効性とデータスペースのバインディングの計画と、データスペースの中断と調整を中止し、ファルハ、カタログ、およびカタログを管理します。 OS ヘルパーは、アポセンタダのレーンをキャッシュするレーンをリレーし、古いマージ元帳をパラケラして、古い証拠を再利用します。
- アップリケ プラノス ペロス ヘルパーの設定/ライフサイクル実行 Nexus (`State::apply_lane_lifecycle`、`Queue::apply_lane_lifecycle`) パラディショナ/リティラル レーンの設定。ルーティング、スナップショット TEU およびマニフェストのレジストリは、計画の自動更新を自動的に実行します。
- Orientacao para operadores: quando um plano falha、verifique dataspaces ausentes ou storage root que nao podem ser criados (階層化されたコールド ルート/ディレトリオス クラ ポー レーン)。 Corrija os caminhos ベースとテント ノヴァメント。レーン/データスペースのダッシュボードからテレメトリの差分を再発行し、新星トポロジーを再現します。

## Telemetria NPoS とバックプレッシャーの証拠

フェーズ B のレトロな分析は、バックプレッシャーの永続的な噂話として、ペースメーカーの NPoS を証明し、テレメトリの決定機能をキャプチャします。 `integration_tests/tests/sumeragi_npos_performance.rs` を統合してシナリオを実行し、JSON (`sumeragi_baseline_summary::<scenario>::...`) の最新のメトリクスを出力します。 localmente com を実行します。

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
````SUMERAGI_NPOS_STRESS_PEERS`、`SUMERAGI_NPOS_STRESS_COLLECTORS_K` または `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` は主要なトポロジを探索します。 os valores Padrao refletem o perfil de coletores 1 s/`k=3` usado em B4。

|シナリオ / テスト |コベルチュラ |テレメトリアチャベ |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | 12 ブロックのブロック時間は、EMA の遅延のレジストラ エンベロープ、フィラメントの詳細なゲージ、シリアル化された冗長送信アンテ、証拠のバンドルを監視します。 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |承認の延期として保証期間を延長し、確定申告を行って、大容量の報告書を輸出する必要があります。 | `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` |ペースメーカーと OS タイムアウトのジッターは、±125 パーミルとアプリケーションで表示されます。 | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` | Empurra ペイロード RBC は、OS の制限をソフト / ハードに保存し、ほとんどのアクセス セッションとコンタドール デ バイトを保存し、保存することができます。 | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |冗長な比率のゲージを再送信し、コレクタ オン ターゲット アバンセンムの送信オス コンタドールを送信し、テレメトリ ペディダ ペロ レトロ エスタ接続をエンドツーエンドで実行します。 | `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` |バックログの検証対象の OS モニターの間隔を決定するチャンクをデカルタで取得し、ドレナーの沈黙期間の OS ペイロードを確認します。 | `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |

JSON ハーネス インプリメ ジュント コム スクレープとして Prometheus をキャプチャし、政府機関の弁護士の証拠としてバックプレッシャーの対応を監視します。

## Atualizacao のチェックリスト

1. アディシオーネ・ノバス・ジャネラス・ルーテッド・トレースは、アンチガス・クアンド・オス・トリメストレス・ギラレムとして引退する。
2. Alertmanager で必要な情報を取得し、チケットを確認します。
3. Quando os config deltas mudarem、atualize o tracker、estanota e a lista de Digests do telemetry Pack no mesmo pull request。
4. ロードマップのポッサム参照、unico ドキュメント、およびアドホック分散に関する最新情報やテレメトリアをリンクします。

## 証拠のインデックス

|アティボ |ローカライザソン |メモ |
|------|----------|------|
|聴覚ルーテッドトレースとの関係 (2026 年第 1 四半期) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` |フェーズ B1 の証拠のフォンテ・カノニカ。エスペルハド パラ オ ポータル em `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`。 |
|トラッカーの構成デルタ | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA の差分に関する情報、GOV-2026-03-19 の改訂履歴。 |
|テレメトリアの治療計画 | `docs/source/nexus_telemetry_remediation_plan.md` |アラート パックのドキュメント、OTLP および B2 の輸出用ガードレールの情報。 |
|トラッカー デ エンサイオ マルチレーン | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 4 月 9 日の最新情報のリスト、マニフェスト/ダイジェスト、検証ツール、第 2 四半期のメモ/議題、ロールバックの証拠。 |
|テレメトリ パックのマニフェスト/ダイジェスト (最新) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) |レジストラ スロット範囲 912 ～ 936、シード `NEXUS-REH-2026Q2` 政府バンドルのアーティファト ハッシュ。 |
| TLS プロファイル マニフェスト | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |第 2 四半期に TLS をキャプチャーし、再実行するためにハッシュを実行します。 Routed-Trace の付録を引用します。 |
|アジェンダ TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 に関するノート (ジャネラ、スロット範囲、ワークロード シード、オーナーのアカウント)。 |
|ランカメントのランブック | `docs/source/runbooks/nexus_multilane_rehearsal.md` |運用パラステージング -> 実行 -> ロールバックのチェックリスト。レーンのトポロジーと輸出業者の方向性を決定します。 |
|テレメトリ パック バリデータ | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI リファレンス ペロ レトロ B4;アーカイブは、アオ・ラド・ド・トラッカー・センペル・ク・オ・パック・ムダールをダイジェストします。 |
|マルチレーン回帰 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prova `nexus.enabled = true` はマルチレーンを構成し、`ConfigLaneRouter` を介して、公開ダイジェストの成果物を公開するためのカタログ / マージログポーレーン (`blocks/lane_{id:03}_{slug}`) のカタログを保存します。 |