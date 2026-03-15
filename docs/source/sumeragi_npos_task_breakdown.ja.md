---
lang: ja
direction: ltr
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

<!-- 日本語訳: docs/source/sumeragi_npos_task_breakdown.md -->

## Sumeragi + NPoS タスク分解

このノートは Phase A ロードマップを小さなエンジニアリングタスクに分割し、残りの Sumeragi/NPoS 作業を段階的に進められるようにするものです。ステータス表記は `✅`（完了）、`⚙️`（進行中）、`⬜`（未着手）、`🧪`（テスト要）に従います。

### A2 - ワイヤレベルメッセージの採用
- ✅ Norito の `Proposal`/`Vote`/`Qc` 型を `BlockMessage` に公開し、エンコード／デコードの往復テストを実施（`crates/iroha_data_model/tests/consensus_roundtrip.rs`）。
- ✅ 旧 `BlockSigned/BlockCommitted` フレームを制御するトグルを退役前に `false` で固定。
- ✅ 旧ブロックメッセージを切り替えるマイグレーションノブを廃止し、Vote/commit certificate モードを唯一のワイヤ経路に。
- ✅ Torii ルータ、CLI、テレメトリ利用者を更新し、旧ブロックフレームより `/v2/sumeragi/*` JSON スナップショットを優先。
- ✅ 統合テストで Vote/commit certificate パイプラインのみを通した `/v2/sumeragi/*` エンドポイントを検証（`integration_tests/tests/sumeragi_vote_qc_commit.rs`）。
- ✅ 機能同等性と相互接続テストが揃い次第、旧フレームを削除。

### フレーム削除計画
1. ✅ Telemetry/CI ハーネスの両方で 72 h のマルチノード耐久テストを実施。Torii スナップショットで提案スループットと commit certificate 形成が安定していることを確認。
2. ✅ Vote/commit certificate パスのみで動作する統合テスト（`sumeragi_vote_qc_commit.rs`）を導入し、混在ピアでも旧フレームなしでコンセンサス可能なことを検証。
3. ✅ オペレーター向けドキュメントと CLI ヘルプから旧ワイヤ経路の記述を削除し、トラブルシューティングを Vote/commit certificate テレメトリに一本化。
4. ✅ 旧メッセージバリアント、テレメトリカウンタ、ペンディングコミットキャッシュを削除。互換性マトリクスを Vote/commit certificate-only に更新。

### A3 - エンジンとペースメーカーの強化
- ✅ `handle_message` 内で Lock/Highestcommit certificate の不変条件を強制（`block_created_header_sanity`）。
- ✅ データ可用性追跡が delivery 記録時に RBC ペイロードハッシュを検証し、不一致セッションを delivered 扱いにしない（`Actor::ensure_block_matches_rbc_payload`）。
- ✅ `require_precommit_qc` を既定設定へ組み込み、負のテストを追加（既定 `true`。ゲート有／無両パスをカバー）。
- ✅ ビュー全体の冗長送信ヒューリスティックを EMA バックのペースメーカー制御に置き換え（`aggregator_retry_deadline` がライブ EMA に基づき冗長送信期限を計算）。
- ✅ キューのバックプレッシャーを検知して提案組み立てをブロック（`BackpressureGate` がキュー飽和時にペースメーカーを停止し、ステータス／テレメトリへ延期回数を記録）。
- ✅ DA が必要な場合、proposal 検証後に availability 投票を送出（ローカル RBC `DELIVER` を待たない）。availability evidence は `availability evidence` で追跡し、commit は待機せずに進行。payload 輸送と投票の循環待ちを避ける。
- ✅ 再起動／ライブネス回帰でコールドスタート時の RBC 復旧（`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`）とダウンタイム後のペースメーカー再開（`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`）を検証。
- ✅ ロック収束を対象とした決定的な再起動／ビュー変更テストを追加（`integration_tests/tests/sumeragi_lock_convergence.rs`）。

