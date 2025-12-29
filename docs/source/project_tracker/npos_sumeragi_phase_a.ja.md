<!-- Japanese translation of docs/source/project_tracker/npos_sumeragi_phase_a.md -->

---
lang: ja
direction: ltr
source: docs/source/project_tracker/npos_sumeragi_phase_a.md
status: complete
translator: manual
---

% NPoS Sumeragi フェーズA トラッカー下書き (2025年12月)

2025-12-03 に配布。シーケンス表は確認のため `@sumeragi-core`、`@telemetry-ops`、`@torii-sdk`、`@governance`、`@qa-consensus`、`@performance-lab`、`@operator-docs` に回覧済みです。担当者は共有トラッカースレッド（#npos-phase-a-sync）で受領・承認状況や依存リスクを返信してください。

| チケットID | マイルストーン | 概要 | 担当 | 依存関係 | 目標完了日 | 備考 |
|-----------|-----------|---------|--------|--------------|-------------|-------|
| GA-A4.1 | A4 — Collector & Randomness Pipeline | PRF 駆動のコレクタ選定を確定し、`status`／テレメトリに決定的なシードスナップショットを公開する。 | `@sumeragi-core` | Pacemaker／DA メトリクス (A3) | 2026-01-05 | マージ前に `@torii-sdk` と CLI フラグをレビューすること。 |
| GA-A4.2 | A4 — Collector & Randomness Pipeline | リビール参加メトリクスと CLI 点検コマンドを公開し、Norito マニフェスト更新を出荷する。 | `@telemetry-ops`, `@torii-sdk` | GA-A4.1 | 2026-01-19 | リビール遅延用 Prometheus アラートテンプレートを追加。完了（テレメトリ要約と CLI は 2025年12月に着地済み）。 |
| GA-A4.3 | A4 — Collector & Randomness Pipeline | `integration_tests/tests/sumeragi_randomness.rs` に遅延リビール復旧とゼロ参加エポックテストを codify。 | `@sumeragi-core` | GA-A4.1 | 2026-01-31 | 完了（`npos_late_vrf_reveal_clears_penalty_and_preserves_seed` と `npos_zero_participation_epoch_reports_full_no_participation` でテレメトリカウンタ確定）。 |
| GA-A5.1 | A5 — Joint Reconfiguration & Evidence | 共同コンセンサスのアクティベーションゲート（旧セットがコミット、新セットは +1 で有効化）を強制し、統合テストを拡張。 | `@sumeragi-core` | GA-A4.3 | 2026-02-21 | 完了 — アクティベーション遅延セマンティクスをカバーする統合テストを追加。リハーサルメモはガバナンスに保管済み。 |
| GA-A5.2 | A5 — Joint Reconfiguration & Evidence | スラッシング／ジェイリングフローのガバナンスドキュメントと CLI を更新し、mdBook の doc-test を追加。 | `@governance`, `@torii-sdk` | GA-A5.1 | 2026-03-05 | 完了 — ドキュメント、CLI ヘルパー、mdBook の doctest を Norito 例とともに更新。 |
| GA-A5.3 | A5 — Joint Reconfiguration & Evidence | 負のパス証拠テスト（重複署名者、偽造署名、古いエポック再生）を拡張。 | `@sumeragi-core`, `@qa-consensus` | GA-A5.1 | 2026-03-14 | 完了 — ファズ用フィクスチャとナイトリー実行で重複署名者、偽造署名、古いホライズン、混在マニフェスト案件を防御。 |
| GA-A6.1 | A6 — Tooling, Docs, and Validation | テレメトリしきい値と RBC ゲーティング検証を備えた VRF 有効 4 ピアのハッピーパス試験を自動化。 | `@qa-consensus`, `@telemetry-ops` | GA-A5.3 | 2026-04-07 | 完了 — NPoS ハッピーパス統合テストが CI で稼働し、ペースメーカ／RBC しきい値はランブックに記載済み。 |
| GA-A6.2 | A6 — Tooling, Docs, and Validation | NPoS パフォーマンス基準値（1 秒ブロック, k=3）を取得し、`status.md`／オペレータドキュメントに記録。 | `@performance-lab`, `@telemetry-ops` | GA-A6.1 | 2026-04-21 | 完了 — Apple M2 Ultra（24 コア、192 GB RAM、macOS 15.0）；`docs/source/generated/sumeragi_baseline_report.md` を参照。 |
| GA-A6.3 | A6 — Tooling, Docs, and Validation | RBC／ペースメーカ／バックプレッシャ計測に関するオペレータ向けトラブルシューティングガイドを公開。 | `@operator-docs`, `@telemetry-ops` | GA-A6.1 | 2026-04-28 | 完了 — `docs/source/telemetry.md:523` にトラブルシューティングランブックを追加。`scripts/sumeragi_backpressure_log_scraper.py` により遅延／RBC 突き合わせの自動ログ相関を提供。 |
| GA-A6.4 | A6 — Tooling, Docs, and Validation | パフォーマンスハーネスを RBC ストアのバックプレッシャとチャンク欠損シナリオで拡張（`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`）。 | `@performance-lab`, `@telemetry-ops` | GA-A6.2 | 2026-05-05 | 完了 — `npos_redundant_send_retries_update_metrics` で冗長送信のファンアウト再試行をカバーし、`npos_pacemaker_jitter_within_band` でペースメーカのジッタ帯域を検証（`cargo test -p integration_tests --test sumeragi_npos_performance` を参照）。 |