### A4 - コレクタ＆ランダムネスパイプライン
- ✅ 決定的コレクタローテーションヘルパーを `collectors.rs` に実装。
- ✅ GA-A4.1 - PRF バックのコレクタ選出で決定的シードと height/view を `/status` とテレメトリに記録。VRF リフレッシュがコミット後のコンテキストを伝搬。オーナー: `@sumeragi-core`。トラッカー: `project_tracker/npos_sumeragi_phase_a.md`（クローズ）。
- ✅ GA-A4.2 - リビール参加テレメトリと CLI コマンドを公開し、Norito マニフェストを更新。オーナー: `@telemetry-ops`, `@torii-sdk`。トラッカー: `project_tracker/npos_sumeragi_phase_a.md:6`。
- ✅ GA-A4.3 - 遅延リビール復旧とゼロ参加エポックのテストを `integration_tests/tests/sumeragi_randomness.rs` に整備（`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`）。ペナルティ解除テレメトリを検証。オーナー: `@sumeragi-core`。トラッカー: `project_tracker/npos_sumeragi_phase_a.md:7`。

### A5 - 共同リコンフィグとエビデンス
- ✅ エビデンスの足場、WSV 永続化、Norito 往復が二重投票、無効提案、無効 commit certificate、二重 exec バリアントをカバー。決定的な重複排除と地平線カットを実装（`sumeragi::evidence`）。
- ✅ GA-A5.1 - 旧セットがコミットし新セットが次ブロックで有効化されるジョイントコンセンサスを強制し、対象統合テストを追加。
- ✅ GA-A5.2 - スラッシュ／拘束ガバナンス文書と CLI フローを更新し、mdBook 同期テストでデフォルトと evidence horizon 記述を固定。
- ✅ GA-A5.3 - 重複署名者、偽署名、古いエポック再生、マニフェスト混在といった負の経路テストと fuzz フィクスチャを導入し、Norito 往復検証をナイトリーで保護。

### A6 - ツーリング・ドキュメント・検証
- ✅ RBC テレメトリ／レポートを整備。DA レポートが実メトリクス（追い出しカウンタ含む）を生成。
- ✅ GA-A6.1 - VRF 有効な 4 ピア NPoS ハッピーパスを CI で実行し、ペースメーカー／RBC 閾値を強制（`integration_tests/tests/sumeragi_npos_happy_path.rs`）。オーナー: `@qa-consensus`, `@telemetry-ops`。トラッカー: `project_tracker/npos_sumeragi_phase_a.md:11`。
- ✅ GA-A6.2 - NPoS パフォーマンスベースライン（1 s ブロック、k=3）を測定し、`status.md`／運用ドキュメントへ掲載（再現可能なシード／ハードウェアマトリクス付）。レポート: `docs/source/generated/sumeragi_baseline_report.md`。トラッカー: `project_tracker/npos_sumeragi_phase_a.md:12`。記録は Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) 上で `scripts/run_sumeragi_baseline.py` に記載のコマンドを使用。
- ✅ GA-A6.3 - RBC/pacemaker/backpressure 計測のトラブルシューティングガイドを整備（`docs/source/telemetry.md:523`）。ログ相関は `scripts/sumeragi_backpressure_log_scraper.py` が担い、pacemaker deferral と missing-availability のペアを手作業 grep なしで取得可能。オーナー: `@operator-docs`, `@telemetry-ops`。トラッカー: `project_tracker/npos_sumeragi_phase_a.md:13`。
- ✅ RBC ストア／チャンク損失パフォーマンスシナリオ（`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`）、冗長ファンアウトカバレッジ（`npos_redundant_send_retries_update_metrics`）、バウンデッドジッタハーネス（`npos_pacemaker_jitter_within_band`）を追加し、A6 スイートがストア soft-limit の deferrals、決定的チャンクドロップ、冗長送信テレメトリ、ペースメーカーのジッタ帯をストレス下で検証するようにした。[integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### 即時タスク
1. ✅ バウンデッドジッタハーネスでペースメーカージッタを測定（`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`）。
2. ✅ `npos_queue_backpressure_triggers_metrics` で RBC 遅延アサーションを強化し、決定的なストア圧力を与える（`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`）。
3. ✅ `/v2/sumeragi/telemetry` の長時間ソークを実施し、複数の高さにわたってスナップショットと Prometheus カウンタを比較。アドバサリアルなコレクタを含むテストでカバー（`integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`）。

このリストをここで管理することで `roadmap.md` はマイルストン中心に保たれ、チームは進捗を追えるチェックリストを得られます。パッチを反映したら随時更新し、完了を記録してください。
